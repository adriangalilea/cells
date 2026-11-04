# The engine

> **Not yet final**, as SPEC.md. This is how the engine is designed; what is built is said in README.md.

The files are the truth. The engine is a typed, current, instant view of them, and the one place a client reads and writes through. It is to a folder of `.tcsv` files what git's index is to a working tree, or Spotlight's to a disk: derived, disposable, rebuilt from the text, and never in the way of anyone who edits the text without it.

## One engine, two ways to run

- **Standalone, in the client.** A page given a `.tcsv` file (the engine as wasm), an app with a file and no daemon: read, render, edit, give it back. Nothing else is needed, and a file is complete on its own.
- **Connected: the daemon.** On a Mac, one process per user (`tcsv serve`, kept by launchd) owns every folder its clients open: each file parsed once, its references held as a graph, every query and chart served from memory. Every client we build speaks to it: tally, the language server, the command line. On iOS there is no process to share, so the app runs the same engine in-process, with the same cache and index, watching its folder through `NSFilePresenter`; the client's code is the same.

## The protocol

Ours, and one: intents and queries in, results and changes out. Results are Arrow IPC frames (typed columns, read by the client as they arrive), commands small messages. It runs over any byte stream:

| Client | Carrier |
|---|---|
| an iOS app | none: in-process calls |
| a Mac app, the language server, the command line | XPC or a Unix socket, reachable only by the user's processes, the client's signature checked |
| a page | a WebSocket |
| shared books on a server (later) | the same, over the network |

A WebSocket to `localhost` is open to every page the browser visits (no CORS protects it), so a client is paired once, as a device is, and every connection proves the pairing; the daemon also checks the connection's origin. Arrow Flight was the alternative and is not taken: it needs gRPC, which no browser speaks without a proxy.

## Reading

- **Parsed once, kept on disk.** Each file's columns are cached as Arrow, memory-mapped, keyed by the file's identity (device, inode, size, modification time, content hash), so a restart parses nothing and the kernel decides what stays in memory.
- **Only what changed is read again.** A file that grew with its start unchanged (its prefix hash the same: a ledger appended to) has only its tail parsed; any other change reparses that one file, at memory speed.
- **Changes are heard, then trusted where they can be.** FSEvents tells the daemon what changed; on a local volume it is trusted, a dropped batch rescanning only the folder it names, and after a restart the daemon asks for every event since the last one it saw. On a network or external volume, where FSEvents is unreliable, the files a query touches are checked by size and time before it is answered. Either way, the next query serves what is on disk.
- **A query sees one moment.** It runs on a snapshot: a file changed while it runs is in the next query, never half in this one.
- **Half-written is pending.** A record cut off at a file's end (a writer caught mid-write) is pending, not a problem, and read again once the file is still.
- **Queries are DuckDB's** over the cached columns, with nothing copied: SQL, and SQL/PGQ for the graph.
- **Clients subscribe.** A query a client keeps (a chart, a table) is answered again when a file it reads changes, and the client gets the new result: charts and tables are live.

## Writing

- **An edit is an intent addressed by identity**: the row `k7m2q9xa`, the column `amount`, the value `1000`. Never a row's position, which a line inserted elsewhere moves. The engine applies intents deterministically and keeps them in a log.
- **Ids are the engine's.** No intent writes an `id`; a client built on the protocol cannot show or change one. Rows without one get theirs at the engine's next write (SPEC.md, "Every row has an identity").
- **Never lose anyone's write.** Locks on a Mac are advisory, honoured only by programs that cooperate, so the engine detects rather than prevents:
  - the write is atomic: the new text written beside the file, then swapped into place (`FileManager.replaceItemAt` on Apple platforms, which keeps the file's metadata), so no one ever reads half of it;
  - it is coordinated: `NSFileCoordinator`, so Finder, iCloud Drive and other apps wait the milliseconds it takes;
  - it is compare-and-swap: just before the swap, the file must still be the version the intent was made against. If someone changed it, the intent is applied to what is there now when its row and cell are untouched, and otherwise refused, the conflict shown to the person.
- **Several files, one intent.** A change that touches many files (an account renamed in every file that references it) is journaled first, applied file by file, marked done, and replayed after a crash. Intents are safe to apply twice.

## Editors

The language server is a client like any other, so an editor goes through the engine too:

- types checked as typed, references completed and followed, a rename offered across the folder;
- the id column dimmed, an id edited by hand warned about, the id column removed refused;
- ids filled in on save, through the editor's format-on-save, so a row added by hand gets its id inside the person's own save, never by a write behind them;
- unsaved edits sent to the daemon as they are typed, so another client shows the file as being edited, and a conflict shows before either side saves.

An editor cannot hide part of a line, so in one the ids are visible, dimmed and guarded; in every app built on the engine they are never shown.

## Across devices

Every device is a full replica, never a viewer of another:

- **The files are the replica.** They sync as files (iCloud Drive, git, Syncthing), so each device holds every file it shows, as text, and whatever moves files moves the books.
- **Each device's engine keeps its own cache** and rebuilds it from its own files. Nothing derived ever crosses devices.
- **Reads and queries are local and instant**, and a device reads, queries and writes with every other one off: the phone opens the books on a plane with the Mac asleep.
- **What is local stays local.** iOS may evict an iCloud Drive file to save space; the engine asks for the download of what it opens, and a folder kept downloaded never leaves the device.

How devices reconcile writes is open, and the write path serves both answers:

- **Owned.** One device owns a set of books. Another records an edit as an intent, shows it at once as pending, and the owner applies it when it is in reach, refusing one that no longer applies. Money never conflicts. This is Turso's embedded replicas: a local copy read in place, writes sent to the primary.
- **Merged.** Each device writes its copy, and two versions of a file merge by row identity against the last version both had: different rows and different cells merge, and only two edits of the same cell are a conflict, shown, never resolved by guessing. This is Turso's offline writes, and Dolt's merge of tables by primary key: applied locally first, reconciled when the devices meet.

Prior art for the whole shape: Ink & Switch's [Local-first software](https://www.inkandswitch.com/local-first/) (the data on the person's devices, the network optional), Turso's replicas, Dolt, and CRDT libraries such as Automerge, which merge without asking but cannot say that two edits to an amount are a conflict, so the engine does not take them for money.

Both rest on what is built now: edits as intents by row identity, kept in a log.

## Proving it

Deterministic simulation testing, as TigerBeetle does: the engine run against a simulated filesystem and clock that inject crashes mid-write, torn writes, dropped and reordered events, two writers at once and a full disk, every run from a seed so any failure replays exactly. Assertions on every assumption, as TigerStyle and this studio both require.
