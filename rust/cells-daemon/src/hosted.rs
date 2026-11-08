//! The engine as a host app runs it: one C function, `cells_engine_serve`, declared in
//! `include/cells_engine.h`. Cells.app's `CellsEngine.app` is a Swift `main` that watches its
//! own image (swift-utils' `AppInstallations`, which every process of an app links) and calls
//! this, so Swift owns the app's life on the Mac and Rust owns the engine, its folder and its
//! log included. What the engine says goes to the host's `record`, one JSON object a call
//! (`trace`), so the host writes it as its own log does.

use std::ffi::c_char;

/// The engine, until the process ends; returns only when it cannot run, its reason handed
/// to `record` as an error, with the exit status a launchd job should end with.
#[unsafe(no_mangle)]
pub extern "C" fn cells_engine_serve(record: extern "C" fn(*const c_char)) -> i32 {
    match crate::hosted(record) {
        Ok(never) => match never {},
        Err(why) => {
            tracing::error!(target: crate::trace::ENGINE, why, "cannot run");
            2
        }
    }
}
