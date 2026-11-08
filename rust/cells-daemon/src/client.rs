//! A client of the engine: the language server, the command line, an app. It reaches the
//! engine over its socket (`wire`), or starts it when nothing answers, as the first client
//! of the day does; once the engine ships inside Cells.app, the app keeps it running and a
//! client only ever finds it.

use std::fs::OpenOptions;
use std::io::BufReader;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::{Duration, Instant};

use crate::wire::{self, FromEngine, ToEngine, VERSION};

pub struct Engine {
    writer: Mutex<UnixStream>,
    /// The engine's answers to requests (`served`, `allowed`, `refused`), in the order they
    /// were asked.
    answers: Mutex<Receiver<FromEngine>>,
    /// The engine's process.
    pub pid: u32,
}

/// How long a client started an engine waits for its socket: its first folders are read
/// after the socket answers, so this is the start of a process, never a folder's size.
const START: Duration = Duration::from_secs(5);

impl Engine {
    /// The running engine, greeted as `client`; with it, every file whose version changes
    /// from now on, as the engine tells them.
    pub fn connect(client: &str) -> Result<(Engine, Receiver<Vec<PathBuf>>), String> {
        let socket = wire::socket();
        let stream = UnixStream::connect(&socket).map_err(|e| format!("{}: {e}", socket.display()))?;
        let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
        let mut writer = stream;
        wire::send(&mut writer, &ToEngine::Hello { version: VERSION, client: client.to_owned() }).map_err(|e| e.to_string())?;
        let pid = match wire::receive(&mut reader).map_err(|e| e.to_string())? {
            Some(FromEngine::Hello { version, pid }) if version == VERSION => pid,
            Some(FromEngine::Refused { why }) => return Err(why),
            Some(other) => return Err(format!("the engine answered the greeting with {other:?}")),
            None => return Err("the engine closed at the greeting".into()),
        };
        let (changes, changed) = channel();
        let (answer, answers) = channel();
        thread::Builder::new().name("cells-engine".into()).spawn(move || hear(reader, changes, answer)).map_err(|e| e.to_string())?;
        Ok((Engine { writer: Mutex::new(writer), answers: Mutex::new(answers), pid }, changed))
    }

    /// The running engine, or one started from `exe` (a `cells` binary) when nothing
    /// answers: detached, in a process group of its own, its trace written to `engine.log`
    /// in the engine's folder. Two clients starting one at once start two; the second finds
    /// the first holding the engine's lock and exits, and both clients reach the first.
    pub fn connect_or_start(client: &str, exe: &Path) -> Result<(Engine, Receiver<Vec<PathBuf>>), String> {
        if let Ok(engine) = Engine::connect(client) {
            return Ok(engine);
        }
        let folder = wire::folder();
        std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        let log_path = folder.join("engine.log");
        let log = OpenOptions::new().create(true).append(true).open(&log_path).map_err(|e| format!("{}: {e}", log_path.display()))?;
        let err = log.try_clone().map_err(|e| e.to_string())?;
        Command::new(exe)
            .arg("serve")
            .stdin(Stdio::null())
            .stdout(log)
            .stderr(err)
            .process_group(0)
            .spawn()
            .map_err(|e| format!("{} serve: {e}", exe.display()))?;
        let began = Instant::now();
        loop {
            match Engine::connect(client) {
                Ok(engine) => return Ok(engine),
                Err(e) if began.elapsed() > START => {
                    return Err(format!(
                        "started {} serve, and it did not answer in {START:?}: {e} (its trace: {})",
                        exe.display(),
                        log_path.display()
                    ));
                }
                Err(_) => thread::sleep(Duration::from_millis(20)),
            }
        }
    }

    /// `folder` served by the engine from now on: the folder served (it, or the folder
    /// served already holding it), or why the engine refused.
    pub fn serve(&self, folder: &Path) -> Result<PathBuf, String> {
        match self.ask(&ToEngine::Serve { folder: folder.to_path_buf() })? {
            FromEngine::Served { folder } => Ok(folder),
            other => panic!("the engine answered a request to serve with {other:?}"),
        }
    }

    /// `script` of the file at `file` allowed to run at `version` (`allowed::version`), the
    /// one its person was just shown: sent only after their yes.
    pub fn allow(&self, file: &Path, script: &str, version: &str) -> Result<(), String> {
        let asked = ToEngine::Allow { file: file.to_path_buf(), script: script.to_owned(), version: version.to_owned() };
        match self.ask(&asked)? {
            FromEngine::Allowed { .. } => Ok(()),
            other => panic!("the engine answered a request to allow with {other:?}"),
        }
    }

    /// `request` sent and its answer read; a refusal is the error.
    fn ask(&self, request: &ToEngine) -> Result<FromEngine, String> {
        // one request answered at a time, so each answer is its request's
        let answers = self.answers.lock().unwrap();
        wire::send(&mut *self.writer.lock().unwrap(), request).map_err(|e| e.to_string())?;
        match answers.recv().map_err(|_| "the engine closed".to_owned())? {
            FromEngine::Refused { why } => Err(why),
            answer => Ok(answer),
        }
    }
}

/// What the engine says, sorted: changes to those listening for them, answers to whoever
/// asked. Ends when the engine closes, which ends both.
fn hear(mut reader: BufReader<UnixStream>, changes: Sender<Vec<PathBuf>>, answer: Sender<FromEngine>) {
    loop {
        match wire::receive(&mut reader) {
            Ok(Some(FromEngine::Changed { paths })) => {
                if changes.send(paths).is_err() {
                    return;
                }
            }
            Ok(Some(said @ (FromEngine::Served { .. } | FromEngine::Allowed { .. } | FromEngine::Refused { .. }))) => {
                let _ = answer.send(said);
            }
            Ok(Some(FromEngine::Hello { .. })) => {
                eprintln!("cells: the engine greeted twice");
                return;
            }
            Ok(None) => return,
            Err(e) => {
                eprintln!("cells: the engine said something unreadable: {e}");
                return;
            }
        }
    }
}
