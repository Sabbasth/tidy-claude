//! Execution state: logging and context.

/// Execution state carrying debug mode and logging context.
pub struct RunState {
    pub debug: bool,
}

impl RunState {
    pub fn new(debug: bool) -> Self {
        Self { debug }
    }

    /// Log a message if debug mode is enabled.
    pub fn log(&self, msg: &str) {
        if self.debug {
            eprintln!("{}", msg);
        }
    }
}
