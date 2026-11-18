//! Byte parity against the upstream golden files.
//!
//! The fixtures are small and live in the repository, so a missing file is a
//! failure, not a skip: a green run always means the bytes were compared.

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

fn binary() -> PathBuf {
    // Cargo builds the binary and points integration tests at it.
    PathBuf::from(env!("CARGO_BIN_EXE_sqlformat-rs"))
}

fn run(args: &[&str]) -> Vec<u8> {
    let out = Command::new(binary())
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("failed to run {}: {error}", binary().display()));
    assert!(
        out.status.success(),
        "exit {:?} for {args:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    out.stdout
}

fn assert_golden(args: &[&str], golden: &str) {
    let fixture = fixtures();
    let expected = std::fs::read(fixture.join(golden)).unwrap_or_else(|error| {
        panic!("missing golden {}: {error}", fixture.join(golden).display())
    });
    let input = fixture.join("kwcase.sql");
    let mut full = args.to_vec();
    let input = input.to_str().expect("utf-8 path");
    full.push(input);
    let got = run(&full);
    assert_eq!(
        got.len(),
        expected.len(),
        "{args:?} vs {golden}: length differs (got {} want {})",
        got.len(),
        expected.len()
    );
    let at = got.iter().zip(&expected).position(|(a, b)| a != b);
    assert!(
        at.is_none(),
        "{args:?} vs {golden}: first difference at byte {:?}",
        at
    );
}

#[test]
fn default_matches_upstream() {
    assert_golden(&[], "golden-default.sql");
}

#[test]
fn reindent_matches_upstream() {
    assert_golden(&["-r"], "golden-r.sql");
}

#[test]
fn keyword_case_matches_upstream() {
    assert_golden(&["-k", "upper"], "golden-kupper.sql");
    assert_golden(&["-k", "lower"], "golden-klower.sql");
}

#[test]
fn identifier_case_matches_upstream() {
    assert_golden(&["-i", "upper"], "golden-iupper.sql");
}

#[test]
fn indent_width_without_reindent_matches_upstream() {
    assert_golden(&["--indent_width", "4"], "golden-iw4.sql");
}

#[test]
fn unsupported_flag_exits_two() {
    let out = Command::new(binary())
        .arg("-a")
        .arg(fixtures().join("kwcase.sql"))
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("not implemented: -a"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn corpus_matches_upstream_checksum() {
    let corpus = fixtures().join("corpus-20k.sql");
    let data = std::fs::read(&corpus)
        .unwrap_or_else(|error| panic!("missing {}: {error}", corpus.display()));
    assert_eq!(data.len(), 3_038_894);

    let out = run(&["-r", corpus.to_str().expect("utf-8 path")]);
    assert_eq!(out.len(), 3_578_891, "corpus output length");
    // Upstream `sqlformat -r` output for the same file.
    let expected = "c481988a319df7a5b6ce332c1c2baa5e";
    let got = sha256_hex_prefix(&out, expected.len());
    assert_eq!(got, expected, "corpus output checksum");
}

/// Minimal SHA-256, so the test suite needs no extra dependency.
fn sha256_hex_prefix(data: &[u8], hex_chars: usize) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut message = data.to_vec();
    let bit_len = (data.len() as u64) * 8;
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in message.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, word) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v[7] = v[6];
            v[6] = v[5];
            v[5] = v[4];
            v[4] = v[3].wrapping_add(t1);
            v[3] = v[2];
            v[2] = v[1];
            v[1] = v[0];
            v[0] = t1.wrapping_add(t2);
        }
        for (slot, value) in h.iter_mut().zip(v) {
            *slot = slot.wrapping_add(value);
        }
    }
    let hex: String = h.iter().map(|word| format!("{word:08x}")).collect();
    hex[..hex_chars.min(hex.len())].to_string()
}
