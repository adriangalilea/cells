// The Cells engine as a host app runs it (rust/cells-daemon, `hosted.rs`): one function.
#pragma once

// What the engine says, one JSON object per call, its text valid only during the call
// (rust/cells-daemon, `trace.rs`):
//   {"phase":"start","span":7,"parent":3,"level":"info","scope":["engine"],"message":"change","fields":{...}}
//   {"phase":"end","span":7}
//   {"span":7,"level":"info","scope":["engine"],"message":"stored","fields":{...}}
typedef void (*cells_engine_record)(const char *json);

// The engine over its socket until the process ends, waiting for the engine's lock when
// another process holds it, every record handed to `record`. Returns only when it cannot
// run, its reason handed to `record`, with the exit status the process should end with.
int cells_engine_serve(cells_engine_record record);
