//! Zed runs `cells lsp` from the `cells` a shell finds on PATH: on a Mac with Cells.app,
//! the signed copy the app links into `~/.local/bin`, the one its engine admits.

use zed_extension_api as zed;

struct Cells;

impl zed::Extension for Cells {
    fn new() -> Self {
        Cells
    }

    fn language_server_command(&mut self, _: &zed::LanguageServerId, worktree: &zed::Worktree) -> zed::Result<zed::Command> {
        let command = worktree.which("cells").ok_or(
            "`cells` is not on PATH: Cells.app links it into ~/.local/bin (Contents/Helpers/cells); elsewhere install the cells command",
        )?;
        Ok(zed::Command { command, args: vec!["lsp".into()], env: worktree.shell_env() })
    }
}

zed::register_extension!(Cells);
