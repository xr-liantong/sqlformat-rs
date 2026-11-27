# sqlformat-rs

A Rust port of the `sqlformat` CLI from [sqlparse](https://github.com/andialbrecht/sqlparse)
(0.6.1, BSD-3-Clause). Same output bytes, same exit codes, ~19x faster on a
3 MB / 20,000-statement file and 2.5x lower peak RSS than CPython.

```
cargo build --release
target/release/sqlformat-rs -r schema.sql
```

## Scope (v0.2)

Implemented flags:

| flag | meaning |
| --- | --- |
| `-r`, `--reindent` | reindent; implies `--strip-whitespace` like upstream |
| `-k`, `--keywords upper\|lower\|capitalize` | keyword case |
| `-i`, `--identifiers upper\|lower\|capitalize` | identifier case |
| `--indent_width N` | indent width, default 2 |
| `--wrap_after N` | re-wrap lists after N columns (`int`; a negative value is a runtime error like upstream) |
| `--comma_first true` | break *before* the comma (`type=bool`: any non-empty value, even `false`, turns it on) |
| `--compact true` | no line break before `END` in `CASE` (`type=bool`, same rule) |
| `--strip-comments` | drop comments; SQL hints (`/*+ … */`, `--+ …`) are kept |
| `-o`, `--outfile FILE` | write to a file instead of stdout |
| `--in-place` | rewrite the input files |
| `-h`, `--help`, `--version` | help / upstream version string `0.6.1` |

Input is a file or `-` for stdin, exactly one file unless `--in-place`.
The help text, the argparse refusals (`usage:` plus `sqlformat: error: …`,
exit 2) and the `[ERROR] …` lines are byte-identical to upstream — including
the fixed `prog='sqlformat'`, the `[Errno N] strerror: 'path'` shape, and the
attached-value forms `-k=upper` / `-kupper` / `--keywords=upper`.
`tools/cli_matrix.sh` compares 26 such invocations against a live upstream and
pins 6 flags as *known gaps* (a gap that starts matching fails the script).

argparse's long-option prefixes work as upstream does: `--indent_w 4` means
`--indent_width 4`, and an ambiguous prefix fails the same way, listing the
candidates in definition order (`ambiguous option: --i could match --in-place,
--identifiers, --indent_width, --indent_after_first, --indent_columns`). A
filename does not have to be UTF-8; it is passed through as bytes.

Every other upstream flag (`-a`, `-s`, `-l`, `--indent_after_first`,
`--indent_columns`, `--encoding`) exits with status **2** and prints
`sqlformat: error: option <flag> is not implemented by this port`. See
[KNOWN-GAPS.md](KNOWN-GAPS.md).

### Limits

Upstream refuses to group more than 10,000 tokens in one list and more than 100
levels of nesting, as `SQLParseError`s; the CLI turns either into
`[ERROR] Failed to format <file>: …` with status **1** and an empty stdout. Both
guards are reproduced byte for byte — `fixtures/tokens-limit.sql` and the
`token_limit_matches_upstream` test cover the first, a 150-deep parenthesis
nesting covers the second. Grouping is lazy: without `-r` the same file passes
through untouched, exactly like upstream.

## Parity

Golden files are the upstream output for `fixtures/kwcase.sql` (858 B:
mixed-case keywords, comments, subqueries, window functions, `BETWEEN`,
`HAVING`). All byte-identical:

| invocation | bytes |
| --- | --- |
| `sqlformat-rs kwcase.sql` | 857 |
| `-r` | 1086 |
| `-k upper` | 857 |
| `-k lower` | 857 |
| `-i upper` | 857 |
| `--indent_width 4` | 857 |

`tests/parity.rs` compares these golden files (and the corpus checksum below)
when the `fixtures/` files are present.

## Performance

`fixtures/corpus-20k.sql` is 3,038,894 B / 20,000 statements, sha256
`4cf98f934f8ac22eeefb503c6b6c320e66b59b1f48ca3e31e152d363017fbf31`.
Command: `sqlformat -r corpus-20k.sql` (ours `-r`), output 3,578,891 B,
sha256 `c481988a319df7a5b6ce332c…` for both.

Box: Ubuntu 24.04, 2 vCPU (`nproc` = 2), CPython 3.12 + sqlparse 0.6.1. Three interleaved
runs each, wall clock and peak RSS (`baseline/run.py`, `ru_maxrss`):

| tool | wall (3 runs) | median | peak RSS |
| --- | --- | --- | --- |
| `sqlformat-rs` | 1.11 / 1.02 / 1.03 s | **1.03 s** | 12,728 KB |
| `sqlformat` (CPython) | 20.29 / 19.27 / 19.76 s | 19.76 s | 31,928 KB |

≈19.2x faster, 0.40x the memory. Measured on one machine, one input; not a
general benchmark.

### Measurement variance

The box is shared, so wall clock drifts between sessions. Two independent
sessions on the same corpus:

| session | `sqlformat-rs` median | `sqlformat` median | ratio |
| --- | --- | --- | --- |
| first | 1.03 s | 19.76 s | 19.2x |
| re-run (later, busier) | 2.01 s (1.59 / 2.01 / 2.10) | 23.32 s (23.32 / 24.79 / 23.04) | 11.6x |

Both runs produced the identical 3,578,891 B output. The acceptance floor is
3x, so the worst observed pair still clears it by ~3.9x; quote the ratio as
11.6x–19.2x rather than a single number.

The pipeline is streamed: one statement is lexed, grouped, stripped,
reindented and serialized before the next is read, so peak memory stays near
the input size rather than the whole token stream (`examples/mem.rs` reports
`hwm=13,068 KB` for the full 1,040,000-token file).

## Quality gates

One command runs every mechanical rule and prints one line per gate:

```
tools/gates.sh
```

```
PASS fmt        cargo fmt --check
PASS clippy     cargo clippy --release --all-targets -- -D warnings
PASS test       cargo test --release
PASS parity     tools/parity.sh            byte parity against the goldens
PASS keywords   tools/check_keywords.py    803 keys, merge order, shadowing
PASS tokens     tools/check_tokens.sh      token stream vs sqlparse, token by token
gates: 6 passed, 0 failed
```

The script exits non-zero if any gate fails and prints that gate's output. Its
failure path is itself checked: appending a badly formatted function makes it
report `FAIL fmt` and exit 1 (verified on 2026-11-19). All six are green on the
tagged commit. No `unwrap`/`expect`/`panic!` on any
path that can fail at runtime; the only `unwrap` uses are in tests and in
`examples/`.

## Layout

- `src/lexer.rs` — tokenizer; 43 rules compiled from `src/kgenerated.rs` plus 7
  hand-written lookaround rules.
- `src/kgenerated.rs` — 50 token regexes and 803 keywords, generated from the
  installed `sqlparse` by `tools/gen_keywords.py`.
- `src/tree.rs`, `src/splitter.rs`, `src/group.rs` — statement splitting and the
  25-pass grouping engine (`sqlparse/engine/grouping.py`).
- `src/filters.rs` — whitespace stripping, reindent, serializer.
- `src/port.rs` — the `sqlparse.format()` pipeline.
- `src/bin/main.rs` — CLI; `src/bin/txdump.rs` — token dump used for diffing
  against `tools/py_tokens.py`.

## License

BSD-3-Clause, see [LICENSE](LICENSE). The algorithm and keyword tables come
from sqlparse, copyright (c) 2016 Andi Albrecht.
