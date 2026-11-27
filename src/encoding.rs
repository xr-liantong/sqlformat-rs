//! A small, dependency-free subset of CPython's `codecs` module.
//!
//! `sqlformat --encoding NAME` opens the input with `encoding=NAME` and writes
//! the output with the same encoding, so a byte-faithful port needs decode and
//! encode for the codecs people actually keep `.sql` files in. Everything here
//! mirrors CPython's behaviour and error text; anything outside the table is
//! refused by `Codec::parse` rather than guessed at.

use std::fmt::Write as _;

/// The codecs `--encoding` accepts. Names follow the CPython spellings so the
/// error text and the round trip agree with `sqlparse`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    Utf8,
    Ascii,
    Latin1,
    Cp1252,
    Utf16,
    Utf16Le,
    Utf16Be,
}

/// The cp1252 code points for bytes 0x80..=0x9F. `None` is an undefined byte,
/// which CPython reports as `character maps to <undefined>`.
const CP1252_HIGH: [Option<char>; 32] = [
    Some('\u{20ac}'), // 0x80
    None,             // 0x81
    Some('\u{201a}'), // 0x82
    Some('\u{0192}'), // 0x83
    Some('\u{201e}'), // 0x84
    Some('\u{2026}'), // 0x85
    Some('\u{2020}'), // 0x86
    Some('\u{2021}'), // 0x87
    Some('\u{02c6}'), // 0x88
    Some('\u{2030}'), // 0x89
    Some('\u{0160}'), // 0x8A
    Some('\u{2039}'), // 0x8B
    Some('\u{0152}'), // 0x8C
    None,             // 0x8D
    Some('\u{017d}'), // 0x8E
    None,             // 0x8F
    None,             // 0x90
    Some('\u{2018}'), // 0x91
    Some('\u{2019}'), // 0x92
    Some('\u{201c}'), // 0x93
    Some('\u{201d}'), // 0x94
    Some('\u{2022}'), // 0x95
    Some('\u{2013}'), // 0x96
    Some('\u{2014}'), // 0x97
    Some('\u{02dc}'), // 0x98
    Some('\u{2122}'), // 0x99
    Some('\u{0161}'), // 0x9A
    Some('\u{203a}'), // 0x9B
    Some('\u{0153}'), // 0x9C
    None,             // 0x9D
    Some('\u{017e}'), // 0x9E
    Some('\u{0178}'), // 0x9F
];

impl Codec {
    /// Match a codec name the way `open(..., encoding=...)` would. `None`
    /// means CPython raises `LookupError: unknown encoding: NAME`, and so does
    /// the caller.
    pub fn parse(name: &str) -> Option<Codec> {
        let normalized = name.trim().to_ascii_lowercase().replace('_', "-");
        match normalized.as_str() {
            "utf-8" | "utf8" | "u8" | "utf" => Some(Codec::Utf8),
            "ascii" | "us-ascii" | "646" | "ansi-x3.4-1968" => Some(Codec::Ascii),
            "latin-1" | "latin1" | "latin" | "iso-8859-1" | "iso8859-1" | "8859" | "cp819"
            | "l1" | "iso-latin-1" => Some(Codec::Latin1),
            "cp1252" | "windows-1252" | "1252" => Some(Codec::Cp1252),
            "utf-16" | "utf16" | "u16" => Some(Codec::Utf16),
            "utf-16-le" | "utf16-le" | "utf16le" | "utf-16le" => Some(Codec::Utf16Le),
            "utf-16-be" | "utf16-be" | "utf16be" | "utf-16be" => Some(Codec::Utf16Be),
            _ => None,
        }
    }

    /// The name CPython puts in an exception message.
    pub fn name(self) -> &'static str {
        match self {
            Codec::Utf8 => "utf-8",
            Codec::Ascii => "ascii",
            Codec::Latin1 => "latin-1",
            Codec::Cp1252 => "charmap",
            Codec::Utf16 => "utf-16",
            Codec::Utf16Le => "utf-16-le",
            Codec::Utf16Be => "utf-16-be",
        }
    }

    /// Decode `bytes`, returning CPython's `UnicodeDecodeError` text on
    /// failure. `latin-1` cannot fail: every byte is a code point.
    pub fn decode(self, bytes: &[u8]) -> Result<String, String> {
        match self {
            Codec::Utf8 => decode_utf8(bytes),
            Codec::Ascii => decode_ascii(bytes),
            Codec::Latin1 => Ok(bytes.iter().map(|&byte| byte as char).collect()),
            Codec::Cp1252 => decode_cp1252(bytes),
            Codec::Utf16 => decode_utf16(bytes, Utf16Order::Bom),
            Codec::Utf16Le => decode_utf16(bytes, Utf16Order::Little),
            Codec::Utf16Be => decode_utf16(bytes, Utf16Order::Big),
        }
    }

    /// Encode `text`. On failure return the bytes written before the first
    /// unencodable character plus CPython's `UnicodeEncodeError` text.
    pub fn encode(self, text: &str) -> Result<Vec<u8>, (Vec<u8>, String)> {
        match self {
            Codec::Utf8 => Ok(text.as_bytes().to_vec()),
            Codec::Ascii => encode_single_byte(text, self, |ch| {
                if (ch as u32) < 0x80 {
                    Some(ch as u8)
                } else {
                    None
                }
            }),
            Codec::Latin1 => encode_single_byte(text, self, |ch| {
                if (ch as u32) < 0x100 {
                    Some(ch as u8)
                } else {
                    None
                }
            }),
            Codec::Cp1252 => encode_single_byte(text, self, |ch| {
                if (ch as u32) < 0x80 {
                    return Some(ch as u8);
                }
                CP1252_HIGH
                    .iter()
                    .position(|&mapped| mapped == Some(ch))
                    .map(|offset| 0x80 + offset as u8)
                    .or_else(|| {
                        let code = ch as u32;
                        if (0xA0..=0xFF).contains(&code) {
                            Some(code as u8)
                        } else {
                            None
                        }
                    })
            }),
            Codec::Utf16 | Codec::Utf16Le | Codec::Utf16Be => {
                let mut out = Vec::with_capacity(text.len() * 2 + 2);
                if self == Codec::Utf16 {
                    out.extend_from_slice(&[0xFF, 0xFE]);
                }
                let big = self == Codec::Utf16Be;
                for ch in text.chars() {
                    let mut units = [0u16; 2];
                    for unit in ch.encode_utf16(&mut units) {
                        let bytes = if big {
                            unit.to_be_bytes()
                        } else {
                            unit.to_le_bytes()
                        };
                        out.extend_from_slice(&bytes);
                    }
                }
                Ok(out)
            }
        }
    }
}

fn decode_utf8(bytes: &[u8]) -> Result<String, String> {
    let mut text = String::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte < 0x80 {
            text.push(byte as char);
            index += 1;
            continue;
        }
        let extra = match byte {
            0xC2..=0xDF => 1,
            0xE0..=0xEF => 2,
            0xF0..=0xF4 => 3,
            _ => {
                return Err(format!(
                    "'utf-8' codec can't decode byte 0x{byte:02x} in position {index}: \
                     invalid start byte"
                ))
            }
        };
        if index + extra >= bytes.len() {
            return Err(format!(
                "'utf-8' codec can't decode bytes in position {index}-{}: \
                 unexpected end of data",
                bytes.len() - 1
            ));
        }
        let mut value = (byte as u32) & (0x7F >> extra);
        for offset in 1..=extra {
            let continuation = bytes[index + offset];
            if continuation & 0xC0 != 0x80 {
                return Err(format!(
                    "'utf-8' codec can't decode byte 0x{byte:02x} in position {index}: \
                     invalid continuation byte"
                ));
            }
            value = (value << 6) | (continuation as u32 & 0x3F);
        }
        match char::from_u32(value) {
            Some(ch) => text.push(ch),
            None => {
                return Err(format!(
                    "'utf-8' codec can't decode bytes in position {index}-{}: \
                     invalid continuation byte",
                    index + extra
                ))
            }
        }
        index += 1 + extra;
    }
    Ok(text)
}

fn decode_ascii(bytes: &[u8]) -> Result<String, String> {
    let mut text = String::with_capacity(bytes.len());
    for (index, &byte) in bytes.iter().enumerate() {
        if byte < 0x80 {
            text.push(byte as char);
        } else {
            return Err(format!(
                "'ascii' codec can't decode byte 0x{byte:02x} in position {index}: \
                 ordinal not in range(128)"
            ));
        }
    }
    Ok(text)
}

fn decode_cp1252(bytes: &[u8]) -> Result<String, String> {
    let mut text = String::with_capacity(bytes.len());
    for (index, &byte) in bytes.iter().enumerate() {
        match byte {
            0x00..=0x7F | 0xA0..=0xFF => text.push(byte as char),
            0x80..=0x9F => match CP1252_HIGH[(byte - 0x80) as usize] {
                Some(ch) => text.push(ch),
                None => {
                    return Err(format!(
                        "'charmap' codec can't decode byte 0x{byte:02x} in position \
                         {index}: character maps to <undefined>"
                    ))
                }
            },
        }
    }
    Ok(text)
}

enum Utf16Order {
    Bom,
    Little,
    Big,
}

fn decode_utf16(bytes: &[u8], order: Utf16Order) -> Result<String, String> {
    let (big, start) = match order {
        Utf16Order::Big => (true, 0),
        Utf16Order::Little => (false, 0),
        Utf16Order::Bom => match bytes {
            [0xFF, 0xFE, ..] => (false, 2),
            [0xFE, 0xFF, ..] => (true, 2),
            _ => {
                return Err("'utf-16' codec can't decode bytes in position 0-1: \
                            missing byte order mark"
                    .to_string())
            }
        },
    };
    let body = &bytes[start..];
    if body.len() % 2 != 0 {
        return Err(format!(
            "'utf-16' codec can't decode bytes in position {}-{}: truncated data",
            bytes.len() - 1,
            bytes.len() - 1
        ));
    }
    let units = body.chunks_exact(2).map(|pair| {
        if big {
            u16::from_be_bytes([pair[0], pair[1]])
        } else {
            u16::from_le_bytes([pair[0], pair[1]])
        }
    });
    let mut text = String::new();
    for decoded in char::decode_utf16(units) {
        match decoded {
            Ok(ch) => text.push(ch),
            Err(err) => {
                let position = (err.unpaired_surrogate() as usize) * 2;
                return Err(format!(
                    "'utf-16' codec can't decode bytes in position {position}-{}: \
                     illegal UTF-16 surrogate",
                    position + 1
                ));
            }
        }
    }
    Ok(text)
}

fn encode_single_byte<F>(text: &str, codec: Codec, map: F) -> Result<Vec<u8>, (Vec<u8>, String)>
where
    F: Fn(char) -> Option<u8>,
{
    let mut out = Vec::with_capacity(text.len());
    for (position, ch) in text.char_indices() {
        match map(ch) {
            Some(byte) => out.push(byte),
            None => {
                let reason = if codec == Codec::Ascii {
                    "ordinal not in range(128)"
                } else if codec == Codec::Cp1252 {
                    "character maps to <undefined>"
                } else {
                    "ordinal not in range(256)"
                };
                return Err((
                    out,
                    format!(
                        "'{}' codec can't encode character '{}' in position {position}: {reason}",
                        codec.name(),
                        escape_char(ch)
                    ),
                ));
            }
        }
    }
    Ok(out)
}

/// `\u4e2d` / `\xe9` / `\n`, as CPython prints it inside the quotes of a
/// `UnicodeEncodeError`.
pub fn escape_char(ch: char) -> String {
    match ch {
        '\t' => "\\t".to_string(),
        '\n' => "\\n".to_string(),
        '\r' => "\\r".to_string(),
        '\\' => "\\\\".to_string(),
        '\'' => "\\'".to_string(),
        _ if (ch as u32) < 0x20 || (ch as u32) == 0x7F => format!("\\x{:02x}", ch as u32),
        _ if (ch as u32) < 0x100 => format!("\\x{:02x}", ch as u32),
        _ if (ch as u32) < 0x10000 => {
            let mut text = String::new();
            let _ = write!(text, "\\u{:04x}", ch as u32);
            text
        }
        _ => {
            let mut text = String::new();
            let _ = write!(text, "\\U{:08x}", ch as u32);
            text
        }
    }
}
