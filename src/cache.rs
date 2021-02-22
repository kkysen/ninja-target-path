use Entry::{Occupied, Vacant};
use std::{env, io};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::str::from_utf8;

use apply::Apply;
use checked_command::CheckedCommand;
use fs_err::{File, OpenOptions};

use crate::ninja;
use crate::util::err;

#[derive(Debug)]
pub struct Cache<'a> {
    dir: &'a Path,
    file: File,
    cache: HashMap<String, PathBuf>,
    dirty: bool,
}

impl<'a> Cache<'a> {
    pub const FILE_NAME: &'static str = "targets.cache.toml";
    
    pub fn read(dir: &'a Path) -> anyhow::Result<Self> {
        let path = dir.join(Self::FILE_NAME);
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;
        let metadata = file.metadata();
        let len = metadata
            .as_ref()
            .map(|m| m.len() as usize + 1) // null-terminated
            .unwrap_or(0);
        let is_out_of_date = (|| -> io::Result<bool> {
            Ok(metadata?.modified()? < env::current_exe()?.metadata()?.modified()?)
        })().unwrap_or(false);
        let cache = if is_out_of_date {
            HashMap::new()
        } else {
            let mut bytes = Vec::with_capacity(len);
            file.read_to_end(&mut bytes)?;
            toml::from_slice(bytes.as_slice())?
        };
        let this = Self {
            dir,
            file,
            cache,
            dirty: false,
        };
        this.apply(Ok)
    }
    
    pub fn write(&mut self) -> anyhow::Result<()> {
        if !self.dirty {
            return Ok(());
        }
        let bytes = toml::to_vec(&self.cache)?;
        self.file.set_len(0)?;
        self.file.write_all(bytes.as_slice())?;
        self.dirty = false;
        Ok(())
    }
    
    pub fn write_drop(mut self) -> anyhow::Result<()> {
        let result = self.write();
        // don't write again in drop()
        self.dirty = false;
        result
    }
}

impl Drop for Cache<'_> {
    fn drop(&mut self) {
        self.write().unwrap()
    }
}

impl Cache<'_> {
    pub fn get(&mut self, target: OsString) -> anyhow::Result<PathBuf> {
        let dir = self.dir.to_path_buf();
        let target_str = target.into_string().unwrap();
        let relative_path = match self.cache.entry(target_str) {
            Occupied(entry) => entry.into_mut(),
            Vacant(entry) => {
                let path = Self::lookup(dir.as_path(), OsStr::new(entry.key()))?;
                self.dirty = true;
                entry.insert(path)
            }
        }.as_path();
        let mut path = dir;
        path.push(relative_path);
        Ok(path)
    }
    
    fn lookup(dir: &Path, target_name: &OsStr) -> anyhow::Result<PathBuf> {
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
        // println!("{:#?}", &query);
        let target = &query[target_name];
        if !target.outputs.is_empty() {
            err("expecting 0 outputs")?;
        }
        if target.rule != b"phony" {
            err("expecting phony rule")?;
        }
        target
            .inputs
            .normal[0]
            .as_path()
            .to_path_buf()
            .apply(Ok)
    }
}
