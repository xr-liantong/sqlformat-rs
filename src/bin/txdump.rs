use std::io::Read;

fn esc(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for c in v.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

fn main() {
    let path = std::env::args().nth(1).unwrap_or_default();
    let text = if path.is_empty() || path == "-" {
        let mut buf = String::new();
        if std::io::stdin().read_to_string(&mut buf).is_err() {
            eprintln!("cannot read stdin");
            std::process::exit(1);
        }
        buf
    } else {
        match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) => {
                eprintln!("cannot read {path}: {err}");
                std::process::exit(1);
            }
        }
    };

    for tok in sqlformat_rs::lexer::tokenize(&text) {
        println!("{}\t{}", tok.ttype.name(), esc(&tok.value));
    }
}
