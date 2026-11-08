; Cells highlights, in the capture names Neovim, Zed and the tree-sitter CLI
; share. Helix names its scopes differently: queries/helix/highlights.scm is
; the same map in Helix's names.
;
; Each capture lands on one node and no node is captured twice, so the
; result is the same whether an editor lets the first pattern win (Helix,
; tree-sitter-highlight) or the last (Neovim).
;
; What a column's header says its cells are (money, a date in its format, a
; reference into another file) is the language server's to colour, as
; semantic tokens laid over these: the grammar classifies a cell by its
; spelling alone.

; ── The prelude ────────────────────────────────────────────────────────────

(fence) @punctuation.special

(comment) @comment @spell

"type" @keyword.type

"export" @keyword.modifier

[
  "import"
  "from"
  "with"
] @keyword.import

(type_definition
  name: (type_name) @type.definition)

(type_definition
  "=" @operator)

; A column's declaration: which column it is, the header says, which a query cannot
; read, so its name wears one capture; the language server's tokens give its column.
(column_declaration
  name: (column_name) @variable.member)

(column_declaration
  [
    ":"
    "="
  ] @operator)

(import_statement
  default: (table_name) @module)

(imported_name) @module

(named_imports
  [
    "{"
    "}"
  ] @punctuation.bracket)

(named_imports
  "," @punctuation.delimiter)

(import_attributes
  [
    "{"
    "}"
  ] @punctuation.bracket)

(import_attribute
  key: (attribute_name) @attribute
  ":" @punctuation.delimiter)

(string) @string.special.path

(table_definition
  name: (table_name) @function)

(table_definition
  "=" @operator)

(script_definition
  name: (table_name) @function)

(script_definition
  "=" @operator)

"run" @keyword

(schedule
  [
    "every"
    "at"
    "on"
    "in"
    "cron"
    "when"
    "changes"
  ] @keyword)

(schedule
  "," @punctuation.delimiter)

(schedule_time) @number

(schedule_word) @string.special

; A fence's backticks are its frame; its language and label say what it holds,
; and its content is the language's own (queries/injections.scm).
[
  (fence_open)
  (fence_close)
] @punctuation.special

(fence_language) @label

(fence_label) @attribute

; ── A formula ──────────────────────────────────────────────────────────────

(column_reference) @variable.member

(property) @variable.member

(table_reference) @module

; The language is in capitals: a function's name, never one in lower case,
; which is a name the file gave (SPEC, The language).
((function_name) @function
  (#match? @function "^[A-Z][A-Z0-9_]*$"))

((function_name) @variable.member
  (#match? @variable.member "[a-z]"))

; TABLE, the file's own table; FIRST, THIS, LAST, rows by where they are.
(table_self) @variable.builtin

(row_word) @variable.builtin

(row_offset) @number

(row_reference
  [
    "["
    "]"
  ] @punctuation.bracket)

(row_reference
  ":" @operator)

(over_clause
  [
    "over"
    "order"
    "by"
  ] @keyword)

(over_clause
  [
    "("
    ")"
  ] @punctuation.bracket)

(over_clause
  "," @punctuation.delimiter)

(order_key
  "desc" @keyword)

(string_literal) @string

(string_literal
  (escape_sequence) @string.escape)

(null) @constant

(binary_expression
  operator: _ @operator)

(unary_expression
  operator: _ @operator)

(not_expression
  operator: _ @keyword.operator)

(null_test
  [
    "is"
    "not"
  ] @keyword.operator)

(case_expression
  [
    "case"
    "else"
    "end"
  ] @keyword)

(when_clause
  [
    "when"
    "then"
  ] @keyword)

(member_expression
  "." @punctuation.delimiter)

(key_expression
  [
    "["
    "]"
  ] @punctuation.bracket)

(call_expression
  [
    "("
    ")"
  ] @punctuation.bracket)

(call_expression
  "," @punctuation.delimiter)

(parenthesized_expression
  [
    "("
    ")"
  ] @punctuation.bracket)

(header_cell
  "=" @operator)

; A computed cell's `=`: left of it what the file stores, right of it how it
; is computed (SPEC, A cell).
(computed_cell
  "=" @punctuation.special)

; ── A type ─────────────────────────────────────────────────────────────────

(base_type) @type.builtin

(currency) @type.builtin

(named_type) @type

; A column typed by another table's key (SPEC, References).
(reference_type
  file: (reference_file) @module
  "." @punctuation.delimiter
  column: (reference_column) @variable.member)

(choice) @constant

(choice
  (escape_sequence) @string.escape)

(choices
  "|" @punctuation.delimiter)

(choices
  [
    "one"
    "of"
  ] @keyword)

(bool_word) @boolean

(bool_word
  (escape_sequence) @string.escape)

(bool_pair
  "/" @punctuation.delimiter)

(bool_pair
  "bool" @type.builtin)

(bound
  operator: [
    ">="
    ">"
    "<="
    "<"
  ] @operator)

(bound
  operator: [
    "min"
    "max"
  ] @keyword.operator)

(range
  ".." @operator)

(range
  [
    "between"
    "and"
  ] @keyword.operator)

(format
  [
    "("
    ")"
  ] @punctuation.bracket)

(scale
  [
    "("
    ")"
  ] @punctuation.bracket)

(format_field) @string.special

(format_literal) @punctuation.special

; The key is loud, may-be-blank is quiet.
(key_mark) @attribute

(optional_mark) @keyword.modifier

; ── Cells ──────────────────────────────────────────────────────────────────

(header_cell
  ":" @punctuation.delimiter)

(header_cell
  "," @punctuation.delimiter)

(field
  "," @punctuation.delimiter)

(number) @number

; What a computed cell stores, the engine's to write: one colour whatever it
; spells, so it never reads as a value someone typed.
(stored) @constant.builtin.computed

(boolean) @boolean

[
  (date)
  (datetime)
  (time)
] @string.special

[
  (url)
  (email)
] @string.special.url

(quoted_field) @string

(quoted_field
  (escape_sequence) @string.escape)

; ── Rainbow columns ────────────────────────────────────────────────────────
; A column's name in the header and its text cells share a colour, six
; colours in turn, through the 24th column. A field holds its trailing comma,
; so the n-th named child of a row is the n-th column even when cells are
; blank; numbers, dates, booleans and quoted text keep their own colours.

(header . (header_cell name: (column_name) @variable))
(header . (_) . (header_cell name: (column_name) @variable.parameter))
(header . (_) . (_) . (header_cell name: (column_name) @tag))
(header . (_) . (_) . (_) . (header_cell name: (column_name) @function))
(header . (_) . (_) . (_) . (_) . (header_cell name: (column_name) @variable.member))
(header . (_) . (_) . (_) . (_) . (_) . (header_cell name: (column_name) @label))
(header . (_) . (_) . (_) . (_) . (_) . (_) . (header_cell name: (column_name) @variable))
(header . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (header_cell name: (column_name) @variable.parameter))
(header . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (header_cell name: (column_name) @tag))
(header . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (header_cell name: (column_name) @function))
(header . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (header_cell name: (column_name) @variable.member))
(header . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (header_cell name: (column_name) @label))
(header . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (header_cell name: (column_name) @variable))
(header . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (header_cell name: (column_name) @variable.parameter))
(header . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (header_cell name: (column_name) @tag))
(header . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (header_cell name: (column_name) @function))
(header . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (header_cell name: (column_name) @variable.member))
(header . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (header_cell name: (column_name) @label))
(header . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (header_cell name: (column_name) @variable))
(header . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (header_cell name: (column_name) @variable.parameter))
(header . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (header_cell name: (column_name) @tag))
(header . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (header_cell name: (column_name) @function))
(header . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (header_cell name: (column_name) @variable.member))
(header . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (header_cell name: (column_name) @label))

(row . (field (text) @variable))
(row . (_) . (field (text) @variable.parameter))
(row . (_) . (_) . (field (text) @tag))
(row . (_) . (_) . (_) . (field (text) @function))
(row . (_) . (_) . (_) . (_) . (field (text) @variable.member))
(row . (_) . (_) . (_) . (_) . (_) . (field (text) @label))
(row . (_) . (_) . (_) . (_) . (_) . (_) . (field (text) @variable))
(row . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (field (text) @variable.parameter))
(row . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (field (text) @tag))
(row . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (field (text) @function))
(row . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (field (text) @variable.member))
(row . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (field (text) @label))
(row . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (field (text) @variable))
(row . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (field (text) @variable.parameter))
(row . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (field (text) @tag))
(row . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (field (text) @function))
(row . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (field (text) @variable.member))
(row . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (field (text) @label))
(row . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (field (text) @variable))
(row . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (field (text) @variable.parameter))
(row . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (field (text) @tag))
(row . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (field (text) @function))
(row . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (field (text) @variable.member))
(row . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (_) . (field (text) @label))
