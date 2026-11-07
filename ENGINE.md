# The engine

> **Not yet final**, as SPEC.md. This is how the engine is designed: [v0](#v0-one-mac) is what is being built, [Later](#later) the direction it is built to grow into. What exists is said in README.md.

The files are the truth. The engine is a typed, current, instant view of them, and the one place a client reads and writes through. It is to a folder of `.cells` files what git's index is to a working tree, or Spotlight's to a disk: derived, rebuilt from the text, and never in the way of anyone who edits the text without it.

The engine is a library: the Rust core, behind a C header on the Apple platforms and as wasm in a page.

## v0: one Mac

### How it runs

- **The daemon.** One process per user, `cells serve`, kept by launchd, hosts the engine for every client on the Mac: Cells.app, the language server, the command line. Each file is parsed once, its references are held as a graph, and every query and chart is served from memory. It runs each file's scripts on their schedules (SPEC.md, "Running"; `cells run` runs one by hand today), through Bun, and only there: a page or an app shows what the last run stored.
- **Three parts, each owning one thing.** `cells::folder`, in the core, decides: the folder's files, the graph of what each reads through its imports, the version of each it last knew, and from a change handed in, what to read again, what to store and which files it still needs. It reaches no disk, clock or thread, so its decisions replay exactly from the changes handed in. `cells-daemon`'s `World` is the one way to the disk, the clock and change events. Its `Daemon` joins the two: what was heard is handed in, and what was decided is carried out, logged and said. Standalone, an app or the language server holds the same `Folder` in its own process; with the daemon, it asks the daemon.
- **Standalone.** A page given a `.cells` file (the wasm engine), or an app given a file without the daemon: read, render, edit, give it back. Nothing else is needed, and a file is complete on its own.

### The protocol

Intents and queries in, results and changes out. Results are Arrow IPC frames (typed columns, read by the client as they arrive); commands are small messages, an intent's body the same JSON as its line in the log. Clients reach the daemon over a Unix socket, reachable only by the user's processes, the client's signature checked.

Built today (`cells-daemon`'s `wire`, `server`, `client`): one JSON object per line over `Cells/engine.sock` in the user's application support folder, so a conversation reads with `nc -U`. The socket is made as Authz makes its own: the engine's folder private, a lock saying which process is the engine, the socket bound under a name of its own, made `0600`, listening, then renamed into place, so its name answers the moment it exists. A client greets with the protocol's version and its name, and a different version parts there; it asks for folders (`serve`) and hears every file whose version changed (`changed`), the engine's own writes included. Who connects is the kernel's word (`getpeereid`): the same user. The code signature check (the engine's own team and Cells' identifiers, an Apple Development leaf only in development) arrives with the signed bundle, since a development build has no signature to hold a client to. A client that finds nothing answering starts the engine (`cells serve`, detached, its trace in `engine.log` beside the socket), as the first client of the day does; once the engine ships inside Cells.app, the app keeps it running (swift-utils' `AppInstallations`) and a client only finds it. The language server is its first client: it asks for the folder the editor names and each open file's own, and diagnoses again on every change the engine tells, watching nothing itself.

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
- **A change stores what it reaches, in every file** (SPEC.md, Writing): the changed file and every file reading it are read again in the order their reads need, and each is written as a write after the version the engine knew leaves it, so a computed cell whose formula now gives another value is stored, and a value typed over its `=` (its formula giving what it gave) is left stale for a person. The engine's write coming back is its own version, read alone: it stores what the files reading it already read. A file is written only over the version the write was made from; one changed since is left as its writer saved it (a `moved` line), and that change is read next and stores what it reaches.
- **An edit the engine did not make is logged too.** When the engine finds a file whose hash is not the last result it logged, it records an `external` entry (the old hash, the new one, which rows changed), so the log explains every version of every file, an editor's included.
- **Typing is not an intent.** An editor's unsaved text is presence, sent to the daemon but never logged; saves and app actions are intents.
- **Rules are checked before a write**: keys, `unique` columns, every `check` (SPEC.md, Open). An intent that would break one is refused with the reason.
- **A run is an intent.** What a script returned is checked against its table's header, then written as one intent (its rows, its entry in `runs`), journalled and applied like any edit, so a crash mid-run leaves the last run's rows whole. A run that fails writes only its failure. The script's stdout and stderr, its duration and outcome go in the log with the intent.

### The log

`.cells/log.jsonl` in the folder, one line per entry: its number, the SHA-256 of the line before (the engine's own hash, so the core keeps no dependency; an edit or a removal breaks every later link), the intent id, its kind (`set`, `add`, `delete`, `rename`, `run`, `external`, `store`, `moved`, `done`), its arguments, for each file the hash it was made against and the hash it produced, its assigned ids and timestamp. Past a size, it continues in a new segment linked to the last. Written today: `external` (a version the engine did not write: the hash before, `null` for a new file, and now, `null` once removed), `store` (a write the engine made of what a change reached: which file's change, the version it was written over and the one written, each cell stored with its value before and after), and `moved` (a write not made, its file changed since). A `store` is logged once its file is written, not journalled before: it follows from the versions logged before it, so a crash between the two loses only a line saying what the files already show.

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
- **The merge**, for what the engine did not write (two hand edits on two devices, a git merge, a conflict version iCloud Drive or Syncthing kept): three ways by row identity, cells compared by typed value, deletions against edits and additions on both sides as conflicts, row order never one, the result checked against the books' rules. Conflicts are a table beside the file (`.cells/conflicts/<file>.cells`), never markers in it, so the file always reads; never resolved by guessing. No file sync hands over a common ancestor (only git does), so each device keeps a store of file versions by hash and finds the ancestor through the logs. git calls the engine as its merge driver. This is Dolt's merge of tables by primary key.
- **The log per device**: one chain per device per folder, signed by the device's key and anchored in its keychain, so a chain deleted or replaced is found, and a merge's parents are known.
- **Pages connected**: a page reaching the engine over a WebSocket, paired once, its origin checked, since every page the browser visits can reach `localhost`.
- **Replicated**: devices agreeing on the order of the intent log (Viewstamped Replication, as TigerBeetle does), for books shared by several people where two writers must never both succeed. It needs an always-on replica: with two devices, consensus needs both awake for every write.

Prior art for the whole shape: Ink & Switch's [Local-first software](https://www.inkandswitch.com/local-first/) (the data on the person's devices, the network optional), Turso's replicas, Dolt, and CRDT libraries such as Automerge, which merge without asking but cannot say that two edits to an amount are a conflict, so the engine does not take them for money.
