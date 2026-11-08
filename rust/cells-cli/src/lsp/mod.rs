//! `cells lsp`: the language server, on stdin and stdout. A client of the same engine as
//! everything else: each request reads the file as the editor holds it, its imports
//! followed from the folder (a file the editor holds open read as it holds it), and
//! answers from the doc. Diagnostics follow every change, in every open file, since a
//! change in one may break a reference in another: a change the editor makes, and one
//! written to the disk by anything else (another editor, Cells.app, a script, the
//! engine storing what a change reached). The engine (`cells serve`) hears the disk and
//! says what changed: the server asks it to serve the folder the editor names, and each
//! open file's own, starting it when none answers.

mod colour;
mod features;
mod folder;
mod text;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use cells_daemon::Engine;
use cells_daemon::allowed::Allowed;
use tower_lsp::jsonrpc::{Error, Result};
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use folder::{File, Folder, canonical, spelling};
use text::Lines;

struct Backend {
    client: Client,
    folder: Arc<Mutex<Folder>>,
    /// The engine, once reached: it serves the editor's folders and says what the disk
    /// changed. Nothing when none could be reached, said to the editor once.
    engine: Mutex<Option<Arc<Engine>>>,
    /// Each open file's text as it was last saved (or opened): what formatting on save
    /// stores the edits since against.
    saved: Mutex<BTreeMap<PathBuf, String>>,
    /// Whether formatting aligns a table's columns: the client's `align` in its
    /// initialization options, on unless it says `false` (SPEC.md, Formatting).
    align: AtomicBool,
    /// Who this server is to the engine, the editor named (`Helix, through cells lsp`): what
    /// history says wrote a version this editor saved.
    name: Mutex<String>,
}

/// Every open file's diagnostics, sent again, its scripts held to the approvals as they
/// are now.
async fn diagnose(client: &Client, folder: Folder) {
    let allowed = Allowed::read();
    for path in folder.open.keys() {
        let Some(file) = folder.read(path) else { continue };
        client.publish_diagnostics(folder.uri(path), features::diagnostics(&file, &allowed), None).await;
    }
}

impl Backend {
    fn folder(&self) -> Folder {
        self.folder.lock().unwrap().clone()
    }

    fn change(&self, path: PathBuf, text: Option<String>) {
        let mut f = self.folder.lock().unwrap();
        let mut open = BTreeMap::clone(&f.open);
        match text {
            Some(t) => open.insert(path, t),
            None => open.remove(&path),
        };
        f.open = Arc::new(open);
    }

    /// The editor's spelling of `dir`, kept when it is not the canonical one.
    fn spell(&self, dir: &Path) {
        let Some((c, s)) = spelling(dir) else { return };
        let mut f = self.folder.lock().unwrap();
        let mut spelled = BTreeMap::clone(&f.spelled);
        spelled.insert(c, s);
        f.spelled = Arc::new(spelled);
    }

    async fn diagnose(&self) {
        diagnose(&self.client, self.folder()).await;
    }

    /// `folder` served by the engine, when one was reached; a refusal (a folder too wide
    /// to serve whole) said to the editor.
    async fn serve(&self, folder: PathBuf) {
        let Some(engine) = self.engine.lock().unwrap().clone() else { return };
        let answer = tokio::task::spawn_blocking(move || engine.serve(&folder)).await.expect("the engine's request never panics");
        if let Err(why) = answer {
            self.client.log_message(MessageType::WARNING, format!("cells: {why}")).await;
        }
    }

    /// The file a request names, read, and the byte its position names.
    fn at(&self, doc: &TextDocumentPositionParams) -> Option<(Folder, File, usize)> {
        let folder = self.folder();
        let file = folder.read(&path(&doc.text_document.uri)?)?;
        let at = Lines::new(&file.text).offset(doc.position);
        Some((folder, file, at))
    }
}

fn path(uri: &Url) -> Option<PathBuf> {
    uri.to_file_path().ok().map(|p| canonical(&p))
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        #[allow(deprecated)]
        let spelled = params
            .workspace_folders
            .as_ref()
            .and_then(|w| w.first())
            .map(|w| w.uri.clone())
            .or(params.root_uri)
            .and_then(|u| u.to_file_path().ok());
        if let Some(s) = &spelled {
            self.spell(s);
        }
        self.folder.lock().unwrap().root = spelled.map(|s| canonical(&s));
        let align = params.initialization_options.as_ref().and_then(|o| o.get("align")).and_then(|a| a.as_bool());
        self.align.store(align.unwrap_or(true), Ordering::Relaxed);
        if let Some(editor) = &params.client_info {
            let mut name = editor.name.clone();
            if let Some(first) = name.get_mut(..1) {
                first.make_ascii_uppercase();
            }
            *self.name.lock().unwrap() = format!("{name}, through cells lsp");
        }
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(TextDocumentSyncOptions {
                    open_close: Some(true),
                    change: Some(TextDocumentSyncKind::FULL),
                    save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions { include_text: Some(true) })),
                    ..TextDocumentSyncOptions::default()
                })),
                document_formatting_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".into(), "[".into(), ":".into(), ",".into()]),
                    ..CompletionOptions::default()
                }),
                definition_provider: Some(OneOf::Left(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: Vec::from([features::ALLOW.to_owned()]),
                    ..ExecuteCommandOptions::default()
                }),
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: Default::default(),
                })),
                inlay_hint_provider: Some(OneOf::Left(true)),
                semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(SemanticTokensOptions {
                    legend: colour::legend(),
                    full: Some(SemanticTokensFullOptions::Bool(true)),
                    ..SemanticTokensOptions::default()
                })),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo { name: "cells".into(), version: Some(env!("CARGO_PKG_VERSION").into()) }),
        })
    }

    /// The engine reached (started when none answers), told the folder the editor named and
    /// every file's open already, and what it says changed diagnosed from then on.
    async fn initialized(&self, _: InitializedParams) {
        let exe = std::env::current_exe().expect("the server knows its own binary");
        let name = self.name.lock().unwrap().clone();
        let reached = tokio::task::spawn_blocking(move || Engine::connect_or_start(&name, &exe)).await.expect("connecting never panics");
        let (engine, changed) = match reached {
            Ok(reached) => reached,
            Err(why) => {
                let said = format!("cells: no engine ({why}): a file another program writes shows here once this one is edited");
                self.client.log_message(MessageType::WARNING, said).await;
                return;
            }
        };
        *self.engine.lock().unwrap() = Some(Arc::new(engine));
        let (told, mut hearing) = tokio::sync::mpsc::unbounded_channel::<Vec<PathBuf>>();
        std::thread::spawn(move || {
            for paths in changed {
                if told.send(paths).is_err() {
                    return;
                }
            }
        });
        let (client, folder) = (self.client.clone(), Arc::clone(&self.folder));
        tokio::spawn(async move {
            while hearing.recv().await.is_some() {
                let held = folder.lock().unwrap().clone();
                diagnose(&client, held).await;
            }
        });
        let held = self.folder();
        // Each open file's own folder beside the editor's: an editor may name none (Helix,
        // where no project marker is), and the engine watches what a served file imports,
        // wherever it is.
        let folders: Vec<PathBuf> =
            held.root.iter().cloned().chain(held.open.keys().filter_map(|p| p.parent().map(Path::to_path_buf))).collect();
        for f in folders {
            self.serve(f).await;
        }
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, p: DidOpenTextDocumentParams) {
        if let Some(path) = path(&p.text_document.uri) {
            if let Some(dir) = p.text_document.uri.to_file_path().ok().as_deref().and_then(Path::parent) {
                self.spell(dir);
            }
            self.saved.lock().unwrap().insert(path.clone(), p.text_document.text.clone());
            self.change(path.clone(), Some(p.text_document.text));
            self.diagnose().await;
            if let Some(dir) = path.parent() {
                self.serve(dir.to_path_buf()).await;
            }
        }
    }

    /// A save: the text it wrote kept, and each script whose code the person changed in this
    /// editor since the last save allowed at the version they saved, since they wrote it. A
    /// script that arrives any other way (a file sent, cloned, copied in) never passes here,
    /// and waits for its person's yes.
    async fn did_save(&self, p: DidSaveTextDocumentParams) {
        let (Some(path), Some(text)) = (path(&p.text_document.uri), p.text) else { return };
        let before = self.saved.lock().unwrap().insert(path.clone(), text.clone());
        let Some(engine) = self.engine.lock().unwrap().clone() else { return };
        let folder = self.folder();
        let (Some(now), Some(then)) = (folder.read_as(&path, text), before.and_then(|t| folder.read_as(&path, t))) else { return };
        let written: Vec<(String, String)> = now
            .doc
            .scripts()
            .iter()
            .filter(|s| then.doc.scripts().iter().find(|t| t.name == s.name).is_none_or(|t| t.code != s.code || t.language != s.language))
            .map(|s| (s.name.clone(), cells_daemon::allowed::version(s)))
            .collect();
        if written.is_empty() {
            return;
        }
        let file = path.clone();
        let kept = tokio::task::spawn_blocking(move || {
            written.iter().map(|(name, v)| engine.allow(&file, name, v)).collect::<std::result::Result<Vec<()>, String>>()
        })
        .await
        .expect("allowing never panics");
        if let Err(why) = kept {
            self.client.log_message(MessageType::WARNING, format!("cells: {why}")).await;
        }
        self.diagnose().await;
    }

    /// The one command a fix runs: a script allowed at the version the editor showed.
    async fn execute_command(&self, p: ExecuteCommandParams) -> Result<Option<LSPAny>> {
        assert_eq!(p.command, features::ALLOW, "the server offers no other command");
        let text = |i: usize| p.arguments.get(i).and_then(|a| a.as_str()).map(str::to_owned);
        let (Some(file), Some(script), Some(version)) = (text(0), text(1), text(2)) else {
            return Err(Error::invalid_params("cells.allow takes a file, a script and its version"));
        };
        let Some(engine) = self.engine.lock().unwrap().clone() else {
            return Err(Error::invalid_params("no engine to keep the approval: `cells allow` in a terminal"));
        };
        tokio::task::spawn_blocking(move || engine.allow(Path::new(&file), &script, &version))
            .await
            .expect("allowing never panics")
            .map_err(Error::invalid_params)?;
        self.diagnose().await;
        Ok(None)
    }

    async fn formatting(&self, p: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let folder = self.folder();
        let Some(path) = path(&p.text_document.uri) else { return Ok(None) };
        let Some(file) = folder.read(&path) else { return Ok(None) };
        let saved = self.saved.lock().unwrap().get(&path).cloned();
        let saved = saved.and_then(|t| folder.read_as(&path, t));
        Ok(features::formatted(&file, saved.as_ref(), self.align.load(Ordering::Relaxed)).map(|e| Vec::from([e])))
    }

    /// An edit: the engine told the text this editor now holds, so the version it saves as
    /// is logged as this editor's.
    async fn did_change(&self, p: DidChangeTextDocumentParams) {
        // Full sync: the last change is the whole text.
        if let (Some(path), Some(c)) = (path(&p.text_document.uri), p.content_changes.into_iter().last()) {
            let engine = self.engine.lock().unwrap().clone();
            if let Some(Err(why)) = engine.map(|e| e.editing(&path, &c.text)) {
                self.client.log_message(MessageType::WARNING, format!("cells: the engine did not hear the edit: {why}")).await;
            }
            self.change(path, Some(c.text));
            self.diagnose().await;
        }
    }

    async fn did_close(&self, p: DidCloseTextDocumentParams) {
        if let Some(path) = path(&p.text_document.uri) {
            self.saved.lock().unwrap().remove(&path);
            self.change(path, None);
            self.client.publish_diagnostics(p.text_document.uri, Vec::new(), None).await;
            self.diagnose().await;
        }
    }

    async fn hover(&self, p: HoverParams) -> Result<Option<Hover>> {
        Ok(self.at(&p.text_document_position_params).and_then(|(folder, file, at)| features::hover(&folder, &file, at)))
    }

    async fn completion(&self, p: CompletionParams) -> Result<Option<CompletionResponse>> {
        let Some((folder, file, at)) = self.at(&p.text_document_position) else { return Ok(None) };
        let items = features::complete(&folder, &file, at);
        Ok((!items.is_empty()).then_some(CompletionResponse::Array(items)))
    }

    async fn goto_definition(&self, p: GotoDefinitionParams) -> Result<Option<GotoDefinitionResponse>> {
        let Some((folder, file, at)) = self.at(&p.text_document_position_params) else { return Ok(None) };
        Ok(features::definition(&folder, &file, at).map(GotoDefinitionResponse::Scalar))
    }

    async fn code_action(&self, p: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let folder = self.folder();
        let Some(file) = path(&p.text_document.uri).and_then(|path| folder.read(&path)) else { return Ok(None) };
        let actions = features::actions(&folder, &file, p.range, &Allowed::read());
        Ok((!actions.is_empty()).then(|| actions.into_iter().map(CodeActionOrCommand::CodeAction).collect()))
    }

    async fn semantic_tokens_full(&self, p: SemanticTokensParams) -> Result<Option<SemanticTokensResult>> {
        let folder = self.folder();
        let Some(file) = path(&p.text_document.uri).and_then(|path| folder.read(&path)) else { return Ok(None) };
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens { result_id: None, data: colour::semantic_tokens(&file.text) })))
    }

    async fn inlay_hint(&self, p: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let folder = self.folder();
        let Some(file) = path(&p.text_document.uri).and_then(|path| folder.read(&path)) else { return Ok(None) };
        Ok(Some(colour::aligned(&file.text, p.range)))
    }

    async fn prepare_rename(&self, p: TextDocumentPositionParams) -> Result<Option<PrepareRenameResponse>> {
        let Some((_, file, at)) = self.at(&p) else { return Ok(None) };
        let lines = Lines::new(&file.text);
        Ok(features::renamed(&file, at).map(|r| match r {
            features::Renamed::Column(_, s) | features::Renamed::Key(_, _, s) => PrepareRenameResponse::Range(lines.range(s)),
        }))
    }

    async fn rename(&self, p: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let Some((folder, file, at)) = self.at(&p.text_document_position) else { return Ok(None) };
        features::rename(&folder, &file, at, &p.new_name).map(Some).map_err(Error::invalid_params)
    }
}

/// Serves the protocol on stdin and stdout until the client exits.
pub fn serve() {
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("a runtime");
    runtime.block_on(async {
        let folder = Arc::new(Mutex::new(Folder { root: None, open: Arc::new(BTreeMap::new()), spelled: Arc::new(BTreeMap::new()) }));
        let (service, socket) = LspService::new(|client| Backend {
            client,
            folder,
            engine: Mutex::new(None),
            saved: Mutex::new(BTreeMap::new()),
            align: AtomicBool::new(true),
            name: Mutex::new("an editor, through cells lsp".into()),
        });
        Server::new(tokio::io::stdin(), tokio::io::stdout(), socket).serve(service).await;
    });
}
