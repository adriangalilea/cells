; What Zed's outline lists in a Cells file: every column by its name, every
; named type, and every table the prelude imports or writes.

(header_cell name: (column_name) @name) @item
(column_declaration name: (column_name) @name) @item
(type_definition name: (type_name) @name) @item
(import_statement default: (table_name) @name) @item
(table_definition name: (table_name) @name) @item
(script_definition name: (table_name) @name) @item
