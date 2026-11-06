/**
 * @file Cells, data and its logic in one plain-text file: RFC 4180 records, a
 * header whose cells carry their column's type and formula
 * (`name:type=formula`), cells that hold their own formula, and a prelude
 * fenced by `---` that names types, imports files and holds tables.
 * @see ../SPEC.md
 *
 * Every column of the header and of a row is a `field` (a `header_cell` in
 * the header) that holds its trailing comma, so the n-th named child of a
 * `row` is the n-th column even when cells are blank: queries colour a column
 * by its position with anchors, `(row . (field) . (field) @second)`.
 *
 * Data cells are classified by their spelling alone (a number, a date, a
 * boolean, a URL, quoted text, text, a formula). What a column's header says
 * its cells are is the language server's to colour, as semantic tokens over
 * these nodes; the grammar never reads the header to classify a row.
 *
 * The prelude's statements are read one line each, and a fence (a script, a
 * table written inline, a script's stored rows and runs) is a node of its
 * own: which fence belongs to which statement, and whether a script has the
 * fences it needs, is the reader's to check, never the highlighter's. A
 * fence's content is injected in the language its info string names.
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

const BASES = ['text', 'int', 'num', 'pct', 'bool', 'date', 'time', 'datetime', 'url', 'email'];

// ISO 4217's active codes, the same list the TypeScript reader holds.
const CURRENCIES = (
  'AED AFN ALL AMD AOA ARS AUD AWG AZN BAM BBD BDT BGN BHD BIF BMD BND BOB BRL BSD ' +
  'BTN BWP BYN BZD CAD CDF CHF CLP CNY COP CRC CUP CVE CZK DJF DKK DOP DZD EGP ERN ' +
  'ETB EUR FJD FKP GBP GEL GHS GIP GMD GNF GTQ GYD HKD HNL HTG HUF IDR ILS INR IQD ' +
  'IRR ISK JMD JOD JPY KES KGS KHR KMF KPW KRW KWD KYD KZT LAK LBP LKR LRD LSL LYD ' +
  'MAD MDL MGA MKD MMK MNT MOP MRU MUR MVR MWK MXN MYR MZN NAD NGN NIO NOK NPR NZD ' +
  'OMR PAB PEN PGK PHP PKR PLN PYG QAR RON RSD RUB RWF SAR SBD SCR SDG SEK SGD SHP ' +
  'SLE SOS SRD SSP STN SVC SYP SZL THB TJS TMT TND TOP TRY TTD TWD TZS UAH UGX USD ' +
  'UYU UZS VES VND VUV WST XAF XCD XCG XOF XPF YER ZAR ZMW ZWG'
).split(' ');

const FORMAT_FIELDS = ['YYYY', 'MM', 'DD', 'HH', 'mm', 'ss'];

const SCHEDULE_WORDS = ['every', 'at', 'on', 'in', 'cron', 'when', 'changes'];

// A formula's keyword, in capitals and only in capitals (SPEC, The language: a
// word in lower case is always a name), named by its lower-case spelling. Its
// precedence over `identifier` makes `AND` a keyword before it is a name.
const keyword = word => alias(token(prec(2, word.toUpperCase())), word);

const commaSep1 = rule => seq(rule, repeat(seq(',', rule)));

module.exports = grammar({
  name: 'cells',

  // Spaces and tabs between tokens; a line break is a token, since it ends a
  // record and a prelude statement. A comment is no extra: `//` is only a
  // comment in the prelude, and a cell such as `https://…` must stay a cell.
  extras: _ => [/[ \t]/],

  externals: $ => [
    // `accounts` in `accounts.account`, recognised only when a column name
    // follows the dot and no `|` or `/` does, so `v1.2|v2.0` stays a choice of
    // two words. A union of references (`shared.account|clients.account`)
    // therefore reads as a choice: which word names a table is the reader's.
    $.reference_file,
    // A fence: its opening backticks (three or more), what it holds up to the
    // line of as many backticks, and that line. Only a scanner counts them.
    $.fence_open,
    $.fence_content,
    $.fence_close,
    $._error_sentinel,
  ],

  rules: {
    source_file: $ => seq(
      optional($.prelude),
      optional($.header),
      repeat(seq($._newline, optional($.row))),
    ),

    // ── The prelude ──────────────────────────────────────────────────────────

    // The closing fence's line break is the prelude's, so the header starts
    // on the next line; a file may end at the fence.
    prelude: $ => prec.right(seq(
      $.fence,
      $._newline,
      repeat($._prelude_line),
      $.fence,
      optional($._newline),
    )),

    fence: _ => '---',

    _prelude_line: $ => choice(
      $._newline,
      seq($._statement, optional($.comment), $._newline),
      seq($.comment, $._newline),
      seq($.code_block, $._newline),
    ),

    _statement: $ => choice(
      $.type_definition,
      $.import_statement,
      $.table_definition,
      $.script_definition,
    ),

    type_definition: $ => seq(
      optional('export'),
      'type',
      field('name', alias($._type_identifier, $.type_name)),
      '=',
      field('type', $.type),
    ),

    // `import accounts from "../accounts.cells"`, `import { Kind, fees } from
    // "./rates.cells"`, a URL pinned `with { sha256: "…" }`.
    import_statement: $ => seq(
      'import',
      choice(field('default', alias($.identifier, $.table_name)), $.named_imports),
      'from',
      field('source', $.string),
      optional($.import_attributes),
    ),

    named_imports: $ => seq('{', commaSep1(alias($.identifier, $.imported_name)), optional(','), '}'),

    import_attributes: $ => seq(
      'with',
      '{',
      commaSep1($.import_attribute),
      optional(','),
      '}',
    ),

    import_attribute: $ => seq(
      field('key', alias($.identifier, $.attribute_name)),
      ':',
      field('value', $.string),
    ),

    string: $ => seq('"', optional(alias(/[^"\r\n]+/, $.string_content)), '"'),

    // `fees = ```cells`, its rows on the lines after it.
    table_definition: $ => seq(
      optional('export'),
      field('name', alias($.identifier, $.table_name)),
      '=',
      field('table', $.code_block),
    ),

    // `eurusd = run every day at 18:00 in Europe/Madrid`; its script, table
    // and runs are the fences on the lines after it.
    script_definition: $ => seq(
      optional('export'),
      field('name', alias($.identifier, $.table_name)),
      '=',
      'run',
      field('schedule', $.schedule),
    ),

    schedule: $ => repeat1(choice(
      ...SCHEDULE_WORDS,
      ',',
      $.schedule_time,
      $.schedule_word,
    )),

    schedule_time: _ => token(prec(1, /\d{1,2}:\d{2}/)),

    schedule_word: _ => /[^\s,\/][^\s,]*/,

    code_block: $ => seq(
      $.fence_open,
      optional(field('language', alias(token.immediate(/[A-Za-z0-9_+-]+/), $.fence_language))),
      optional(field('label', alias(/[a-z]+/, $.fence_label))),
      $._newline,
      optional(field('content', $.fence_content)),
      $.fence_close,
    ),

    comment: _ => token(seq('//', /[^\r\n]*/)),

    // ── A formula ────────────────────────────────────────────────────────────
    // SQL's scalar expressions (SPEC, Formulas): after a header cell's type,
    // `total:EUR=cost * qty`, and after a cell's `=`, `32.25=96.75 / 3`.

    _expression: $ => choice(
      alias($._number, $.number),
      $.string_literal,
      alias(token(prec(2, choice('TRUE', 'FALSE'))), $.boolean),
      alias(keyword('null'), $.null),
      $.table_self,
      $.column_reference,
      $.member_expression,
      $.key_expression,
      $.row_reference,
      $.call_expression,
      $.binary_expression,
      $.unary_expression,
      $.not_expression,
      $.null_test,
      $.case_expression,
      $.parenthesized_expression,
    ),

    column_reference: $ => choice($.identifier, alias($._quoted_name, $.quoted_name)),

    _quoted_name: _ => seq('"', /[^"\r\n]+/, '"'),

    string_literal: $ => seq(
      '\'',
      repeat(choice(/[^'\r\n]+/, alias('\'\'', $.escape_sequence))),
      '\'',
    ),

    // `TABLE`: the file's own table, whatever the file is named.
    table_self: _ => 'TABLE',

    // `to.kind`: the column `kind` of the row the reference `to` names.
    member_expression: $ => prec(9, seq(
      field('object', choice($.column_reference, $.member_expression, $.key_expression, $.table_self)),
      token.immediate('.'),
      field('property', alias(token.immediate(/[A-Za-z_][A-Za-z0-9_]*/), $.property)),
    )),

    // `fees[kind]`: a table's row by its key.
    key_expression: $ => prec(9, seq(
      field('table', choice(alias($.identifier, $.table_reference), $.table_self)),
      token.immediate('['),
      field('key', $._expression),
      ']',
    )),

    // `running[-1]`, `total[FIRST:THIS]`: the column on a row by where it is
    // from this one, or on a range of rows (SPEC, Rows by where they are).
    row_reference: $ => prec(9, seq(
      field('column', alias($.identifier, $.column_reference)),
      token.immediate('['),
      choice(
        field('offset', $.row_offset),
        seq(field('from', $._row_end), ':', field('to', $._row_end)),
      ),
      ']',
    )),

    _row_end: $ => choice($.row_offset, $.row_word),

    row_offset: _ => token(prec(3, /[+-]\d+/)),

    row_word: _ => choice('FIRST', 'THIS', 'LAST'),

    // A function is a name in capitals; a call of a name in lower case is the
    // same node, which the queries colour as the name it is (`sum(` is no
    // function, SPEC, The language).
    call_expression: $ => prec(9, seq(
      field('function', alias($.identifier, $.function_name)),
      token.immediate('('),
      optional(commaSep1($._expression)),
      ')',
      optional(field('window', $.over_clause)),
    )),

    // `SUM(amount) OVER (ORDER BY date DESC, id)`: a window.
    over_clause: $ => seq(
      keyword('over'),
      '(',
      keyword('order'),
      keyword('by'),
      commaSep1($.order_key),
      ')',
    ),

    order_key: $ => seq($._expression, optional(keyword('desc'))),

    binary_expression: $ => choice(
      ...[
        [keyword('or'), 1],
        [keyword('and'), 2],
        ['=', 4], ['<>', 4], ['!=', 4], ['<', 4], ['<=', 4], ['>', 4], ['>=', 4],
        ['||', 5],
        ['+', 6], ['-', 6],
        ['*', 7], ['/', 7],
      ].map(([operator, precedence]) =>
        prec.left(/** @type {number} */ (precedence), seq(
          field('left', $._expression),
          field('operator', /** @type {any} */ (operator)),
          field('right', $._expression),
        )),
      ),
    ),

    not_expression: $ => prec(3, seq(field('operator', keyword('not')), field('operand', $._expression))),

    null_test: $ => prec.left(4, seq(
      field('operand', $._expression),
      keyword('is'),
      optional(keyword('not')),
      keyword('null'),
    )),

    unary_expression: $ => prec(8, seq(field('operator', '-'), field('operand', $._expression))),

    case_expression: $ => seq(
      keyword('case'),
      repeat1($.when_clause),
      optional(seq(keyword('else'), field('else', $._expression))),
      keyword('end'),
    ),

    when_clause: $ => seq(
      keyword('when'),
      field('condition', $._expression),
      keyword('then'),
      field('value', $._expression),
    ),

    parenthesized_expression: $ => seq('(', $._expression, ')'),

    // ── The header ───────────────────────────────────────────────────────────

    header: $ => $._header_cells,

    _header_cells: $ => choice(
      seq(
        repeat1(alias($._header_cell_with_comma, $.header_cell)),
        optional(alias($._header_cell_content, $.header_cell)),
      ),
      alias($._header_cell_content, $.header_cell),
    ),

    _header_cell_with_comma: $ => seq(optional($._header_cell_content), ','),

    _header_cell_content: $ => choice(
      seq(field('name', $.column_name), optional($._typed)),
      $._typed,
      $._quoted_header_cell,
    ),

    // A header cell quoted (its formula holds a `,` or a `"`) reads as a bare one inside
    // its quotes: its name up to the first `:`, then its type and formula.
    _quoted_header_cell: $ => seq(
      '"',
      optional(field('name', alias($._quoted_column_name, $.column_name))),
      optional($._typed),
      '"',
    ),

    _quoted_column_name: _ => token(/([^":\r\n]|"")+/),

    _typed: $ => seq(':', field('type', $.type), optional(seq('=', field('formula', $._expression)))),

    column_name: _ => /[^:,\r\n" \t][^:,\r\n]*/,

    // ── A type ───────────────────────────────────────────────────────────────
    // The same grammar after a header cell's `:` and after a prelude's
    // `type Name =`. Each shorthand and its explicit word make one node:
    // `!` and `key` are both a key_mark, `>=0` and `min 0` both a bound.

    type: $ => repeat1(choice(
      $.base_type,
      $.currency,
      $.named_type,
      $.reference_type,
      $.choices,
      $.bool_pair,
      $.bound,
      $.range,
      $.scale,
      $.format,
      $.key_mark,
      $.optional_mark,
    )),

    // `EUR(6)`: a currency's scale, its digits after the point.
    scale: $ => seq('(', alias(token(prec(1, /\d+/)), $.number), ')'),

    base_type: _ => choice(...BASES),

    currency: _ => choice(...CURRENCIES),

    named_type: $ => $._type_identifier,

    // A column typed by another table's key: `accounts.account`.
    reference_type: $ => seq(
      field('file', $.reference_file),
      token.immediate('.'),
      field('column', alias(token.immediate(/[A-Za-z0-9_][A-Za-z0-9_-]*/), $.reference_column)),
    ),

    choices: $ => choice(
      seq($.choice, repeat1(seq('|', $.choice))),
      alias($._quoted_choice, $.choice),
      prec.right(seq('one', 'of', repeat1(alias($._one_of_item, $.choice)))),
    ),

    // A choice, bare or quoted. A base type's or a currency's word is a
    // choice when `|` joins it to others (`text|code` is two choices).
    choice: $ => choice($._quoted_choice, $._bare_choice, 'one', 'of', 'and', 'bool'),

    // `one of a b c` takes words until an explicit keyword.
    _one_of_item: $ => choice($._quoted_choice, $._bare_choice, 'bool'),

    _bare_choice: $ => choice($._number, $._type_identifier, $._word, ...BASES.filter(b => b !== 'bool'), ...CURRENCIES),

    _quoted_choice: $ => seq(
      '\'',
      repeat(choice(/[^']+/, alias('\'\'', $.escape_sequence))),
      '\'',
    ),

    bool_pair: $ => choice(
      seq(field('true', $.bool_word), '/', field('false', $.bool_word)),
      prec.right(1, seq('bool', field('true', $.bool_word), field('false', $.bool_word))),
    ),

    bool_word: $ => choice($._quoted_choice, $._bare_choice, 'one', 'of', 'and', 'bool'),

    bound: $ => choice(
      seq(field('operator', choice('>=', '>', '<=', '<')), field('value', alias($._number, $.number))),
      seq(field('operator', choice('min', 'max')), field('value', alias($._number, $.number))),
    ),

    range: $ => choice(
      seq(field('min', alias($._number, $.number)), '..', field('max', alias($._number, $.number))),
      seq('between', field('min', alias($._number, $.number)), 'and', field('max', alias($._number, $.number))),
    ),

    format: $ => seq('(', repeat(choice($.format_field, $.format_literal)), ')'),

    format_field: _ => choice(...FORMAT_FIELDS),

    format_literal: _ => /[^)\s]/,

    key_mark: _ => choice('!', 'key'),

    optional_mark: _ => choice('?', 'optional'),

    // ── A row ────────────────────────────────────────────────────────────────

    row: $ => choice(
      seq(
        repeat1(alias($._field_with_comma, $.field)),
        optional(alias($._value, $.field)),
      ),
      alias($._value, $.field),
    ),

    _field_with_comma: $ => seq(optional($._value), ','),

    _value: $ => choice($.quoted_field, $._written, $.computed_cell, alias($._quoted_computed, $.computed_cell)),

    // The order is the lexer's tie-break: a cell that reads as a date and as
    // text is a date, `84.20` is a number before it is text.
    _written: $ => choice(
      $.datetime,
      $.date,
      $.time,
      alias($._data_number, $.number),
      $.boolean,
      $.url,
      $.email,
      $.text,
    ),

    // A computed cell (SPEC, A cell): what the file stores, `=`, then its own
    // formula or none (its column's): `84.20=`, `32.25=96.75 / 3`, `=96.75 / 3`.
    computed_cell: $ => seq(
      optional(field('stored', $._written)),
      '=',
      optional(field('formula', $._expression)),
    ),

    // A quoted cell is text up to its first `=` with no `'` before it; with one it is a
    // computed cell inside its quotes (its own formula holds a `,`), what it stores
    // before the `=` as written.
    quoted_field: $ => seq(
      '"',
      repeat(choice($._quoted_text, alias('""', $.escape_sequence), alias("'=", $.escape_sequence))),
      '"',
    ),

    _quoted_computed: $ => seq(
      '"',
      optional(field('stored', alias($._quoted_text, $.text))),
      '=',
      optional(field('formula', $._expression)),
      '"',
    ),

    _quoted_text: _ => token(/([^"=']|'[^"=])+|'/),

    datetime: _ => /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}(:\d{2}(\.\d+)?)?(Z|[+-]\d{2}:?\d{2})?/,

    date: _ => /\d{4}-\d{2}-\d{2}/,

    time: _ => /\d{2}:\d{2}(:\d{2})?/,

    _data_number: _ => /[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?%?/,

    boolean: _ => choice('true', 'false', 'yes', 'no'),

    // A value never holds a bare `=`, the split of a computed cell; a text
    // writes one `'=`.
    url: _ => /[A-Za-z][A-Za-z0-9+.-]*:\/\/([^,\s"=']|'=?)+/,

    email: _ => /([^\s@,"=']|'=?)+@([^\s@,"=']|'=?)+\.([^\s@,"=']|'=?)+/,

    text: _ => /([^,\r\n" \t=']|'=?)([^,\r\n=']|'=?)*/,

    // ── Tokens shared by the rules above ─────────────────────────────────────
    // Declared after the rules that use them, before `_word`: on a tie the
    // earlier token wins, so `84.20` is a number and `Category` a type name
    // before either is a bare word.

    _number: _ => /[+-]?(\d+(\.\d+)?|\.\d+)([eE][+-]?\d+)?/,

    _type_identifier: _ => /[A-Z][A-Za-z0-9_]*/,

    identifier: _ => /[A-Za-z_][A-Za-z0-9_]*/,

    // A bare word inside a type: anything but a reserved character, a space,
    // and `..`; a single `.` may stand inside one (`v1.2`).
    _word: _ => /[^\s|\/!?<>=()',".]+(\.[^\s|\/!?<>=()',".]+)*/,

    _newline: _ => /\r?\n/,
  },
});
