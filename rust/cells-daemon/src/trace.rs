//! What the daemon does, said as it does it: one line per step on stderr, its local time
//! (the log keeps the same instant as `at`, milliseconds since the Unix epoch), a verb and
//! what it acted on. `said` lines are always written: the
//! folders served, every change heard, every write and every write not made. `traced`
//! lines only with `--verbose`: each file read again, what it found and how long it took,
//! and each batch of paths heard.

use std::cell::RefCell;

pub struct Trace {
    verbose: bool,
    /// Every line, kept instead of written when a test reads them.
    kept: Option<RefCell<Vec<String>>>,
}

impl Trace {
    pub fn new(verbose: bool) -> Trace {
        Trace { verbose, kept: None }
    }

    /// A trace that keeps its lines, the verbose ones too, for a test to read.
    pub fn kept() -> Trace {
        Trace { verbose: true, kept: Some(RefCell::new(Vec::new())) }
    }

    pub fn lines(&self) -> Vec<String> {
        self.kept.as_ref().expect("a kept trace").borrow().clone()
    }

    pub fn said(&self, at: u64, verb: &str, what: &str) {
        let line = format!("{} {verb:<8} {what}", clock(at));
        match &self.kept {
            Some(kept) => kept.borrow_mut().push(line),
            None => eprintln!("{line}"),
        }
    }

    pub fn traced(&self, at: u64, verb: &str, what: &str) {
        if self.verbose {
            self.said(at, verb, what);
        }
    }
}

/// `HH:MM:SS.mmm` of a time in milliseconds since the Unix epoch, on the Mac's clock as a
/// person reads it (its time zone).
fn clock(ms: u64) -> String {
    let secs = (ms / 1000) as libc::time_t;
    // SAFETY: an all-zero `tm` is a valid value, and localtime_r writes into the one it is
    // handed, touching nothing shared.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let found = unsafe { !libc::localtime_r(&secs, &mut tm).is_null() };
    assert!(found, "localtime_r has no local time for {secs}");
    format!("{:02}:{:02}:{:02}.{:03}", tm.tm_hour, tm.tm_min, tm.tm_sec, ms % 1000)
}
