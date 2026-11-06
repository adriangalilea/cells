; What Helix's symbol picker (`space s`) lists in a Cells file: every column by
; its name, every named type, and every table the prelude imports or writes.

(header_cell name: (column_name) @definition.constant)
(type_definition name: (type_name) @definition.type)
(import_statement default: (table_name) @definition.module)
(table_definition name: (table_name) @definition.module)
(script_definition name: (table_name) @definition.module)
