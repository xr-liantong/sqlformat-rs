# Known gaps

Scope of v0.2 and every place this port knowingly differs from `sqlparse`
0.6.1 / `sqlformat`.

## Not implemented (exit status 2)

This port prints `sqlformat: error: option <flag> is not implemented by this
port` on stderr and exits 2 (a wrong guess is worse than a refusal).
`tools/cli_matrix.sh` pins each of these as a *known gap*: if one ever starts
matching upstream, the script fails so this file gets updated.

CLI flags the upstream `sqlformat` binary really accepts:

- `-a`, `--reindent_aligned` — needs `AlignedIndentFilter`
- `-s`, `--use_space_around_operators`
- `-l`, `--language`
- `--indent_after_first`, `--indent_columns`
- `--encoding`

Keyword arguments of `sqlparse.format()` that the upstream CLI never accepted
(`--indent_tabs`, `--right_margin`, `--truncate_strings`, `--truncate_char`,
`--output_format` are rejected there as `unrecognized arguments`, exit 2; since
v0.2 we print the same `unrecognized arguments: <flag>` line):

- `--indent_tabs`, `--right_margin`, `--truncate_strings`, `--truncate_char`,
  `--output_format`

`--wrap_after`, `--comma_first` and `--compact` landed in v0.2 and match
upstream on `fixtures/lists.sql` (five golden cases, including all three
together). Upstream types them as `int` and `bool`, so the option-arity and
validation quirks are reproduced too: `--wrap_after -3` is a runtime error
(exit 1, `Invalid options: wrap_after requires a positive integer`) while
`--comma_first` without a value is a usage error (exit 2), and any non-empty
string — even `false` — enables it. `-a` still needs `AlignedIndentFilter`.

## Fixed in v0.2

- `-r --wrap_after N` wraps at the same columns as upstream
  (`sqlf03-cte.sql` 708/651 B, `sqlf11-unicode.sql` 235 B): grouping now grows
  a reused group's cached value, as Python's `grp.value += …` does (#5).
- A non-UTF-8 filename no longer panics: argv is read with `args_os`, so the
  path stays bytes (`[ERROR]` lines show it lossily) (#6).
- Long options accept unambiguous prefixes and report ambiguity in argparse
  order; options upstream never accepted are `unrecognized arguments`, not
  "not implemented" (#7).
- `--strip-comments` is implemented, hints excepted.

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

Runtime failures use upstream's `[ERROR] ...` line and exit 1, including a
`--indent_width` that parses as an integer but is not positive (upstream
rejects that in `format()`, not in argparse, and so do we). Option errors use
argparse-shaped text and exit 2, byte-identical to upstream — `usage: sqlformat
[OPTIONS] FILE [FILE ...]`, `sqlformat: error: argument -k/--keywords: invalid
choice: 'bogus' (choose from 'upper', 'lower', 'capitalize')`, `unrecognized
arguments: --bogus --x2`, and `argument -o/--outfile: expected one argument`.
Upstream passes `prog='sqlformat'` to ArgumentParser, so that name is fixed and
a byte comparison needs no binary rename (unlike xlsx2csv's argparse). `I/O`
failures copy CPython's `str(OSError)`: `[Errno 2] No such file or directory:
'nope.sql'`. `tests/cli.rs` pins the same bytes without needing Python;
`tools/cli_matrix.sh` compares 26 invocations against a live upstream.

## CI

`.github/workflows/` is intentionally absent: the account token used to push
lacks the `workflow` scope, so GitHub rejects any commit that adds a workflow
file. The gates in the README are run manually (and by `tools/parity.sh`).
