# sqlformat-rs — v0.1 plan

Port of the `sqlformat` CLI shipped with `sqlparse` 0.6.1 (MIT).
Spec: `rust-scout/specs/sqlformat-keywords.md` (14,261 B) — the only input.

## Scope (v0.1)

CLI only, stdin/stdout only:

| flag | meaning |
|---|---|
| `-r`, `--reindent` | reindent statements; **without it the CLI echoes the input unchanged** |
| `-k`, `--keywords` | `upper` / `lower` / `capitalize` — case of the `Keyword` subtree, **not** a keyword table |
| `-i`, `--identifiers` | `upper` / `lower` / `capitalize` — case of the `Name` subtree |
| `--indent_width N` | long option only, default 2 |

Verified against the installed CLI (`sqlparse 0.6.1`, `sqlformat -h`), not
against the summary: `-i` is **identifiers**, not indent width; indent width
is `--indent_width` with no short form.

Out of scope: `parse()` / Token API, `--strip-comments`, file arguments,
`-o/--outfile`, `--in-place`, `-l/--language`, `-a`, `-s`, `--wrap_after`,
`--comma_first`, `--compact`, `--indent_after_first`, `--indent_columns`,
`--encoding`. Everything out of scope exits 2 with an explicit
`not implemented` line, never silently ignored.

## Non-negotiable facts from the spec

1. **No dialect switch.** `Lexer.default_initialization()` merges nine tables
   in a fixed order: COMMON → ORACLE → MYSQL → PLPGSQL → HQL → MSACCESS →
   SNOWFLAKE → BIGQUERY → BASE. The target is the resulting **single 803-key
   table**, in the order that merge produces it.
2. **`-k/--keywords` is `{upper, lower, capitalize}`**, applied to the
   `Keyword` subtree only (`Name.Builtin` is *not* touched; `-i` touches the
   `Name` subtree).
3. 813 declared keys across the nine tables, 803 after shadowing; 10 keys are
   declared twice, and of those only 3 change type (re-derived on the box with
   `tools/check_keywords.py`, which asserts the table against `sqlparse`
   itself — the spec's "913 keys / 10 shadowings" was off):
   `MAP`/`TIMESTAMP` → `Name.Builtin` (HQL wins), `CHARACTER` → `Keyword`
   (PLPGSQL wins). **No `HashMap` insert-overwrite** — the merge order is the
   semantics; a map that overwrites silently inverts these ten.
4. **50 `SQL_REGEX` entries, order is the spec.** Matching is anchored at the
   current offset, first match wins; the entries cannot be folded into one
   alternation.
5. Six lookarounds (`(?<!\w)`, `(?<=\.)`, `(?![_A-ZÀ-Ü])`, `(?<![\w\])])`)
   must be hand-written in Rust; `regex` supports neither look-behind nor the
   Unicode classes the spec assumes.
6. Python `\w` is `isalnum()`, Rust `\w` also matches combining marks and
   connector punctuation — compare with an explicit predicate, not `\w`.

## Baseline and status (2026-11-19, box)

Chief's baseline for the brief: 3.1 MB / 20,000 statements → **28.30 s**,
output 4,895,557 B, RSS 36,652 KB. That was a different corpus (its output is
1.37x larger per statement), so it is the sanity check, not the acceptance
number.

Our corpus: `fixtures/corpus-20k.sql`, 3,038,894 B / 20,000 single-line
`SELECT`, sha256 `4cf98f93…`, upstream `-r` output 3,578,891 B
(sha256 `c481988a…`). Three runs each:

| tool | median wall | peak RSS |
|---|---|---|
| `sqlformat` 0.6.1 (CPython 3.12) | 23.318 s (23.32 / 24.79 / 23.04) | 31,696 KB |
| `sqlformat-rs` (this port) | 2.009 s (1.59 / 2.01 / 2.10) | 15,104 KB |

11.6x on wall, 0.48x RSS; an earlier session measured 19.2x (1.03 vs 19.76 s)
on the same corpus, so the box's load varies — quote 11.6x–19.2x. The 3x
acceptance floor is met by the worst run (3.9x).

Milestones 1–5 above are **done**: table generated and asserted, lexer
token-identical over 1,040,000 tokens, `-r` byte-identical on the corpus,
7/7 golden parity, 33 tests, `fmt`/`clippy -D warnings` clean, tag `v0.1.0`
on `xr-liantong/sqlformat-rs`.

## Acceptance

* Byte-identical stdout and exit code against the upstream CLI on every
  fixture (stdin, no files): `cmp` on stdout, `$?` equality.
* Fixtures: 1 MB single query, multi-statement file, comments with keywords,
  quoted identifiers, dollar-quoted bodies, non-ASCII identifiers.
* Baseline: upstream CLI three runs each, median wall + peak RSS.
* Gate: median wall >= 3x faster, RSS <= 2x upstream, plus `fmt --check`,
  `clippy -D warnings`, `cargo test` all 0.
* No `unwrap/expect/panic!` outside tests; line count <= 3x the upstream
  implementation.

## Milestones (one reportable result per ~40 min)

1. upstream baseline measured on box (sqlparse 0.6.1 installed in a venv)
2. keyword table generated and unit-tested (803 keys; the 3 retyped keys
   `MAP`/`TIMESTAMP`/`CHARACTER` asserted by name)
3. lexer: 50 regexes ported, token stream identical to Python on the fixtures
4. formatter: `-r` output byte-identical on fixture 1
5. all fixtures byte-identical + gates green + tag v0.1.0

## Known risks (ranked)

1. ordered regexes must stay ordered; any "optimisation" that merges them
   changes results
2. lookaround rewriting is where correctness usually dies — each gets its own
   test with the failing input from the spec
3. `\w` semantics (see above)
4. grader/case options apply to subtrees, not to the whole token stream
