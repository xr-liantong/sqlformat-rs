//! Peak-memory probe for the streamed pipeline: report VmHWM per stage.
//!
//! Usage: `cargo run --release --example mem -- <file> <stage>`

use sqlformat_rs::{filters, group, lexer, splitter, tree::Tree};

fn vm_hwm_kb() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmHWM:") {
            return rest
                .trim()
                .trim_end_matches(" kB")
                .trim()
                .parse()
                .unwrap_or(0);
        }
    }
    0
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = &args[0];
    let stage = args.get(1).map(String::as_str).unwrap_or("full");
    let sql = std::fs::read_to_string(path).unwrap();

    let mut lexer = lexer::Lexer::new(&sql);
    let mut splitter = splitter::Splitter::default();
    let mut count = 0usize;
    let mut statements = 0usize;
    let mut strip = filters::StripWhitespace;
    let mut reindent = filters::ReindentFilter::new(2);
    let mut out = String::new();

    while let Some((ttype, start, end)) = lexer.next_token() {
        count += 1;
        let value = &sql[start..end];
        let Some(statement) = splitter.feed(ttype, value) else {
            continue;
        };
        statements += 1;
        if stage == "tok" {
            continue;
        }
        let mut tree = Tree::from_tokens(statement);
        let root = tree.root;
        if stage == "split" {
            std::mem::forget(tree);
            continue;
        }
        group::group(&mut tree).unwrap();
        if stage == "group" {
            std::mem::forget(tree);
            continue;
        }
        strip.process(&mut tree, root);
        if stage == "strip" {
            std::mem::forget(tree);
            continue;
        }
        reindent.process(&mut tree, root);
        if stage != "reindent" {
            out.push_str(&filters::serialize(&tree, root));
        }
    }
    println!(
        "{stage:<8} {count:>7} tokens {statements:>6} stmts hwm={} KB out={} B",
        vm_hwm_kb(),
        out.len()
    );
}
