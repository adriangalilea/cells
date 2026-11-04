// The one token TCSV's lexer cannot decide by itself: the file of a reference
// type, `accounts` in `accounts.account` (PROVISIONAL, SPEC's Open section).
//
// A bare word may hold a dot (`v1.2`), so `accounts.account` is also a word.
// It is a reference only when it stands alone: a word, a dot, a column name,
// and then neither `|` nor `/`, which would make it a choice or a boolean's
// word. The token ends before the dot, so the file and the column are
// separate nodes.

#include "tree_sitter/parser.h"

#include <stdbool.h>

enum TokenType {
  REFERENCE_FILE,
  ERROR_SENTINEL,
};

static bool is_word_start(int32_t c) {
  return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c == '_';
}

static bool is_word_char(int32_t c) {
  return is_word_start(c) || (c >= '0' && c <= '9') || c == '-';
}

void *tree_sitter_tcsv_external_scanner_create(void) { return NULL; }

void tree_sitter_tcsv_external_scanner_destroy(void *payload) {}

unsigned tree_sitter_tcsv_external_scanner_serialize(void *payload, char *buffer) { return 0; }

void tree_sitter_tcsv_external_scanner_deserialize(void *payload, const char *buffer, unsigned length) {}

bool tree_sitter_tcsv_external_scanner_scan(void *payload, TSLexer *lexer, const bool *valid_symbols) {
  // Error recovery marks every token valid; the reference is never a guess.
  if (valid_symbols[ERROR_SENTINEL] || !valid_symbols[REFERENCE_FILE]) return false;

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
