; Helix's selections and motions in a Cells file, by what a table is made of. A
; cell is an argument (`mia`/`maa` select it, `]a`/`[a` go to the next and the
; one before), a header cell or a row cell alike, its comma in the around; a
; row and the header are entries (`]e`/`[e`); a formula is a function's body
; (`mif`, the column's whole header cell or the cell its `maf`); a `//` is a
; comment (`mic`).

(header_cell) @parameter.around
(header_cell name: (_) @parameter.inside)
(field (_) @parameter.inside) @parameter.around

(header) @entry.around
(row) @entry.around

(header_cell formula: (_) @function.inside) @function.around
(computed_cell formula: (_) @function.inside) @function.around

(comment) @comment.inside
(comment) @comment.around
