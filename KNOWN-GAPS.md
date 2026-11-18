# Known gaps

Scope of v0.1 and every place this port knowingly differs from `sqlparse`
0.6.1 / `sqlformat`.

## Not implemented (exit status 2)

This port prints `not implemented: <flag>` on stderr and exits 2 (a wrong guess
is worse than a refusal). The list mixes two kinds of upstream surface:

CLI flags the upstream `sqlformat` binary really accepts:

- `-a`, `--reindent_aligned` — needs `AlignedIndentFilter`
- `-s`, `--use_space_around_operators`
- `-l`, `--language`
- `--strip-comments`
- `--indent_after_first`, `--indent_columns`
- `--wrap_after`, `--comma_first`, `--compact`
- `--encoding`

Keyword arguments of `sqlparse.format()` that the upstream CLI never accepted
(`--indent_tabs`, `--right_margin`, `--truncate_strings`, `--truncate_char`,
`--output_format` are rejected there as `unrecognized arguments`, exit 2; we
also exit 2, with a different message):

- `--indent_tabs`, `--right_margin`, `--truncate_strings`, `--truncate_char`,
  `--output_format`

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

Runtime failures use upstream's `[ERROR] ...` line and exit 1. Option errors
use argparse-shaped text (`usage: sqlformat-rs [OPTIONS] FILE [FILE ...]` plus
`sqlformat-rs: error: argument -k/--keywords: invalid choice: ...`) and exit 2,
like upstream; the wording after `error:` is ours, not byte-identical.

## CI

`.github/workflows/` is intentionally absent: the account token used to push
lacks the `workflow` scope, so GitHub rejects any commit that adds a workflow
file. The gates in the README are run manually (and by `tools/parity.sh`).
