-- The cells language server, as vim.lsp.config("cells") finds it on the
-- runtimepath; a vim.lsp.config("cells", { ... }) of your own is laid over it.
return {
  cmd = { "cells", "lsp" },
  filetypes = { "cells" },
  root_markers = { ".git" },
}
