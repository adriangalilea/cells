; A row, a header and a prelude statement start at the line's start: Cells
; indents nothing of its own. What this query is for is the fences: Helix
; indents an injected language by that language's query only when the file's
; own language has one, so with this file a ```ts script indents as TypeScript.
; A fence whose language has no indent query in Helix (and a ```cells table,
; Cells again) keeps each line's whitespace as written.

(fence_content) @opaque
