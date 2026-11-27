//! Print the parse tree of the last statement: class, type, cached len, text.
use sqlformat_rs::{group, lexer, splitter, tokens::Tk, tree::Tree};

fn show(tree: &Tree, id: sqlformat_rs::tree::Id, depth: usize) {
    let pad = "  ".repeat(depth);
    if let Some(class) = tree.class(id) {
        println!(
            "{pad}{:?} len={} text={:?}",
            class,
            tree.cached_len(id),
            trunc(&tree.text(id))
        );
        for child in tree.nodes[id].children.clone() {
            show(tree, child, depth + 1);
        }
    } else {
        println!(
            "{pad}{:?} len={} text={:?}",
            tree.ttype(id).unwrap_or(Tk::Error),
            tree.cached_len(id),
            trunc(&tree.text(id))
        );
    }
}

fn trunc(s: &str) -> String {
    let mut out: String = s.chars().take(34).collect();
    if s.chars().count() > 34 {
        out.push('…');
    }
    out
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: dump FILE");
    let text = std::fs::read_to_string(path).unwrap();
    let toks = lexer::tokenize(&text);
    let mut stmts = splitter::split(toks);
    let stmt = stmts.pop().expect("no statement");
    let mut tree = stmt;
    group::group(&mut tree).unwrap();
    show(&tree, tree.root, 0);
}
