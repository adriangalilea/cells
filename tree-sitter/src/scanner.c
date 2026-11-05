// The tokens Cells's lexer cannot decide by itself.
//
// The file of a reference type, `accounts` in `accounts.account`. A bare word
// may hold a dot (`v1.2`), so `accounts.account` is also a word. It is a
// reference only when it stands alone: a word, a dot, a column name, and then
// neither `|` nor `/`, which would make it a choice or a boolean's word. The
// token ends before the dot, so the file and the column are separate nodes.
//
// A fence (SPEC, The prelude): it opens with three backticks or more and
// closes at the first line that is as many backticks and nothing else, spaces
// and tabs aside. Counting them takes state: the opening count is kept until
// the closing line, serialized in one byte.

#include "tree_sitter/alloc.h"
#include "tree_sitter/parser.h"

#include <stdbool.h>
#include <stdint.h>

enum TokenType {
  REFERENCE_FILE,
  FENCE_OPEN,
  FENCE_CONTENT,
  FENCE_CLOSE,
  ERROR_SENTINEL,
};

typedef struct {
  uint8_t ticks;  // the open fence's backticks, 0 when none is open
} Scanner;

static bool is_word_start(int32_t c) {
  return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c == '_';
}

static bool is_word_char(int32_t c) {
  return is_word_start(c) || (c >= '0' && c <= '9') || c == '-';
}

static bool is_line_end(TSLexer *lexer) {
  return lexer->lookahead == '\n' || lexer->lookahead == '\r' || lexer->eof(lexer);
}

void *tree_sitter_cells_external_scanner_create(void) {
  Scanner *s = ts_calloc(1, sizeof(Scanner));
  return s;
}

void tree_sitter_cells_external_scanner_destroy(void *payload) { ts_free(payload); }

unsigned tree_sitter_cells_external_scanner_serialize(void *payload, char *buffer) {
  buffer[0] = (char)((Scanner *)payload)->ticks;
  return 1;
}

void tree_sitter_cells_external_scanner_deserialize(void *payload, const char *buffer, unsigned length) {
  ((Scanner *)payload)->ticks = length == 1 ? (uint8_t)buffer[0] : 0;
}

// At a line's start: is this line the open fence's closing one? Consumes the
// backticks and the spaces after them whatever the answer.
static bool closing_line(Scanner *s, TSLexer *lexer) {
  unsigned n = 0;
  while (lexer->lookahead == '`') {
    lexer->advance(lexer, false);
    n++;
  }
  while (lexer->lookahead == ' ' || lexer->lookahead == '\t') lexer->advance(lexer, false);
  return n == s->ticks && is_line_end(lexer);
}

static bool scan_reference(TSLexer *lexer) {
  while (lexer->lookahead == ' ' || lexer->lookahead == '\t') lexer->advance(lexer, true);

  if (!is_word_start(lexer->lookahead)) return false;
  while (is_word_char(lexer->lookahead)) lexer->advance(lexer, false);
  if (lexer->lookahead != '.') return false;
  lexer->mark_end(lexer);

  lexer->advance(lexer, false);
  if (!is_word_char(lexer->lookahead)) return false;
  while (is_word_char(lexer->lookahead)) lexer->advance(lexer, false);

  while (lexer->lookahead == ' ' || lexer->lookahead == '\t') lexer->advance(lexer, false);
  switch (lexer->lookahead) {
    case '|':
    case '/':
    case '.':
      return false;
    default:
      lexer->result_symbol = REFERENCE_FILE;
      return true;
  }
}

bool tree_sitter_cells_external_scanner_scan(void *payload, TSLexer *lexer, const bool *valid_symbols) {
  Scanner *s = payload;
  // Error recovery marks every token valid; none of these is ever a guess.
  if (valid_symbols[ERROR_SENTINEL]) return false;

  if (valid_symbols[FENCE_CLOSE] || valid_symbols[FENCE_CONTENT]) {
    // At the start of the fence's first line, or of the line after its content.
    if (s->ticks == 0) return false;
    if (lexer->lookahead == '`' && valid_symbols[FENCE_CLOSE]) {
      if (closing_line(s, lexer)) {
        lexer->mark_end(lexer);
        lexer->result_symbol = FENCE_CLOSE;
        s->ticks = 0;
        return true;
      }
      if (!valid_symbols[FENCE_CONTENT]) return false;
    } else if (!valid_symbols[FENCE_CONTENT]) {
      return false;
    }
    // The content: every line up to the closing one, or the file's end.
    bool any = false;
    for (;;) {
      while (!is_line_end(lexer)) lexer->advance(lexer, false);
      if (lexer->eof(lexer)) {
        lexer->mark_end(lexer);
        lexer->result_symbol = FENCE_CONTENT;
        return true;
      }
      if (lexer->lookahead == '\r') lexer->advance(lexer, false);
      if (lexer->lookahead == '\n') lexer->advance(lexer, false);
      any = true;
      lexer->mark_end(lexer);
      if (lexer->lookahead == '`' && closing_line(s, lexer)) {
        lexer->result_symbol = FENCE_CONTENT;
        return any;
      }
    }
  }

  if (valid_symbols[FENCE_OPEN]) {
    while (lexer->lookahead == ' ' || lexer->lookahead == '\t') lexer->advance(lexer, true);
    if (lexer->lookahead == '`') {
      unsigned n = 0;
      while (lexer->lookahead == '`') {
        lexer->advance(lexer, false);
        n++;
      }
      if (n < 3 || n > 255) return false;
      lexer->mark_end(lexer);
      s->ticks = (uint8_t)n;
      lexer->result_symbol = FENCE_OPEN;
      return true;
    }
    if (!valid_symbols[REFERENCE_FILE]) return false;
  }

  if (valid_symbols[REFERENCE_FILE]) return scan_reference(lexer);
  return false;
}
