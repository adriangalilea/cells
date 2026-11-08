# How a value looks: the cases it would add

The conformance cases SPEC.md's proposal "How a value looks" (Open) would add to `tests/`, drafted here so they are read beside the proposal and never run: every runner reads every folder under `tests/`, and none of this is the format until it is decided. Once it is, each case moves to the folder its rule belongs to (`choices/`, `prelude/`) and the reader learns it in the same commit.

Each case is a `.cells` and its `.json`, the suite's shape ([tests/README.md](../../tests/README.md)) with what the proposal adds:

- `tints`: each column holding a choice, or each named type, and the tint of each of its words that has one, as written (`"green"`, `"#3fa34d"`). A word with none is left out.
- `looks`: each column or named type a `look` statement names, and its rules in order, each as written after the `:` and split at its commas, its spaces cut.

| Case | What it pins |
|---|---|
| `choices/tinted-words` | a choice's words with tints, one without; a cell holds the word alone |
| `choices/tinted-hex` | a tint as `#` and six hex digits |
| `choices/broken-unknown-tint` | a tint no name is breaks the column, naming the word |
| `choices/quoted-hash` | `#` inside a quoted word is part of the word |
| `prelude/look-rules` | `look` on a named type and on a column, the rules in order |
| `prelude/broken-look-rule` | a rule that does not read takes out only its line; the values are whole |
