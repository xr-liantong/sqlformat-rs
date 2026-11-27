//! The CLI text matrix: help, argparse refusals and `[ERROR]` lines must match
//! upstream `sqlformat` (sqlparse 0.6.1) byte for byte.
//!
//! `tests/data/upstream-help.txt` is the exact stdout of
//! `python3 -m sqlparse --help` on the baseline box (1,859 B, md5
//! `fd15244636cdfdb01a44e899f3da4641`). Upstream passes `prog='sqlformat'` to
//! ArgumentParser, so that string is fixed and no binary rename is needed —
//! unlike xlsx2csv, whose argparse uses `argv[0]`.
//!
//! `tools/cli_matrix.sh` runs the same matrix against a live upstream install;
//! these tests pin the same bytes without needing Python.

use std::path::{Path, PathBuf};
use std::process::Command;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_sqlformat-rs"))
}

fn run(args: &[&str]) -> (String, String, i32) {
    let out = Command::new(binary()).args(args).output().expect("spawn");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

#[test]
fn help_matches_upstream_byte_for_byte() {
    let expected =
        std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/upstream-help.txt"))
            .expect("tests/data/upstream-help.txt");
    let (stdout, stderr, rc) = run(&["--help"]);
    assert_eq!(rc, 0);
    assert_eq!(stderr, "");
    assert_eq!(
        stdout.as_bytes(),
        expected.as_slice(),
        "help text drifted from upstream ({} vs {} bytes)",
        stdout.len(),
        expected.len()
    );
}

#[test]
fn usage_refusal_uses_the_fixed_prog_name() {
    let (stdout, stderr, rc) = run(&[]);
    assert_eq!(rc, 2);
    assert_eq!(stdout, "");
    assert_eq!(
        stderr,
        "usage: sqlformat [OPTIONS] FILE [FILE ...]\n\
         sqlformat: error: the following arguments are required: filename\n"
    );
}

#[test]
fn unknown_options_are_reported_like_argparse() {
    // argparse echoes every unrecognised token, in order, on one line.
    let (_, stderr, rc) = run(&["--bogus", "--x2", "a.sql"]);
    assert_eq!(rc, 2);
    assert_eq!(
        stderr,
        "usage: sqlformat [OPTIONS] FILE [FILE ...]\n\
         sqlformat: error: unrecognized arguments: --bogus --x2\n"
    );
    // Required positionals are checked before unrecognised extras.
    let (_, stderr, rc) = run(&["--bogus"]);
    assert_eq!(rc, 2);
    assert!(stderr.contains("the following arguments are required: filename"));
}

#[test]
fn expected_one_argument_names_the_flag_that_was_used() {
    for (flag, label) in [
        ("-o", "-o/--outfile"),
        ("-k", "-k/--keywords"),
        ("-i", "-i/--identifiers"),
        ("--indent_width", "--indent_width"),
        ("--wrap_after", "--wrap_after"),
    ] {
        let (_, stderr, rc) = run(&[flag]);
        assert_eq!(rc, 2, "{flag}");
        assert!(
            stderr.ends_with(&format!("argument {label}: expected one argument\n")),
            "{flag}: {stderr}"
        );
    }
}

#[test]
fn attached_values_and_equals_forms_parse_like_argparse() {
    // `-kVALUE`, `-k=VALUE` and `--keywords=VALUE` are all accepted upstream.
    for args in [
        vec!["-kupper", "x.sql"],
        vec!["-k=upper", "x.sql"],
        vec!["--keywords=upper", "x.sql"],
    ] {
        let (stdout, _, rc) = run(&args);
        // x.sql does not exist: the parse succeeded, the read failed.
        assert_eq!(rc, 1, "{args:?}");
        assert_eq!(stdout, "", "{args:?}");
    }
}

#[test]
fn io_failures_use_cpython_errno_text() {
    let (_, stderr, rc) = run(&["definitely-missing.sql"]);
    assert_eq!(rc, 1);
    assert_eq!(
        stderr,
        "[ERROR] Failed to read definitely-missing.sql: \
         [Errno 2] No such file or directory: 'definitely-missing.sql'\n"
    );

    let dir = std::env::temp_dir();
    let (_, stderr, rc) = run(&["-o", dir.to_str().unwrap(), "definitely-missing.sql"]);
    // The read fails first, so this pins the read message, not the open one.
    assert_eq!(rc, 1);
    assert!(stderr.starts_with("[ERROR] Failed to read"));
}
