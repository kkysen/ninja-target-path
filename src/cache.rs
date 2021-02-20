use Entry::{Occupied, Vacant};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use apply::Apply;
use checked_command::CheckedCommand;

use crate::ninja;
use crate::util::err;
use std::str::from_utf8;

pub struct Cache {
    path: PathBuf,
    cache: HashMap<String, PathBuf>,
    dirty: bool,
}

impl Cache {
    pub fn path(&self) -> &Path {
        self.path.as_path()
    }
    
    pub fn dir(&self) -> &Path {
        // constructor Self::read adds the file name to the dir, so I can always unwrap this
        self.path().parent().unwrap()
    }
    
    pub fn read(dir: PathBuf) -> anyhow::Result<Self> {
        let path = {
            let mut path = dir;
            path.push(".targets.cache.toml");
            path
        };
        let cache = match fs_err::read(path.as_path()) {
            Ok(bytes) => toml::from_slice(bytes.as_slice())?,
            Err(error) => match error.kind() {
                ErrorKind::NotFound => HashMap::new(),
                _ => Err(error)?,
            },
        };
        Self {
            path,
            cache,
            dirty: false,
        }.apply(Ok)
    }
    
    pub fn write(&mut self) -> anyhow::Result<()> {
        if !self.dirty {
            return Ok(());
        }
        let bytes = toml::to_vec(&self.cache)?;
        fs_err::write(self.path(), bytes)?;
        self.dirty = false;
        Ok(())
    }
    
    pub fn get(&mut self, target: String) -> anyhow::Result<PathBuf> {
        let dir = self.dir().to_path_buf();
        let relative_path = match self.cache.entry(target) {
            Occupied(entry) => entry.into_mut(),
            Vacant(entry) => {
                let path = Self::lookup(dir.as_path(), entry.key())?;
                self.dirty = true;
                entry.insert(path)
            }
        }.as_path();
        let mut path = dir;
        path.push(relative_path);
        Ok(path)
    }
    
    fn lookup(dir: &Path, target_name: &str) -> anyhow::Result<PathBuf> {
        let output = CheckedCommand::new("ninja")
            .arg("-C")
            .arg(dir)
            .arg("-t")
            .arg("query")
            .arg(target_name)
            .stderr(Stdio::inherit())
            .output()?;
        let query = ninja::Query::parse(output.stdout.as_slice())
            .map_err(|e| {
                println!("{}", from_utf8(output.stdout.as_slice()).unwrap());
                e
            })?;
        println!("{:#?}", &query);
        if query.targets.len() != 1 {
            err("only expecting 1 target")?;
        };
        let target = &query.targets[0];
        if target.name != target_name.as_bytes() {
            err("wrong target name")?;
        }
        if !target.outputs.is_empty() {
            err("expecting 0 outputs")?;
        }
        if target.rule != b"phony" {
            err("expecting phony rule")?;
        }
        query
            .targets[0]
            .inputs
            .normal[0]
            .as_path()
            .to_path_buf()
            .apply(Ok)
    }
}

impl Drop for Cache {
    fn drop(&mut self) {
        self.write().unwrap()
    }
}
