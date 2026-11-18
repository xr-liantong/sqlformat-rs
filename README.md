# sqlformat-rs

A Rust port of the `sqlformat` CLI from [sqlparse](https://github.com/andialbrecht/sqlparse)
(0.6.1, BSD-3-Clause). Same output bytes, same exit codes, ~19x faster on a
3 MB / 20,000-statement file and 2.5x lower peak RSS than CPython.

```
cargo build --release
target/release/sqlformat-rs -r schema.sql
```

## Scope (v0.1)

Implemented flags:

| flag | meaning |
| --- | --- |
| `-r`, `--reindent` | reindent; implies `--strip-whitespace` like upstream |
| `-k`, `--keywords upper\|lower\|capitalize` | keyword case |
| `-i`, `--identifiers upper\|lower\|capitalize` | identifier case |
| `--indent_width N` | indent width, default 2 |
| `-o`, `--outfile FILE` | write to a file instead of stdout |
| `--in-place` | rewrite the input files |
| `-h`, `--help`, `--version` | help / upstream version string `0.6.1` |

Input is a file or `-` for stdin, exactly one file unless `--in-place`.
Every other upstream flag (`-a`, `-s`, `-l`, `--strip-comments`,
`--indent_after_first`, `--indent_columns`, `--wrap_after`, `--comma_first`,
`--compact`, `--encoding`, …) exits with status **2** and prints
`not implemented: <flag>`. See [KNOWN-GAPS.md](KNOWN-GAPS.md).

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

Box: Ubuntu 24.04, 4 vCPU, CPython 3.12 + sqlparse 0.6.1. Three interleaved
runs each, wall clock and peak RSS (`baseline/run.py`, `ru_maxrss`):

| tool | wall (3 runs) | median | peak RSS |
| --- | --- | --- | --- |
| `sqlformat-rs` | 1.11 / 1.02 / 1.03 s | **1.03 s** | 12,728 KB |
| `sqlformat` (CPython) | 20.29 / 19.27 / 19.76 s | 19.76 s | 31,928 KB |

≈19.2x faster, 0.40x the memory. Measured on one machine, one input; not a
general benchmark.

The pipeline is streamed: one statement is lexed, grouped, stripped,
reindented and serialized before the next is read, so peak memory stays near
the input size rather than the whole token stream (`examples/mem.rs` reports
`hwm=13,068 KB` for the full 1,040,000-token file).

## Quality gates

```
cargo fmt --check
cargo clippy --release --all-targets -- -D warnings
cargo test --release
bash tools/parity.sh          # byte parity against the golden files
```

All four are green on the tagged commit. No `unwrap`/`expect`/`panic!` on any
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
