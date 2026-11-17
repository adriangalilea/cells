// The Cells engine as a host app runs it (rust/cells-daemon, `hosted.rs`): the engine, and
// what the host hears of the person's files handed to it.
#pragma once

#include <stdint.h>

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

// The last FSEvents event ID the engine kept as handled, where the host's streams start
// (FSEvents replays what happened since): 0 when it kept none, and then the host hears from
// now on and the engine's walk at its start finds what changed before.
uint64_t cells_engine_heard_since(void);

// One batch the host heard, in the order FSEvents gave it, its text valid only during the
// call; callable from any thread from before `cells_engine_serve` (a batch waits until the
// engine runs). `file` is the system's file ID (absent on a stream's own events), `flags`
// FSEvents' flags in words: created removed renamed modified meta xattr cloned file dir
// symlink hardlink own scan dropped wrapped history-done root-changed mount unmount.
//   {"events":[{"n":412,"path":"/Users/a/x.cells","file":8812,"flags":["renamed","file"]}]}
// A batch that is none ends the process saying why: the host and the engine disagree.
void cells_engine_heard(const char *json);
