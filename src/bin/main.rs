//! `sqlformat` CLI: the flag subset this port implements, byte-compatible
//! output, and upstream's exit codes (`0` ok, `1` error, `2` usage).

use std::io::{Read, Write};
use std::process::ExitCode;

use sqlformat_rs::port::{format_sql, FormatOptions};

const USAGE: &str = "\
usage: sqlformat [-h] [-o OUTFILE] [--version] [--in-place] [-r]
                 [--indent_width INDENT_WIDTH] [-k {upper,lower,capitalize}]
                 [-i {upper,lower,capitalize}]
                 filename [filename ...]

positional arguments:
  filename              file to format, or '-' for stdin

options:
  -h, --help            show this help message and exit
  -o OUTFILE, --outfile OUTFILE
                        write output to a file
  --version             show program's version number and exit
  --in-place            rewrite the file in place
  -r, --reindent        reindent statements
  --indent_width INDENT_WIDTH
                        width of the indentation (default 2)
  -k, --keywords {upper,lower,capitalize}
  -i, --identifiers {upper,lower,capitalize}
";

/// Flags upstream has but this port does not implement yet.
const UNSUPPORTED: [&str; 18] = [
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
    "--wrap_after",
    "--comma_first",
    "--compact",
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
fn usage_error(message: &str) -> u8 {
    eprintln!("usage: sqlformat-rs [OPTIONS] FILE [FILE ...]");
    eprintln!("sqlformat-rs: error: {message}");
    2
}

fn case_flag(flag: &str) -> &'static str {
    match flag {
        "-k" | "--keywords" => "-k/--keywords",
        _ => "-i/--identifiers",
    }
}

fn case_value(flag: &str, value: &str) -> Result<String, u8> {
    match value {
        "upper" | "lower" | "capitalize" => Ok(value.to_string()),
        other => Err(usage_error(&format!(
            "argument {}: invalid choice: '{other}' (choose from 'upper', 'lower', 'capitalize')",
            case_flag(flag)
        ))),
    }
}

fn run() -> Result<(), u8> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut options = FormatOptions::default();
    let mut files: Vec<String> = Vec::new();
    let mut outfile: Option<String> = None;
    let mut in_place = false;
    let mut indent_width: Option<i64> = None;

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
                eprintln!("not implemented: {arg}");
                return Err(2);
            }
            "-k" | "--keywords" | "-i" | "--identifiers" | "-o" | "--outfile"
            | "--indent_width" => {
                index += 1;
                let value = args.get(index).ok_or_else(|| {
                    usage_error(&format!(
                        "argument {}: expected one argument",
                        case_flag(&arg)
                    ))
                })?;
                match arg.as_str() {
                    "-k" | "--keywords" => options.keyword_case = Some(case_value("-k", value)?),
                    "-i" | "--identifiers" => {
                        options.identifier_case = Some(case_value("-i", value)?)
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
            _ if arg.starts_with('-') && arg != "-" => {
                eprintln!("not implemented: {arg}");
                return Err(2);
            }
            _ => files.push(arg),
        }
        index += 1;
    }

    if files.is_empty() {
        return Err(usage_error(
            "the following arguments are required: filename",
        ));
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
        std::fs::read_to_string(filename)
            .map_err(|err| error(&format!("Failed to read {filename}: {err}")))?
    };

    let formatted = format_sql(&data, options)
        .map_err(|err| error(&format!("Failed to format {filename}: {err}")))?;

    if in_place {
        std::fs::write(filename, &formatted)
            .map_err(|err| error(&format!("Failed to open {filename}: {err}")))?;
        return Ok(());
    }
    if let Some(outfile) = outfile {
        std::fs::write(outfile, &formatted)
            .map_err(|err| error(&format!("Failed to open {outfile}: {err}")))?;
        return Ok(());
    }

    let mut stdout = std::io::stdout();
    stdout
        .write_all(formatted.as_bytes())
        .and_then(|()| stdout.flush())
        .map_err(|err| error(&format!("Failed to write stdout: {err}")))
}
