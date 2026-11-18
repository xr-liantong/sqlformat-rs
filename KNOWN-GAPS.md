# Known gaps

Scope of v0.1 and every place this port knowingly differs from `sqlparse`
0.6.1 / `sqlformat`.

## Not implemented (exit status 2)

Upstream accepts these flags; this port prints `not implemented: <flag>` on
stderr and exits 2 (a wrong guess is worse than a refusal):

- `-a`, `--reindent_aligned` — needs `AlignedIndentFilter`
- `-s`, `--use_space_around_operators`
- `-l`, `--language`
- `--strip-comments`
- `--indent_after_first`, `--indent_columns`, `--indent_tabs`
- `--wrap_after`, `--comma_first`, `--compact`
- `--encoding`
- `--right_margin`, `--truncate_strings`, `--truncate_char`, `--output_format`

The `ReindentFilter` already carries `wrap_after`, `comma_first`,
`indent_columns` and `compact` internally, so wiring those three is mostly a
CLI change; `-a` needs a new filter.

## Python API not exposed

`sqlparse.parse()`, `Token`/`TokenList` objects and `sqlparse.format()` as a
library are out of scope. Only the CLI behaviour is reproduced.

## `\w` semantics on non-ASCII text

Python's `re` treats `\w` as Unicode letters/digits plus `_`. The Rust `regex`
crate additionally treats combining marks and connector punctuation as word
characters. The seven lookaround rules are hand-written and use
`char::is_alphanumeric() || '_'` (Python semantics), but the other 43 rules
use the `regex` crate, so a combining mark directly after a letter can split a
token differently. Plain ASCII and precomposed Latin/Cyrillic/Greek text are
unaffected; the corpus is pure ASCII.

## Error text

`[ERROR] ...` messages and exit codes match, but Python tracebacks for
malformed options are replaced by argparse-style usage text. `--bogus` output
is not byte-identical.

## CI

`.github/workflows/` is intentionally absent: the account token used to push
lacks the `workflow` scope, so GitHub rejects any commit that adds a workflow
file. The gates in the README are run manually (and by `tools/parity.sh`).
