# sqlformat-rs v0.1 — streaming rewrite closes the RSS gate

Date: 2026-11-19. Box `/workspace/sqlformat-rs`, rustc 1.100.0.

## Problem

First complete port produced byte-identical output but peaked at
151,620 KB RSS on `fixtures/corpus-20k.sql` (3,038,894 B, 1,040,000 tokens,
20,000 statements) against 31,800 KB for CPython `sqlformat`. The plan's gate
(RSS <= 2x upstream) failed at 4.9x.

Cause, measured with `examples/mem.rs` (`VmHWM`):

| stage | peak |
| --- | --- |
| tokenize only | 74,308 KB |
| + split all statements into 20k trees | 151,624 KB |
| full pipeline (trees leaked) | 276,164 KB |

`splitter::split` collected every statement's `Vec<Token>` before building any
tree, so the whole token stream and the whole node arena were alive at once.

## Fix

- `lexer::Lexer<'a>`: cursor over the input yielding `(Tk, start, end)` byte
  ranges; `tokenize()` kept as the collecting wrapper for tests and `txdump`.
- `splitter::Splitter::feed(ttype, value) -> Option<Vec<Token>>` and
  `finish()`: hands out one statement at a time.
- `port::format_sql`: lex -> split -> group -> strip -> reindent -> serialize
  for one statement, then drop it. Case filters applied per token before the
  splitter (same order as the upstream preprocess filters).

Output is unchanged: 3,578,891 B, sha256 `c481988a319df7a5b6ce332c...`.

## Gates

- parity: 7/7 (`tools/parity.sh`), goldens byte-identical.
- `cargo test --release`: 29 passed (22 unit + 7 integration).
- `cargo clippy --release --all-targets -- -D warnings`: clean.
- `cargo fmt --check`: clean.

Interleaved on the box, `sqlformat -r fixtures/corpus-20k.sql`:

| tool | wall | median | peak RSS |
| --- | --- | --- | --- |
| sqlformat-rs | 1.11 / 1.02 / 1.03 s | 1.03 s | 12,728 KB |
| sqlformat (CPython 3.12, sqlparse 0.6.1) | 20.29 / 19.27 / 19.76 s | 19.76 s | 31,928 KB |

19.2x faster, 0.40x memory. Gate target was <= 2x memory; achieved 0.4x.
