//! `cells lsp`: the language server, on stdin and stdout. A client of the same engine as
//! everything else: each request reads the file as the editor holds it, its imports
//! followed from the folder (a file the editor holds open read as it holds it), and
//! answers from the doc. Diagnostics follow every change, in every open file, since a
//! change in one may break a reference in another: a change the editor makes, and one
//! written to the disk by anything else (another editor, Cells.app, a script), which the
//! server hears by watching the folder itself, never by asking the editor.

mod colour;
mod features;
mod folder;
mod text;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tower_lsp::jsonrpc::{Error, Result};
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use folder::{File, Folder, canonical, spelling};
use text::Lines;

struct Backend {
    client: Client,
    folder: Arc<Mutex<Folder>>,
    /// The directories watched for what is written to the disk.
    watch: Arc<Mutex<Watch>>,
    /// Each open file's text as it was last saved (or opened): what formatting on save
    /// stores the edits since against.
    saved: Mutex<BTreeMap<PathBuf, String>>,
    /// Whether formatting aligns a table's columns: the client's `align` in its
    /// initialization options, on unless it says `false` (SPEC.md, Formatting).
    align: AtomicBool,
}

struct Watch {
    watcher: RecommendedWatcher,
    /// Each directory watched, and whether with everything under it.
    dirs: Vec<(PathBuf, bool)>,
}

/// A file a table can be read from, the kinds whose change is heard.
fn table_file(p: &Path) -> bool {
    p.extension().is_some_and(|e| e == "cells" || e == "csv" || e == "tsv")
}

impl Watch {
    /// `dir` watched, unless a directory watched already covers it.
    fn watch(&mut self, dir: &Path, whole: bool) {
        if self.dirs.iter().any(|(d, all)| d == dir || (*all && dir.starts_with(d))) {
            return;
        }
        let mode = if whole { RecursiveMode::Recursive } else { RecursiveMode::NonRecursive };
        if let Err(e) = self.watcher.watch(dir, mode) {
            eprintln!("cells lsp: {} is not watched, a change written there goes unheard: {e}", dir.display());
            return;
        }
        self.dirs.push((dir.to_path_buf(), whole));
    }
}

/// Every open file's diagnostics, sent again, and the directory of every file each read
/// watched: the file's own, and each its imports reached, wherever they are (an editor
/// may name no folder, and an import may climb out of the one it names).
async fn diagnose(client: &Client, folder: Folder, watch: &Mutex<Watch>) {
    for path in folder.open.keys() {
        let Some(file) = folder.read(path) else { continue };
        {
            let mut w = watch.lock().unwrap();
            for read in std::iter::once(&file.path).chain(&file.reached) {
                if let Some(dir) = read.parent() {
                    w.watch(dir, false);
                }
            }
        }
        client.publish_diagnostics(folder.uri(path), features::diagnostics(&file), None).await;
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
        diagnose(&self.client, self.folder(), &self.watch).await;
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
        let root = spelled.map(|s| canonical(&s));
        // the folder the editor named, whole: a file added anywhere in it is a key a rename
        // and a completion may reach
        if let Some(r) = &root {
            self.watch.lock().unwrap().watch(r, true);
        }
        self.folder.lock().unwrap().root = root;
        let align = params.initialization_options.as_ref().and_then(|o| o.get("align")).and_then(|a| a.as_bool());
        self.align.store(align.unwrap_or(true), Ordering::Relaxed);
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

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, p: DidOpenTextDocumentParams) {
        if let Some(path) = path(&p.text_document.uri) {
            if let Some(dir) = p.text_document.uri.to_file_path().ok().as_deref().and_then(Path::parent) {
                self.spell(dir);
            }
            self.saved.lock().unwrap().insert(path.clone(), p.text_document.text.clone());
            self.change(path, Some(p.text_document.text));
            self.diagnose().await;
        }
    }

    async fn did_save(&self, p: DidSaveTextDocumentParams) {
        if let (Some(path), Some(text)) = (path(&p.text_document.uri), p.text) {
            self.saved.lock().unwrap().insert(path, text);
        }
    }

    async fn formatting(&self, p: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let folder = self.folder();
        let Some(path) = path(&p.text_document.uri) else { return Ok(None) };
        let Some(file) = folder.read(&path) else { return Ok(None) };
        let saved = self.saved.lock().unwrap().get(&path).cloned();
        let saved = saved.and_then(|t| folder.read_as(&path, t));
        Ok(features::formatted(&file, saved.as_ref(), self.align.load(Ordering::Relaxed)).map(|e| Vec::from([e])))
    }

    async fn did_change(&self, p: DidChangeTextDocumentParams) {
        // Full sync: the last change is the whole text.
        if let (Some(path), Some(c)) = (path(&p.text_document.uri), p.content_changes.into_iter().last()) {
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
        let actions = features::actions(&folder, &file, p.range);
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
        // What the disk says changed, heard on the watcher's thread: only that something
        // did, since every open file is diagnosed again whatever it was.
        let (heard, mut hearing) = tokio::sync::mpsc::unbounded_channel::<()>();
        let watcher = notify::recommended_watcher(move |e: notify::Result<notify::Event>| {
            if e.is_ok_and(|e| !e.kind.is_access() && e.paths.iter().any(|p| table_file(p))) {
                // fails only once the server is gone
                let _ = heard.send(());
            }
        })
        .expect("a file watcher");
        let watch = Arc::new(Mutex::new(Watch { watcher, dirs: Vec::new() }));
        let (service, socket) = LspService::new(|client| {
            let (told, held, watching) = (client.clone(), folder.clone(), watch.clone());
            tokio::spawn(async move {
                while hearing.recv().await.is_some() {
                    // a save is several events: one pass for all of them
                    while hearing.try_recv().is_ok() {}
                    let folder = held.lock().unwrap().clone();
                    diagnose(&told, folder, &watching).await;
                }
            });
            Backend { client, folder, watch, saved: Mutex::new(BTreeMap::new()), align: AtomicBool::new(true) }
        });
        Server::new(tokio::io::stdin(), tokio::io::stdout(), socket).serve(service).await;
    });
}
