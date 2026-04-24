// state.rs — RunState mutable runtime state (to be implemented in Phase 2)

#[derive(Default)]
pub struct RunState {
    pub debug: bool,
    pub stats: std::collections::HashMap<String, usize>,
}

impl RunState {
    pub fn log(&self, msg: &str) {
        if self.debug {
            eprintln!("[debug] {msg}");
        }
    }

    pub fn count(&mut self, key: &str, n: usize) {
        *self.stats.entry(key.to_string()).or_default() += n;
    }
}
