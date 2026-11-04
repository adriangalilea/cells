/**
 * @file TCSV, typed CSV: RFC 4180 records, a header whose cells carry their
 * column's type (`name:type`), and an optional prelude fenced by `---`.
 * @see ../SPEC.md
 *
 * Every column of the header and of a row is a `field` (a `header_cell` in
 * the header) that holds its trailing comma, so the n-th named child of a
 * `row` is the n-th column even when cells are blank: queries colour a column
 * by its position with anchors, `(row . (field) . (field) @second)`.
 *
 * Data cells are classified by their spelling alone (a number, a date, a
 * boolean, a URL, quoted text, text). What a column's header says its cells
 * are is the language server's to colour, as semantic tokens over these
 * nodes; the grammar never reads the header to classify a row.
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

module.exports = grammar({
  name: 'tcsv',

  // Spaces and tabs between tokens; a line break is a token, since it ends a
  // record and a prelude statement. A comment is no extra: `//` is only a
  // comment in the prelude, and a cell such as `https://…` must stay a cell.
  extras: _ => [/[ \t]/],

  externals: $ => [
    // PROVISIONAL (SPEC, Open: references between files). `accounts` in
    // `accounts.account`, recognised only when a column name follows the dot
    // and no `|` or `/` does, so `v1.2|v2.0` stays a choice of two words.
    $.reference_file,
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
    ),

    _statement: $ => choice($.use_statement, $.type_definition, $.computed_column),

    // PROVISIONAL (SPEC, Open: imports; the keyword is not decided).
    use_statement: $ => seq('use', field('path', $.path)),

    path: _ => /[^\s]+/,

    type_definition: $ => seq(
      'type',
      field('name', alias($._type_identifier, $.type_name)),
      '=',
      field('type', $.type),
    ),

    computed_column: $ => seq(
      field('name', alias($.identifier, $.column_name)),
      '=',
      field('value', $._expression),
    ),

    _expression: $ => choice(
      alias($._number, $.number),
      alias($.identifier, $.column_reference),
      $.binary_expression,
      $.unary_expression,
      $.parenthesized_expression,
    ),

    binary_expression: $ => choice(
      ...[['+', 1], ['-', 1], ['*', 2], ['/', 2]].map(([operator, precedence]) =>
        prec.left(/** @type {number} */ (precedence), seq(
          field('left', $._expression),
          field('operator', /** @type {string} */ (operator)),
          field('right', $._expression),
        )),
      ),
    ),

    unary_expression: $ => prec(3, seq(field('operator', '-'), field('operand', $._expression))),

    parenthesized_expression: $ => seq('(', $._expression, ')'),

    comment: _ => token(seq('//', /[^\r\n]*/)),

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
      seq(field('name', $.column_name), optional(seq(':', field('type', $.type)))),
      seq(':', field('type', $.type)),
      field('name', alias($.quoted_field, $.column_name)),
    ),

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
      $.format,
      $.key_mark,
      $.optional_mark,
    )),

    base_type: _ => choice(...BASES),

    currency: _ => choice(...CURRENCIES),

    named_type: $ => $._type_identifier,

    // PROVISIONAL (SPEC, Open): a column typed by another file's key.
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

    // The order is the lexer's tie-break: a cell that reads as a date and as
    // text is a date, `84.20` is a number before it is text.
    _value: $ => choice(
      $.quoted_field,
      $.datetime,
      $.date,
      $.time,
      alias($._data_number, $.number),
      $.boolean,
      $.url,
      $.email,
      $.text,
    ),

    quoted_field: $ => seq(
      '"',
      repeat(choice(/[^"]+/, alias('""', $.escape_sequence))),
      '"',
    ),

    datetime: _ => /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}(:\d{2}(\.\d+)?)?(Z|[+-]\d{2}:?\d{2})?/,

    date: _ => /\d{4}-\d{2}-\d{2}/,

    time: _ => /\d{2}:\d{2}(:\d{2})?/,

    _data_number: _ => /[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?%?/,

    boolean: _ => choice('true', 'false', 'yes', 'no'),

    url: _ => /[A-Za-z][A-Za-z0-9+.-]*:\/\/[^,\s"]+/,

    email: _ => /[^\s@,"]+@[^\s@,"]+\.[^\s@,"]+/,

    text: _ => /[^,\r\n" \t][^,\r\n]*/,

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
