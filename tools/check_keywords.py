#!/usr/bin/env python3
"""Assert src/kgenerated.rs KEYWORDS == sqlparse's merged table, shadowing included.

The merge order is the semantics (see gen_keywords.py): first table wins.
"""
import re
import sys
from pathlib import Path

from sqlparse import keywords as K

TABLES = [
    ("COMMON", K.KEYWORDS_COMMON),
    ("ORACLE", K.KEYWORDS_ORACLE),
    ("MYSQL", K.KEYWORDS_MYSQL),
    ("PLPGSQL", K.KEYWORDS_PLPGSQL),
    ("HQL", K.KEYWORDS_HQL),
    ("MSACCESS", K.KEYWORDS_MSACCESS),
    ("SNOWFLAKE", K.KEYWORDS_SNOWFLAKE),
    ("BIGQUERY", K.KEYWORDS_BIGQUERY),
    ("BASE", K.KEYWORDS),
]

def short(tok):
    """Token.Keyword.DML -> keyworddml, matching Tk::KeywordDml."""
    return str(tok).replace("Token.", "").replace(".", "").lower()


def rust_short(variant):
    return variant.lower()


def py_merged():
    out, shadowed = {}, []
    for name, table in TABLES:
        for key, tok in table.items():
            if key in out:
                if out[key] is not tok:
                    shadowed.append((key, out[key], tok))
                continue
            out[key] = tok
    return out, shadowed


def rust_table():
    src = Path("src/kgenerated.rs").read_text()
    body = src.split("pub static KEYWORDS", 1)[1].split("];", 1)[0]
    rows = re.findall(r'\("((?:[^"\\]|\\.)*)", Tk::(\w+)\)', body)
    unescape = lambda s: s.replace('\\\\', '\\').replace('\\"', '"')
    return [(unescape(k), t) for k, t in rows]


def main():
    merged, shadowed = py_merged()
    rust = rust_table()
    problems = 0

    dupes = len(rust) - len({k for k, _ in rust})
    print(f"python merged keys: {len(merged)}   declared across tables: "
          f"{sum(len(t) for _, t in TABLES)}   shadowed: {len(shadowed)}")
    print(f"rust KEYWORDS rows: {len(rust)}   duplicates: {dupes}")
    if dupes:
        problems += 1

    rmap = dict(rust)
    for key, tok in merged.items():
        want = short(tok)
        got = rust_short(rmap[key]) if key in rmap else None
        if got != want:
            problems += 1
            print(f"MISMATCH {key}: rust={got} python={want}")
    missing = set(merged) - set(rmap)
    extra = set(rmap) - set(merged)
    if missing:
        problems += 1
        print(f"MISSING in rust ({len(missing)}): {sorted(missing)[:10]}")
    if extra:
        problems += 1
        print(f"EXTRA in rust ({len(extra)}): {sorted(extra)[:10]}")

    # The three keys the proposal called out as type-changing by shadowing.
    for key in ("MAP", "TIMESTAMP", "CHARACTER"):
        print(f"  {key}: python={short(merged[key]) if key in merged else None} rust={rust_short(rmap[key]) if key in rmap else None}")

    print("OK" if problems == 0 else f"FAIL ({problems} problems)")
    return 0 if problems == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
