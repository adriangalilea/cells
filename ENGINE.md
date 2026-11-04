# The engine

> **Not yet final**, as SPEC.md. This is how the engine is designed: [v0](#v0-one-mac) is what is being built, [Later](#later) the direction it is built to grow into. What exists is said in README.md.

The files are the truth. The engine is a typed, current, instant view of them, and the one place a client reads and writes through. It is to a folder of `.tcsv` files what git's index is to a working tree, or Spotlight's to a disk: derived, rebuilt from the text, and never in the way of anyone who edits the text without it.

The engine is a library: the Rust core, behind a C header on the Apple platforms and as wasm in a page.

## v0: one Mac

### How it runs

- **The daemon.** One process per user, `tcsv serve`, kept by launchd, hosts the engine for every client on the Mac: tally, the language server, the command line. Each file is parsed once, its references are held as a graph, and every query and chart is served from memory.
- **Standalone.** A page given a `.tcsv` file (the wasm engine), or an app given a file without the daemon: read, render, edit, give it back. Nothing else is needed, and a file is complete on its own.

### The protocol

Intents and queries in, results and changes out. Results are Arrow IPC frames (typed columns, read by the client as they arrive); commands are small messages, an intent's body the same JSON as its line in the log. Clients reach the daemon over XPC or a Unix socket, reachable only by the user's processes, the client's signature checked.

### Reading

- **Parsed once, kept on disk.** Each file's columns are cached as Arrow, memory-mapped, keyed by the file's identity (device, inode, size, modification time) and its content hash, so a restart parses nothing and the kernel decides what stays in memory. Size and time alone never decide: two writes in one tick, or a clock stepped back, leave both equal over different bytes, so the hash does.
- **Only what changed is read again.** A file that grew with its start unchanged (its prefix hash the same: a ledger appended to) has only its tail parsed; any other change reparses that one file, at memory speed.
- **Changes are heard.** FSEvents says what changed; on a local volume it is trusted, a dropped batch rescanning only the folder it names, and after a restart the daemon asks for every event since the last one it saw. On a network or external volume, where FSEvents is unreliable, the files a query touches are checked before it is answered. Events are batched and handled at the engine's own pace. Either way, the next query serves what is on disk.
- **A query sees one moment.** It runs on a snapshot: a file changed while it runs is in the next query, never half in this one.
- **Half-written is pending.** A record cut off at a file's end (a writer caught mid-write) is pending, not a problem, and read again once the file is still.
- **Queries are DuckDB's** over the cached columns, with nothing copied: SQL, and SQL/PGQ for the graph.
- **Clients subscribe.** A query a client keeps (a chart, a table) is answered again when a file it reads changes: charts and tables are live. A subscriber too slow to keep up is told it lagged and reads again, never queued without bound.

### Writing

- **An edit is an intent addressed by identity**: the row `k7m2q9xa`, the column `amount`, the value `1000`. Never a row's position, which a line inserted elsewhere moves.
- **Ids are the engine's.** No intent writes an `id`; a client built on the protocol cannot show or change one. Rows without one get theirs at the engine's next write (SPEC.md, "Every row has an identity").
- **Every intent is idempotent by its own id**, eight base-32 characters made by the client: applying it a second time changes nothing and answers the first result.
- **Every intent names what it was made against**: for each file it touches, the content hash it read.
- **Applying is deterministic.** Nothing an intent applies reads a clock, a random source or the environment: its new row ids and its timestamp are assigned when it is accepted and written into it, so a replay gives the same bytes.
- **Journal, then apply.** The accepted intent is appended to the log and flushed; then each file is written: the new text beside the file, flushed, swapped into place inside an `NSFileCoordinator` write (`FileManager.replaceItemAt`, which keeps the file's metadata); then a `done` entry names each resulting hash. A crash between the two replays from the log, and a replay over a file already written is a no-op. A change across many files (an account renamed in every file that references it) is one intent, the same way.
- **Never lose anyone's write.** Locks on a Mac are advisory, honoured only by programs that cooperate, so the engine detects rather than prevents. Just before the swap, the file must still be the version the intent was made against. If someone changed it, the intent is applied to what is there now when its rows and cells are untouched, and otherwise refused, the conflict shown to the person, never resolved by guessing.
- **An edit the engine did not make is logged too.** When the engine finds a file whose hash is not the last result it logged, it records an `external` entry (the old hash, the new one, which rows changed), so the log explains every version of every file, an editor's included.
- **Typing is not an intent.** An editor's unsaved text is presence, sent to the daemon but never logged; saves and app actions are intents.
- **Rules are checked before a write**: keys, `unique` columns, every `check` (SPEC.md, Open). An intent that would break one is refused with the reason.

### The log

`.tcsv/log.jsonl` in the folder, one line per entry: its number, the BLAKE3 hash of the line before (so an edit or a removal breaks every later link), the intent id, its kind (`set`, `add`, `delete`, `rename`, `external`, `done`), its arguments, for each file the hash it was made against and the hash it produced, its assigned ids and timestamp. Past a size, it continues in a new segment linked to the last.

### Editors

The language server is a client like any other, so an editor goes through the engine too:

- types checked as typed, references completed and followed, a rename offered across the folder;
- the id column dimmed, an id edited by hand warned about, the id column removed refused;
- ids filled in on save, through the editor's format-on-save, so a row added by hand gets its id inside the person's own save, never by a write behind them;
- unsaved edits sent to the daemon as they are typed, so another client shows the file as being edited, and a conflict shows before either side saves.

An editor cannot hide part of a line, so in one the ids are visible, dimmed and guarded; in every app built on the engine they are never shown.

### Proving it

Deterministic simulation testing, as TigerBeetle's VOPR does: the real engine against a simulated world, every run from one seed, so any failure replays exactly from the seed and the commit.

- **One way to the world.** The engine reaches the filesystem, the clock, change events and coordination through one trait; production and simulation differ in that one implementation. In the core, real clocks, threads, the real filesystem and `HashMap` iteration order are banned (a lint enforces it).
- **The world simulated**: the daemon and its clients, and editors that ignore coordination; durability only after a flush, renames atomic but durable only once their folder is flushed; a clock that drifts and jumps, so size and time lie; FSEvents delayed, coalesced, reordered and dropped.
- **What it injects**, each probability drawn from the seed: a crash at any call into the world, torn writes, lost unflushed writes, a full disk and I/O errors, bit rot in the cache, an editor truncating, appending, inserting, reordering, reformatting, pasting a row twice or deleting the id column, two writers at once.
- **One seed, many streams**: the seed draws the world's shape and one sub-seed per subsystem, so how often one subsystem draws never shifts another's.
- **What it checks**, beside assertions on every assumption: no acknowledged write lost (it is in the files or was refused with the reason given); never half a file read; the cache equal to a fresh parse before every query; untouched rows keeping their bytes; a log replayed any number of times giving the same files; the books' rules holding in every version written.
- **Exhaustive where it is small**: every crash point of one write.

Assertions stay on in release builds, as TigerStyle says: a confused engine stops rather than writing books wrong.

## Later

The direction v0 is built so it never needs a rewrite to reach. None of it is being built now.

### Other devices

- **Every device a full replica.** The files sync as files (iCloud Drive, git, Syncthing); each device's engine keeps its own cache from its own files; reads, queries and records work with every other device off. On iOS the app hosts the engine itself (no daemon exists there), watching its folder through `NSFilePresenter`, asking iCloud Drive to download what it opens.
- **Books owned.** A folder names one device whose engine applies intents to it. Another device records an intent, shows it at once marked pending, and sends it to the owner, which applies it when it is in reach after checking every rule, or refuses it with the reason. The reason is money: an invoice once, a balance that must hold, a limit, a pending payment settled once are rules over many rows, and no merge of cells keeps them (two devices each recording a valid 50 from an available 80 merge into an overdraft with no cell in conflict). This is Turso's embedded replicas, the replica writing ahead of its primary as pending.
- **The transport** between devices: a mesh where devices dial keys, not addresses, end to end encrypted, paired once, with an outbox holding intents for a device out of reach. tether, the studio's own, is the candidate.
- **The merge**, for what the engine did not write (two hand edits on two devices, a git merge, a conflict version iCloud Drive or Syncthing kept): three ways by row identity, cells compared by typed value, deletions against edits and additions on both sides as conflicts, row order never one, the result checked against the books' rules. Conflicts are a table beside the file (`.tcsv/conflicts/<file>.tcsv`), never markers in it, so the file always reads; never resolved by guessing. No file sync hands over a common ancestor (only git does), so each device keeps a store of file versions by hash and finds the ancestor through the logs. git calls the engine as its merge driver. This is Dolt's merge of tables by primary key.
- **The log per device**: one chain per device per folder, signed by the device's key and anchored in its keychain, so a chain deleted or replaced is found, and a merge's parents are known.
- **Pages connected**: a page reaching the engine over a WebSocket, paired once, its origin checked, since every page the browser visits can reach `localhost`.
- **Replicated**: devices agreeing on the order of the intent log (Viewstamped Replication, as TigerBeetle does), for books shared by several people where two writers must never both succeed. It needs an always-on replica: with two devices, consensus needs both awake for every write.

Prior art for the whole shape: Ink & Switch's [Local-first software](https://www.inkandswitch.com/local-first/) (the data on the person's devices, the network optional), Turso's replicas, Dolt, and CRDT libraries such as Automerge, which merge without asking but cannot say that two edits to an amount are a conflict, so the engine does not take them for money.
