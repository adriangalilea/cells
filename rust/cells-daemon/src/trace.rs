//! What the engine does, said as it does it: `tracing` records written the way the utils
//! logging contract writes events (github.com/adriangalilea/utils): the target names the
//! scope (`cells::engine`), the message is fixed and says what happened, the values are
//! fields, and a piece of work is a span (`serve`, a `change` and everything it stored).
//!
//! Where they go is the host's: Cells.app's `CellsEngine.app` hands a sink
//! (`cells_engine_serve`) and writes each record through swift-utils' `Log`, the contract's
//! writer, so the engine's lines join the app's stream, its unified log and its traces. A
//! `cells serve` run without one says them on stderr as a person reads them: the local
//! time, the message, the fields, each span's lines under it.
//!
//! Every message `info` is said always: the folders served, every change heard, every
//! write and every write not made, who joined and left. `debug` (`--verbose`, or the host's
//! threshold) adds each file read again, what it found and how long it took.
//!
//! This copies tether's bridge (tether-ffi, `log.rs`) once, for a C host; a third Rust
//! program that logs is where both become one crate beside the contract's other writers.

use std::ffi::{CString, c_char};
use std::fmt::Write as _;
use std::io::Write as _;

use serde_json::{Map, Value, json};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;

/// The engine's own records: every one carries it, and only these are said.
pub const ENGINE: &str = "cells::engine";

/// A record handed to a host: one JSON object, a C string the host copies before returning.
pub type Sink = extern "C" fn(*const c_char);

/// The engine's records said on stderr for the rest of the process, `debug` ones too when
/// `verbose`.
pub fn to_stderr(verbose: bool) {
    let level = if verbose { Level::DEBUG } else { Level::INFO };
    // a second call (a test, a host that set its own) keeps the first
    let _ = tracing_subscriber::registry().with(Targets::new().with_target("cells", level)).with(Human).try_init();
}

/// The engine's records handed to `sink` for the rest of the process, every level: the
/// host's writer decides what it shows.
pub fn to_host(sink: Sink) {
    let _ = tracing_subscriber::registry().with(Targets::new().with_target("cells", Level::TRACE)).with(Host(sink)).try_init();
}

/// A record's fields, the message apart.
#[derive(Default)]
struct Fields {
    message: String,
    rest: Map<String, Value>,
}

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "message" => self.message = value.into(),
            name => _ = self.rest.insert(name.into(), Value::from(value)),
        }
    }
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        match field.name() {
            "message" => self.message = format!("{value:?}"),
            name => _ = self.rest.insert(name.into(), Value::from(format!("{value:?}"))),
        }
    }
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.rest.insert(field.name().into(), Value::from(value));
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        self.rest.insert(field.name().into(), Value::from(value));
    }
    fn record_f64(&mut self, field: &Field, value: f64) {
        self.rest.insert(field.name().into(), Value::from(value));
    }
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.rest.insert(field.name().into(), Value::from(value));
    }
}

/// The scope a target names, under `cells`: `cells::engine` is `["engine"]`.
fn scope(target: &str) -> Vec<&str> {
    target.strip_prefix("cells").unwrap_or(target).split("::").filter(|s| !s.is_empty()).collect()
}

fn level(l: &Level) -> &'static str {
    match *l {
        Level::ERROR => "error",
        Level::WARN => "warn",
        Level::INFO => "info",
        Level::DEBUG => "debug",
        Level::TRACE => "trace",
    }
}

/// Each record as one JSON object to the host:
///
/// - `{"phase":"start","span":7,"parent":3,"level":"info","scope":["engine"],"message":"change","fields":{…}}`
/// - `{"phase":"end","span":7}`
/// - `{"span":7,"level":"info","scope":["engine"],"message":"stored","fields":{…}}`, a line,
///   in the span it was said in (`null` outside every span)
struct Host(Sink);

impl Host {
    fn send(&self, record: Value) {
        let text = CString::new(record.to_string()).expect("JSON holds no NUL");
        (self.0)(text.as_ptr());
    }
}

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for Host {
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let mut fields = Fields::default();
        attrs.record(&mut fields);
        let parent = ctx.span(id).and_then(|s| s.parent()).map(|p| p.id().into_u64());
        let meta = attrs.metadata();
        self.send(json!({
            "phase": "start", "span": id.into_u64(), "parent": parent, "level": level(meta.level()),
            "scope": scope(meta.target()), "message": meta.name(), "fields": fields.rest,
        }));
    }

    fn on_close(&self, id: Id, _: Context<'_, S>) {
        self.send(json!({ "phase": "end", "span": id.into_u64() }));
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let mut fields = Fields::default();
        event.record(&mut fields);
        let span = ctx.event_span(event).map(|s| s.id().into_u64());
        let meta = event.metadata();
        self.send(json!({
            "span": span, "level": level(meta.level()), "scope": scope(meta.target()),
            "message": fields.message, "fields": fields.rest,
        }));
    }
}

/// Each record as a person reads it on stderr: `13:24:40.799 stored   quarters.cells
/// via=q4/money.cells …`, a span's lines under its own, the `path` field first and bare.
struct Human;

/// A record as a line, without its time: the message, then the fields.
fn line(depth: usize, message: &str, fields: &Map<String, Value>) -> String {
    let mut out = format!("{}{message:<8}", "  ".repeat(depth));
    let bare = |v: &Value| match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    if let Some(path) = fields.get("path") {
        let _ = write!(out, " {}", bare(path));
    }
    for (k, v) in fields.iter().filter(|(k, _)| *k != "path") {
        let _ = write!(out, " {k}={}", bare(v));
    }
    out
}

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for Human {
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let mut fields = Fields::default();
        attrs.record(&mut fields);
        let depth = ctx.span(id).map_or(0, |s| s.scope().count() - 1);
        say(&line(depth, attrs.metadata().name(), &fields.rest));
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let mut fields = Fields::default();
        event.record(&mut fields);
        let depth = ctx.event_scope(event).map_or(0, |s| s.count());
        say(&line(depth, &fields.message, &fields.rest));
    }
}

fn say(line: &str) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("the clock is after 1970").as_millis() as u64;
    let _ = writeln!(std::io::stderr(), "{} {line}", clock(now));
}

/// A time in milliseconds since the Unix epoch on the Mac's clock as a person reads it (its
/// time zone).
fn local(ms: u64) -> libc::tm {
    let secs = (ms / 1000) as libc::time_t;
    // SAFETY: an all-zero `tm` is a valid value, and localtime_r writes into the one it is
    // handed, touching nothing shared.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let found = unsafe { !libc::localtime_r(&secs, &mut tm).is_null() };
    assert!(found, "localtime_r has no local time for {secs}");
    tm
}

/// `HH:MM:SS.mmm`, local.
fn clock(ms: u64) -> String {
    let tm = local(ms);
    format!("{:02}:{:02}:{:02}.{:03}", tm.tm_hour, tm.tm_min, tm.tm_sec, ms % 1000)
}

/// `YYYY-MM-DD HH:MM`, local: when history says something happened.
pub fn when(ms: u64) -> String {
    let tm = local(ms);
    format!("{}-{:02}-{:02} {:02}:{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min)
}

/// The engine's records said while `f` runs on this thread, each as `Human` writes it
/// without its time: what a test reads.
#[cfg(test)]
pub(crate) fn caught<R>(f: impl FnOnce() -> R) -> (R, Vec<String>) {
    use std::sync::{Arc, Mutex};
    struct Catch(Arc<Mutex<Vec<String>>>);
    impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for Catch {
        fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
            let mut fields = Fields::default();
            attrs.record(&mut fields);
            let depth = ctx.span(id).map_or(0, |s| s.scope().count() - 1);
            self.0.lock().unwrap().push(line(depth, attrs.metadata().name(), &fields.rest));
        }
        fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
            let mut fields = Fields::default();
            event.record(&mut fields);
            let depth = ctx.event_scope(event).map_or(0, |s| s.count());
            self.0.lock().unwrap().push(line(depth, &fields.message, &fields.rest));
        }
    }
    let lines = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::registry().with(Catch(Arc::clone(&lines)));
    let out = tracing::subscriber::with_default(subscriber, f);
    let lines = lines.lock().unwrap().clone();
    (out, lines)
}
