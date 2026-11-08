// The Cells engine as a host app runs it (rust/cells-daemon, `hosted.rs`): one function.
#pragma once

// The engine over its socket until the process ends, waiting for the engine's lock when
// another process holds it. Returns only when it cannot run, its reason said on stderr,
// with the exit status the process should end with.
int cells_engine_serve(void);
