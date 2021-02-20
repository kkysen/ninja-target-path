pub struct Parser<'a> {
    bytes: &'a [u8],
    rest: &'a [u8],
    consume: bool,
}

impl<'a> Parser<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            rest: bytes,
            consume: true,
        }
    }
}

impl Parser<'_> {
    pub fn consume(&mut self, consume: bool) {
        self.consume = consume;
    }
    
    pub fn is_done(&self) -> bool {
        self.rest.is_empty()
    }
    
    pub fn has_more(&self) -> bool {
        !self.is_done()
    }
    
    pub fn skip(&mut self, n: usize) {
        if self.consume {
            self.rest = &self.rest[n..];
        }
    }
    
    pub fn lookahead<T, F>(&mut self, mut f: F) -> T
        where F: FnMut(&mut Self) -> T {
        self.consume(false);
        let t = f(self);
        self.consume(true);
        t
    }
}

impl<'a> Parser<'a> {
    pub fn current_position(&self) -> usize {
        // Why is offset_from unsafe but this isn't?
        // I think b/c offset_from works on pointers to non-1-byte types, too
        let start_ptr = self.bytes.as_ptr() as usize;
        let current_ptr = self.rest.as_ptr() as usize;
        current_ptr - start_ptr
    }
    
    pub fn surrounding2(&self, mut before: usize, mut after: usize) -> &'a [u8] {
        let current = self.current_position();
        let remaining = self.bytes.len() - current;
        if before > current {
            before = current;
        }
        if after > remaining {
            after = remaining;
        }
        &self.bytes[current - before..current + after]
    }
    
    pub fn surrounding(&self, before_and_after: usize) -> &'a [u8] {
        self.surrounding2(before_and_after, before_and_after)
    }
    
    pub fn before(&self, n: usize) -> &'a [u8] {
        self.surrounding2(n, 0)
    }
    
    pub fn after(&self, n: usize) -> &'a [u8] {
        self.surrounding2(0, n)
    }
}

impl<'a> Parser<'a> {
    pub fn until_matching(&mut self, mut predicate: impl FnMut(u8) -> bool) -> &'a [u8] {
        let i = match self.rest
            .iter()
            .position(|&b| predicate(b)) {
            None => self.rest.len(),
            Some(i) => i,
        };
        let (chunk, rest) = self.rest.split_at(i);
        if self.consume {
            self.rest = rest;
        }
        chunk
    }
    
    pub fn until(&mut self, b: u8) -> &'a [u8] {
        let before = self.until_matching(|a| a == b);
        if self.has_more() {
            self.skip(1);
        }
        before
    }
    
    pub fn line(&mut self) -> &'a [u8] {
        let line = self.until(b'\n');
        // support CRLF, too
        match line.split_last() {
            Some((b'\r', line)) => line,
            _ => line,
        }
    }
    
    pub fn count(&mut self, b: u8) -> usize {
        let consecutive = self.until_matching(|a| a != b);
        consecutive.len()
    }
}

impl Iterator for Parser<'_> {
    type Item = u8;
    
    fn next(&mut self) -> Option<Self::Item> {
        let (&next, rest) = self.rest.split_first()?;
        self.rest = rest;
        Some(next)
    }
}
