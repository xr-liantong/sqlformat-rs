//! `sqlformat` CLI: the flag subset this port implements, byte-compatible
//! output, and upstream's exit codes (`0` ok, `1` error, `2` usage).

use std::ffi::{OsStr, OsString};
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
const UNSUPPORTED: [&str; 11] = [
    "-a",
    "--reindent_aligned",
    "-s",
    "--use_space_around_operators",
    "-l",
    "--language",
    "--indent_after_first",
    "--indent_columns",
    "--right_margin",
    "--truncate_strings",
    "--truncate_char",
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
        "--encoding" => "--encoding",
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

/// Long options in argparse definition order. `ambiguous option: --i could
/// match ...` lists the candidates in this order, so this is a list, not a set.
const LONGS: [&str; 18] = [
    "--help",
    "--outfile",
    "--in-place",
    "--version",
    "--keywords",
    "--identifiers",
    "--language",
    "--strip-comments",
    "--reindent",
    "--indent_width",
    "--indent_after_first",
    "--indent_columns",
    "--reindent_aligned",
    "--use_space_around_operators",
    "--wrap_after",
    "--comma_first",
    "--compact",
    "--encoding",
];

/// Long options that consume the next token, or an `=value` suffix.
const TAKES_VALUE: [&str; 8] = [
    "--outfile",
    "--keywords",
    "--identifiers",
    "--indent_width",
    "--wrap_after",
    "--comma_first",
    "--compact",
    "--encoding",
];

/// argparse lets any long option be shortened to an unambiguous prefix; a
/// prefix that matches several is an error listing all of them.
fn resolve_long(given: &str) -> Result<Option<&'static str>, u8> {
    if let Some(hit) = LONGS.iter().find(|long| **long == given) {
        return Ok(Some(hit));
    }
    let matches: Vec<&'static str> = LONGS
        .iter()
        .copied()
        .filter(|long| long.starts_with(given))
        .collect();
    match matches.len() {
        0 => Ok(None),
        1 => Ok(Some(matches[0])),
        _ => Err(usage_error(&format!(
            "ambiguous option: {given} could match {}",
            matches.join(", ")
        ))),
    }
}

/// argparse accepts `--outfile=X`, the attached short forms `-kX` / `-k=X`,
/// and abbreviated long options, so all three are rewritten here into plain
/// tokens before the match in `run`. Unknown options keep their spelling:
/// argparse echoes them back whole in `unrecognized arguments`.
fn expand(args: Vec<OsString>) -> Result<Vec<OsString>, u8> {
    let mut out = Vec::with_capacity(args.len());
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].clone();
        index += 1;
        // A non-UTF-8 argument cannot be an option, so it is a filename.
        let Some(text) = arg.to_str() else {
            out.push(arg);
            continue;
        };
        if text == "--" {
            // End of options: everything after it is a filename.
            out.extend(args[index..].iter().cloned());
            break;
        }
        if let Some(rest) = text.strip_prefix("--") {
            let (name, inline) = match rest.split_once('=') {
                Some((name, value)) => (format!("--{name}"), Some(value.to_string())),
                None => (format!("--{rest}"), None),
            };
            match resolve_long(&name)? {
                Some(long) if TAKES_VALUE.contains(&long) => {
                    out.push(OsString::from(long));
                    if let Some(value) = inline {
                        out.push(OsString::from(value));
                    }
                }
                // A flag does not take a value: `--in-place=1` stays one token.
                Some(long) if inline.is_none() => out.push(OsString::from(long)),
                _ => out.push(OsString::from(text)),
            }
            continue;
        }
        if text.starts_with('-') && text.len() > 2 {
            let flag = text.as_bytes()[1] as char;
            if matches!(flag, 'k' | 'i' | 'o') {
                let rest = &text[2..];
                let value = rest.strip_prefix('=').unwrap_or(rest);
                out.push(OsString::from(format!("-{flag}")));
                out.push(OsString::from(value));
                continue;
            }
        }
        out.push(OsString::from(text));
    }
    Ok(out)
}

fn take_value(args: &[OsString], index: &mut usize, label: &str) -> Result<OsString, u8> {
    let value = args
        .get(*index)
        .ok_or_else(|| usage_error(&format!("argument {label}: expected one argument")))?;
    *index += 1;
    Ok(value.clone())
}

/// Case choices and widths must be UTF-8; only filenames may be arbitrary
/// bytes, which is why those keep their `OsString`.
fn text_value(value: &OsStr, label: &str) -> Result<String, u8> {
    match value.to_str() {
        Some(text) => Ok(text.to_owned()),
        None => Err(usage_error(&format!(
            "argument {label}: invalid value: {value:?}"
        ))),
    }
}

fn run() -> Result<(), u8> {
    // `args_os`: a filename is bytes on every platform upstream supports, and
    // `std::env::args()` panics on a non-UTF-8 argv entry (#6).
    let args = expand(std::env::args_os().skip(1).collect())?;
    let mut options = FormatOptions::default();
    let mut files: Vec<OsString> = Vec::new();
    let mut outfile: Option<OsString> = None;
    let mut in_place = false;
    let mut indent_width: Option<i64> = None;
    let mut wrap_after: Option<i64> = None;
    let mut unknown: Vec<String> = Vec::new();
    let mut codec: Option<String> = None;

    let mut index = 0;
    while index < args.len() {
        let raw = args[index].clone();
        index += 1;
        let Some(arg) = raw.to_str().map(str::to_owned) else {
            files.push(raw);
            continue;
        };
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
            "--strip-comments" => options.strip_comments = true,
            _ if UNSUPPORTED.contains(&arg.as_str()) => {
                return Err(usage_error(&format!(
                    "option {arg} is not implemented by this port"
                )));
            }
            "--wrap_after" | "--comma_first" | "--compact" => {
                let label = option_label(&arg);
                let value = take_value(&args, &mut index, label)?;
                let value = text_value(&value, label)?;
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
            "--encoding" => {
                let value = take_value(&args, &mut index, "--encoding")?;
                codec = Some(text_value(&value, "--encoding")?);
            }
            "-k" | "--keywords" | "-i" | "--identifiers" | "-o" | "--outfile"
            | "--indent_width" => {
                let label = option_label(&arg);
                let value = take_value(&args, &mut index, label)?;
                match arg.as_str() {
                    "-k" | "--keywords" => {
                        options.keyword_case = Some(case_value(label, &text_value(&value, label)?)?)
                    }
                    "-i" | "--identifiers" => {
                        options.identifier_case =
                            Some(case_value(label, &text_value(&value, label)?)?)
                    }
                    "-o" | "--outfile" => outfile = Some(value),
                    _ => match text_value(&value, label)?.parse::<i64>() {
                        Ok(width) => indent_width = Some(width),
                        Err(_) => {
                            return Err(usage_error(&format!(
                                "argument --indent_width: invalid int value: '{}'",
                                text_value(&value, label)?
                            )))
                        }
                    },
                }
            }
            _ if arg.starts_with('-') && arg != "-" => unknown.push(arg),
            _ => files.push(OsString::from(arg)),
        }
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

    // `open(..., encoding=NAME)` looks the codec up at read time, after
    // argparse has validated everything else.
    let codec = match codec.as_deref() {
        None => sqlformat_rs::encoding::Codec::Utf8,
        Some(name) => match sqlformat_rs::encoding::Codec::parse(name) {
            Some(codec) => codec,
            None => {
                eprintln!("LookupError: unknown encoding: {name}");
                return Err(1);
            }
        },
    };

    for filename in &files {
        process_file(filename, &options, outfile.as_deref(), in_place, codec)?;
    }
    Ok(())
}

fn process_file(
    filename: &OsStr,
    options: &FormatOptions,
    outfile: Option<&OsStr>,
    in_place: bool,
    codec: sqlformat_rs::encoding::Codec,
) -> Result<(), u8> {
    let shown = filename.to_string_lossy();
    let shown_out = outfile.map(|path| path.to_string_lossy());

    if filename == OsStr::new("-") && in_place {
        return Err(error("Cannot use --in-place with stdin"));
    }

    let data = if filename == OsStr::new("-") {
        let mut buffer = Vec::new();
        std::io::stdin()
            .read_to_end(&mut buffer)
            .map_err(|err| error(&format!("Failed to read stdin: {err}")))?;
        codec.decode(&buffer).map_err(decode_error)?
    } else {
        let bytes = std::fs::read(filename).map_err(|err| {
            error(&format!(
                "Failed to read {shown}: {}",
                io_message(&err, &shown)
            ))
        })?;
        // A decode failure is a traceback upstream: the message goes out
        // without the `[ERROR] ` prefix, and nothing is written.
        codec.decode(&bytes).map_err(decode_error)?
    };

    let formatted = format_sql(&data, options)
        .map_err(|err| error(&format!("Failed to format {shown}: {err}")))?;

    if in_place {
        write_encoded(filename, &formatted, codec, &shown)?;
        return Ok(());
    }
    if let (Some(outfile), Some(shown_out)) = (outfile, shown_out) {
        write_encoded(outfile, &formatted, codec, &shown_out)?;
        return Ok(());
    }

    let mut stdout = std::io::stdout();
    stdout
        .write_all(formatted.as_bytes())
        .and_then(|()| stdout.flush())
        .map_err(|err| error(&format!("Failed to write stdout: {err}")))
}

/// A decode failure is a traceback upstream; the last line names the
/// exception, so printing the same line keeps `tail -1` byte-identical.
fn decode_error(message: String) -> u8 {
    if message.starts_with("UnicodeError") {
        eprintln!("{message}");
    } else {
        eprintln!("UnicodeDecodeError: {message}");
    }
    1
}

/// Write `text` with `codec`, mirroring `open(outfile, 'w', encoding=...)`:
/// the file is created (and truncated) before the first character is encoded,
/// so an unencodable character leaves the bytes written so far behind.
fn write_encoded(
    path: &OsStr,
    text: &str,
    codec: sqlformat_rs::encoding::Codec,
    shown: &str,
) -> Result<(), u8> {
    let encoded = match codec.encode(text) {
        Ok(bytes) => bytes,
        Err((partial, message)) => {
            let _ = std::fs::write(path, &partial);
            eprintln!("{message}");
            return Err(1);
        }
    };
    std::fs::write(path, encoded).map_err(|err| {
        error(&format!(
            "Failed to open {shown}: {}",
            io_message(&err, shown)
        ))
    })
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
