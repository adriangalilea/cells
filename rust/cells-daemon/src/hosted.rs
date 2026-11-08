//! The engine as a host app runs it: one C function, `cells_engine_serve`, declared in
//! `include/cells_engine.h`. Cells.app's `CellsEngine.app` is a Swift `main` that watches its
//! own image (swift-utils' `AppInstallations`, which every process of an app links) and calls
//! this, so Swift owns the app's life on the Mac and Rust owns the engine, its folder and its
//! log included.

/// The engine, until the process ends; returns only when it cannot run, its reason said on
/// stderr, with the exit status a launchd job should end with.
#[unsafe(no_mangle)]
pub extern "C" fn cells_engine_serve() -> i32 {
    match crate::hosted() {
        Ok(never) => match never {},
        Err(why) => {
            eprintln!("cells engine: {why}");
            2
        }
    }
}
