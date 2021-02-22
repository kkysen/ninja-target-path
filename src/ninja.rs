use std::ffi::OsStr;
use std::fmt::{Debug, Display, Formatter};
use std::fmt;
use std::ops::{Deref, Index};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::str::from_utf8;

use anyhow::Context;
use apply::Apply;

use DependencyType::{Implicit, Normal, Ordered};

use crate::parser::Parser;
use crate::util::err;
use std::collections::HashMap;

impl Parser<'_> {
    fn check_count(&mut self, b: u8, expected: usize, prefix: impl Display) -> anyhow::Result<()> {
        let actual = self.count(b);
        if actual != expected {
            err(format!("{}: expected {} '{}'s but found {}", prefix, expected, b as char, actual))?;
        }
        Ok(())
    }
}

enum DependencyType {
    Normal,
    Implicit,
    Ordered,
}

impl Parser<'_> {
    fn dependency_type(&mut self) -> anyhow::Result<DependencyType> {
        self.check_count(b' ', 4, "input")?;
        let num_bars = self.count(b'|');
        let dependency_type = match num_bars {
            0 => Normal,
            1 => Implicit,
            2 => Ordered,
            _ => err("more than 2 '|'s for an input")?,
        };
        if num_bars > 0 && self.count(b' ') != 1 {
            err("no ' ' following '|'s for an input")?;
        }
        Ok(dependency_type)
    }
}

#[derive(Eq, PartialEq, Copy, Clone, Ord, PartialOrd, Hash)]
pub struct Bytes<'a>(pub &'a [u8]);

impl<'a> From<&'a [u8]> for Bytes<'a> {
    fn from(this: &'a [u8]) -> Self {
        Self(this)
    }
}

impl<'a> From<&'a OsStr> for Bytes<'a> {
    fn from(this: &'a OsStr) -> Self {
        this.as_bytes().into()
    }
}

impl<'a> From<&'a str> for Bytes<'a> {
    fn from(this: &'a str) -> Self {
        this.as_bytes().into()
    }
}

impl<'a> Deref for Bytes<'a> {
    type Target = &'a [u8];
    
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, T> PartialEq<T> for Bytes<'a> where T: AsRef<[u8]> {
    fn eq(&self, other: &T) -> bool {
        self.0 == other.as_ref()
    }
}

impl Display for Bytes<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match from_utf8(self) {
            Ok(s) => f.write_str(s),
            Err(e) => write!(f, "{}", e),
        }
    }
}

impl Debug for Bytes<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "\"{}\"", self)
    }
}

impl<'a> Bytes<'a> {
    pub fn as_os_str(&self) -> &'a OsStr {
        OsStr::from_bytes(self.0)
    }
    
    pub fn as_path(&self) -> &'a Path {
        Path::new(self.as_os_str())
    }
}

#[derive(Debug)]
pub struct Dependencies<'a> {
    pub normal: Vec<Bytes<'a>>,
    pub implicit: Vec<Bytes<'a>>,
    pub ordered: Vec<Bytes<'a>>,
}

impl<'a> Parser<'a> {
    fn dependencies(&mut self) -> anyhow::Result<Dependencies<'a>> {
        let mut normal: Vec<Bytes<'a>> = Vec::new();
        let mut implicit: Vec<Bytes<'a>> = Vec::new();
        let mut ordered: Vec<Bytes<'a>> = Vec::new();
        while self.lookahead(|this| this.count(b' ') == 4) {
            let dependencies = match self.dependency_type()? {
                Normal => &mut normal,
                Implicit => &mut implicit,
                Ordered => &mut ordered,
            };
            dependencies.push(self.line().into());
        }
        Dependencies {
            normal,
            implicit,
            ordered,
        }.apply(Ok)
    }
}

#[derive(Debug)]
pub struct Target<'a> {
    pub name: Bytes<'a>,
    pub rule: Bytes<'a>,
    pub inputs: Dependencies<'a>,
    pub outputs: Vec<Bytes<'a>>,
}

impl<'a> Parser<'a> {
    /// Parse the target name.
    fn name(&mut self) -> anyhow::Result<Bytes<'a>> {
        let name = self.until(b':');
        if self.line() != b"" {
            err("extra bytes on line after target name")?;
        }
        Ok(name.into())
    }
    
    /// Parse a dependencies header, i.e., either `input` or `output`.
    fn dependencies_header(&mut self, name: &str) -> anyhow::Result<()> {
        self.check_count(b' ', 2, format_args!("indent for '{}'", name))?;
        if self.until(b':') != name.as_bytes() {
            err(format!("'{}' missing", name))?;
        }
        Ok(())
    }
    
    /// Parse a ninja target, at least what `ninja -t query` outputs.
    ///
    /// The expected form is:
    /// ```ninja
    /// {target}:
    ///   input: {rule}
    ///     {input}
    ///     ...
    ///     | {implicit_input}
    ///     ...
    ///     || {order_input}
    ///     ...
    ///   outputs:
    ///     {output}
    ///     ...
    /// ```
    fn target(&mut self) -> anyhow::Result<Target<'a>> {
        let name = self.name()?;
        self.dependencies_header("input")?;
        self.check_count(b' ', 1, "after 'input'")?;
        let rule = self.line().into();
        let inputs = self.dependencies()?;
        self.dependencies_header("outputs")?;
        if self.line() != b"" {
            err("extra bytes on line after 'outputs':")?;
        }
        let outputs = self.dependencies()?;
        if !outputs.implicit.is_empty() {
            err("implicit '|' outputs are not allowed")?;
        }
        if !outputs.ordered.is_empty() {
            err("ordered '||' outputs are not allowed")?;
        }
        let outputs = outputs.normal;
        Target {
            name,
            rule,
            inputs,
            outputs,
        }.apply(Ok)
    }
}

#[derive(Debug)]
pub struct Query<'a> {
    pub targets: HashMap<Bytes<'a>, Target<'a>>,
}

impl<'a> Parser<'a> {
    fn query(&mut self) -> anyhow::Result<Query<'a>> {
        let mut targets = HashMap::new();
        while self.has_more() {
            let target = self.target()?;
            targets.insert(target.name, target);
        }
        Query {
            targets,
        }.apply(Ok)
    }
}

impl<'a> Query<'a> {
    pub fn parse(bytes: &'a [u8]) -> anyhow::Result<Query<'a>> {
        let mut parser = Parser::new(bytes);
        parser.query().context(format!("index {}: '{}'",
                                       parser.current_position(),
                                       from_utf8(parser.surrounding(5))?
        ))
    }
}

impl<'a, 'b: 'a, T: Into<Bytes<'a>>> Index<T> for Query<'a> {
    type Output = Target<'a>;
    
    fn index(&self, index: T) -> &Self::Output {
        &self.targets[&index.into()]
    }
}
