//! `sqlformat` CLI: the flag subset this port implements, byte-compatible
//! output, and upstream's exit codes (`0` ok, `1` error, `2` usage).

use std::io::{Read, Write};
use std::process::ExitCode;

use sqlformat_rs::port::{format_sql, FormatOptions};

const USAGE: &str = "\
usage: sqlformat [OPTIONS] FILE [FILE ...]

Format FILE according to OPTIONS. Use \"-\" as FILE to read from stdin.

positional arguments:
  filename              file(s) to format (use \"-\" for stdin)

options:
  -h, --help            show this help message and exit
  -o FILE, --outfile FILE
                        write output to FILE (defaults to stdout)
  --in-place            format files in-place (overwrite existing files)
  --version             show program's version number and exit

Formatting Options:
  -k CHOICE, --keywords CHOICE
                        change case of keywords, CHOICE is one of \"upper\",
                        \"lower\", \"capitalize\"
  -i CHOICE, --identifiers CHOICE
                        change case of identifiers, CHOICE is one of \"upper\",
                        \"lower\", \"capitalize\"
  -l LANG, --language LANG
                        output a snippet in programming language LANG, choices
                        are \"python\", \"php\"
  --strip-comments      remove comments
  -r, --reindent        reindent statements
  --indent_width INDENT_WIDTH
                        indentation width (defaults to 2 spaces)
  --indent_after_first  indent after first line of statement (e.g. SELECT)
  --indent_columns      indent all columns by indent_width instead of keyword
                        length
  -a, --reindent_aligned
                        reindent statements to aligned format
  -s, --use_space_around_operators
                        place spaces around mathematical operators
  --wrap_after WRAP_AFTER
                        Column after which lists should be wrapped
  --comma_first COMMA_FIRST
                        Insert linebreak before comma (default False)
  --compact COMPACT     Try to produce more compact output (default False)
  --encoding ENCODING   Specify the input encoding (default utf-8)
";

/// Flags upstream has but this port does not implement yet.
const UNSUPPORTED: [&str; 15] = [
    "-a",
    "--reindent_aligned",
    "-s",
    "--use_space_around_operators",
    "-l",
    "--language",
    "--strip-comments",
    "--indent_after_first",
    "--indent_columns",
    "--indent_tabs",
    "--encoding",
    "--right_margin",
    "--truncate_strings",
    "--truncate_char",
    "--output_format",
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => ExitCode::from(code),
    }
}

fn error(message: &str) -> u8 {
    eprintln!("[ERROR] {message}");
    1
}

/// argparse-style refusal: usage line plus `error:` line, exit code 2.
/// Upstream passes `prog='sqlformat'` to ArgumentParser, so the name is fixed
/// no matter what the binary is called; a byte comparison therefore needs no
/// rename, unlike xlsx2csv's argparse.
fn usage_error(message: &str) -> u8 {
    eprintln!("usage: sqlformat [OPTIONS] FILE [FILE ...]");
    eprintln!("sqlformat: error: {message}");
    2
}

/// argparse's `argument <label>: ...` label for an option that takes a value.
fn option_label(flag: &str) -> &'static str {
    match flag {
        "-k" | "--keywords" => "-k/--keywords",
        "-i" | "--identifiers" => "-i/--identifiers",
        "-o" | "--outfile" => "-o/--outfile",
        "--indent_width" => "--indent_width",
        "--wrap_after" => "--wrap_after",
        "--comma_first" => "--comma_first",
        _ => "--compact",
    }
}

fn case_value(label: &str, value: &str) -> Result<String, u8> {
    match value {
        "upper" | "lower" | "capitalize" => Ok(value.to_string()),
        other => Err(usage_error(&format!(
            "argument {label}: invalid choice: '{other}' (choose from 'upper', 'lower', 'capitalize')"
        ))),
    }
}

/// argparse accepts `--outfile=X` and the attached short forms `-kX` / `-k=X`,
/// so expand them into two tokens before the match below. Unknown options keep
/// their original spelling: argparse echoes them back whole.
fn expand(args: Vec<String>) -> Vec<String> {
    const VALUE_OPTS: [&str; 8] = [
        "--outfile",
        "--keywords",
        "--identifiers",
        "--indent_width",
        "--wrap_after",
        "--comma_first",
        "--compact",
        "--encoding",
    ];
    let mut out = Vec::with_capacity(args.len());
    for arg in args {
        if let Some(rest) = arg.strip_prefix("--") {
            if let Some((name, value)) = rest.split_once('=') {
                let long = format!("--{name}");
                if VALUE_OPTS.contains(&long.as_str()) {
                    out.push(long);
                    out.push(value.to_string());
                    continue;
                }
            }
            out.push(arg);
            continue;
        }
        if arg.starts_with('-') && arg.len() > 2 {
            let flag = arg.as_bytes()[1] as char;
            if matches!(flag, 'k' | 'i' | 'o') {
                let rest = &arg[2..];
                let value = rest.strip_prefix('=').unwrap_or(rest);
                out.push(format!("-{flag}"));
                out.push(value.to_string());
                continue;
            }
        }
        out.push(arg);
    }
    out
}

fn run() -> Result<(), u8> {
    let args = expand(std::env::args().skip(1).collect());
    let mut options = FormatOptions::default();
    let mut files: Vec<String> = Vec::new();
    let mut outfile: Option<String> = None;
    let mut in_place = false;
    let mut indent_width: Option<i64> = None;
    let mut wrap_after: Option<i64> = None;
    let mut unknown: Vec<String> = Vec::new();

    let mut index = 0;
    while index < args.len() {
        let arg = args[index].clone();
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(());
            }
            "--version" => {
                println!("{}", sqlformat_rs::UPSTREAM_VERSION);
                return Ok(());
            }
            "-r" | "--reindent" => options.reindent = true,
            "--in-place" => in_place = true,
            _ if UNSUPPORTED.contains(&arg.as_str()) => {
                return Err(usage_error(&format!(
                    "option {arg} is not implemented by this port"
                )));
            }
            "--wrap_after" | "--comma_first" | "--compact" => {
                index += 1;
                let value = args.get(index).ok_or_else(|| {
                    usage_error(&format!(
                        "argument {}: expected one argument",
                        option_label(&arg)
                    ))
                })?;
                match arg.as_str() {
                    "--wrap_after" => match value.parse::<i64>() {
                        Ok(width) => wrap_after = Some(width),
                        Err(_) => {
                            return Err(usage_error(&format!(
                                "argument --wrap_after: invalid int value: '{value}'"
                            )))
                        }
                    },
                    // argparse `type=bool`: any non-empty string is True, so
                    // `--comma_first false` switches it *on*, as upstream does.
                    "--comma_first" => options.comma_first = !value.is_empty(),
                    _ => options.compact = !value.is_empty(),
                }
            }
            _ if arg.starts_with("--wrap_after=") => {
                let value = &arg["--wrap_after=".len()..];
                match value.parse::<i64>() {
                    Ok(width) => wrap_after = Some(width),
                    Err(_) => {
                        return Err(usage_error(&format!(
                            "argument --wrap_after: invalid int value: '{value}'"
                        )))
                    }
                }
            }
            _ if arg.starts_with("--comma_first=") => {
                options.comma_first = !arg["--comma_first=".len()..].is_empty();
            }
            _ if arg.starts_with("--compact=") => {
                options.compact = !arg["--compact=".len()..].is_empty();
            }
            "-k" | "--keywords" | "-i" | "--identifiers" | "-o" | "--outfile"
            | "--indent_width" => {
                index += 1;
                let value = args.get(index).ok_or_else(|| {
                    usage_error(&format!(
                        "argument {}: expected one argument",
                        option_label(&arg)
                    ))
                })?;
                match arg.as_str() {
                    "-k" | "--keywords" => {
                        options.keyword_case = Some(case_value(option_label("-k"), value)?)
                    }
                    "-i" | "--identifiers" => {
                        options.identifier_case = Some(case_value(option_label("-i"), value)?)
                    }
                    "-o" | "--outfile" => outfile = Some(value.clone()),
                    _ => match value.parse::<i64>() {
                        Ok(width) => indent_width = Some(width),
                        Err(_) => {
                            return Err(usage_error(&format!(
                                "argument --indent_width: invalid int value: '{value}'"
                            )))
                        }
                    },
                }
            }
            _ if arg.starts_with("--indent_width=") => {
                let value = &arg["--indent_width=".len()..];
                match value.parse::<i64>() {
                    Ok(width) => indent_width = Some(width),
                    Err(_) => {
                        return Err(usage_error(&format!(
                            "argument --indent_width: invalid int value: '{value}'"
                        )))
                    }
                }
            }
            _ if arg.starts_with('-') && arg != "-" => unknown.push(arg),
            _ => files.push(arg),
        }
        index += 1;
    }

    if files.is_empty() {
        return Err(usage_error(
            "the following arguments are required: filename",
        ));
    }

    if !unknown.is_empty() {
        return Err(usage_error(&format!(
            "unrecognized arguments: {}",
            unknown.join(" ")
        )));
    }

    // `format()` validates the value, not argparse: a non-positive width is a
    // runtime error (exit 1), not a usage error (exit 2).
    if let Some(width) = indent_width {
        if width <= 0 {
            return Err(error(
                "Invalid options: indent_width requires a positive integer",
            ));
        }
        options.indent_width = width as usize;
    }
    if let Some(width) = wrap_after {
        if width < 0 {
            return Err(error(
                "Invalid options: wrap_after requires a positive integer",
            ));
        }
        options.wrap_after = width;
    }
    if files.len() > 1 {
        if outfile.is_some() {
            return Err(error("Cannot use -o/--outfile with multiple files"));
        }
        if !in_place {
            return Err(error("Multiple files require --in-place flag"));
        }
    }

    for filename in &files {
        process_file(filename, &options, outfile.as_deref(), in_place)?;
    }
    Ok(())
}

fn process_file(
    filename: &str,
    options: &FormatOptions,
    outfile: Option<&str>,
    in_place: bool,
) -> Result<(), u8> {
    if filename == "-" && in_place {
        return Err(error("Cannot use --in-place with stdin"));
    }

    let data = if filename == "-" {
        let mut buffer = String::new();
        std::io::stdin()
            .read_to_string(&mut buffer)
            .map_err(|err| error(&format!("Failed to read stdin: {err}")))?;
        buffer
    } else {
        std::fs::read_to_string(filename).map_err(|err| {
            error(&format!(
                "Failed to read {filename}: {}",
                io_message(&err, filename)
            ))
        })?
    };

    let formatted = format_sql(&data, options)
        .map_err(|err| error(&format!("Failed to format {filename}: {err}")))?;

    if in_place {
        std::fs::write(filename, &formatted).map_err(|err| {
            error(&format!(
                "Failed to open {filename}: {}",
                io_message(&err, filename)
            ))
        })?;
        return Ok(());
    }
    if let Some(outfile) = outfile {
        std::fs::write(outfile, &formatted).map_err(|err| {
            error(&format!(
                "Failed to open {outfile}: {}",
                io_message(&err, outfile)
            ))
        })?;
        return Ok(());
    }

    let mut stdout = std::io::stdout();
    stdout
        .write_all(formatted.as_bytes())
        .and_then(|()| stdout.flush())
        .map_err(|err| error(&format!("Failed to write stdout: {err}")))
}

/// CPython's `str(OSError)` shape, e.g. `[Errno 2] No such file or directory:
/// 'nope.sql'`. Upstream puts that straight into `[ERROR] Failed to read ...`.
fn io_message(err: &std::io::Error, path: &str) -> String {
    match err.raw_os_error() {
        Some(code) => format!("[Errno {code}] {}: '{path}'", strerror(code)),
        None => format!("{err}: '{path}'"),
    }
}

fn strerror(code: i32) -> &'static str {
    match code {
        1 => "Operation not permitted",
        2 => "No such file or directory",
        5 => "Input/output error",
        12 => "Cannot allocate memory",
        13 => "Permission denied",
        17 => "File exists",
        20 => "Not a directory",
        21 => "Is a directory",
        24 => "Too many open files",
        28 => "No space left on device",
        30 => "Read-only file system",
        36 => "File name too long",
        40 => "Too many levels of symbolic links",
        _ => "Unknown error",
    }
}
