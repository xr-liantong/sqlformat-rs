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

fn case_value(flag: &str, value: &str) -> Result<String, u8> {
    match value {
        "upper" | "lower" | "capitalize" => Ok(value.to_string()),
        other => Err(error(&format!("Invalid value for {flag}: '{other}'"))),
    }
}

fn run() -> Result<(), u8> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut options = FormatOptions::default();
    let mut files: Vec<String> = Vec::new();
    let mut outfile: Option<String> = None;
    let mut in_place = false;

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
                let value = args
                    .get(index)
                    .ok_or_else(|| error(&format!("argument {arg}: expected one argument")))?;
                match arg.as_str() {
                    "-k" | "--keywords" => {
                        options.keyword_case = Some(case_value("keyword_case", value)?)
                    }
                    "-i" | "--identifiers" => {
                        options.identifier_case = Some(case_value("identifier_case", value)?)
                    }
                    "-o" | "--outfile" => outfile = Some(value.clone()),
                    _ => match value.parse::<usize>() {
                        Ok(width) if width >= 1 => options.indent_width = width,
                        _ => return Err(error("indent_width requires a positive integer")),
                    },
                }
            }
            _ if arg.starts_with("--indent_width=") => {
                let value = &arg["--indent_width=".len()..];
                match value.parse::<usize>() {
                    Ok(width) if width >= 1 => options.indent_width = width,
                    _ => return Err(error("indent_width requires a positive integer")),
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
        eprintln!("[ERROR] the following arguments are required: filename");
        return Err(2);
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
