-- Cells (github.com/adriangalilea/cells) in Neovim 0.11 or later, written by
-- `cells editors install neovim` with the parser and queries beside it; run it
-- again after updating Cells. The server's config is lsp/cells.lua.

vim.filetype.add({ extension = { cells = "cells" } })
vim.lsp.enable("cells")

vim.api.nvim_create_autocmd("FileType", {
  pattern = "cells",
  callback = function(args)
    vim.bo[args.buf].commentstring = "// %s"
    vim.treesitter.start(args.buf, "cells")
  end,
})

-- The server lays a file out as `cells fmt` does, its columns aligned, as inlay
-- hints; formatting on write stores every computed value the edit reaches.
vim.api.nvim_create_autocmd("LspAttach", {
  callback = function(args)
    local client = vim.lsp.get_client_by_id(args.data.client_id)
    if client.name ~= "cells" then
      return
    end
    vim.lsp.inlay_hint.enable(true, { bufnr = args.buf })
    vim.api.nvim_create_autocmd("BufWritePre", {
      buffer = args.buf,
      callback = function()
        vim.lsp.buf.format({ bufnr = args.buf, name = "cells" })
      end,
    })
  end,
})

-- A name the server reads as a column wears that column's colour, the six the
-- grammar's queries cycle through; anything else keeps the grammar's.
local RAINBOW = { "@variable", "@variable.parameter", "@tag", "@function", "@variable.member", "@label" }
local function rainbow()
  for n, group in ipairs(RAINBOW) do
    vim.api.nvim_set_hl(0, "@lsp.typemod.column.column" .. n .. ".cells", { link = group, default = true })
  end
end
rainbow()
vim.api.nvim_create_autocmd("ColorScheme", { callback = rainbow })
