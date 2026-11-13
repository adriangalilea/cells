; A fence's content is the language its info string names: a script's
; ```ts, a table's ```cells (Cells again, a header and rows).

(code_block
  language: (fence_language) @injection.language
  content: (fence_content) @injection.content)

((comment) @injection.content
  (#set! injection.language "comment"))
