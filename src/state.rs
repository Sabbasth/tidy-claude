//! Mutable run-time state passed through the CLI.

use std::collections::HashMap;

pub struct RunState {
    pub debug: bool,
    pub stats: HashMap<String, usize>,
}

impl RunState {
    pub fn new(debug: bool) -> Self {
        Self {
            debug,
            stats: HashMap::new(),
        }
    }

    pub fn log(&self, msg: &str) {
        if self.debug {
            eprintln!("{}", msg);
        }
    }

    pub fn count(&mut self, key: &str, n: usize) {
        *self.stats.entry(key.to_string()).or_insert(0) += n;
    }
}

impl Default for RunState {
    fn default() -> Self {
        Self::new(false)
    }
}
