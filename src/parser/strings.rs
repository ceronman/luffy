//! Decoding of string literal text into a concrete `String` value.
//!
//! The lexer only guarantees that a `Str` token starts and ends with an
//! unescaped `"` (an escaped quote `\"` never closes the token early, and an
//! unterminated string is rejected by the lexer as an `Error` token before it
//! reaches here). Turning the text between the quotes into an actual
//! `String` — validating escape sequences and decoding them — happens here,
//! mirroring how `numbers.rs` interprets `Int`/`Float` token text.
//!
//! Luffy uses a Swift-style escape set: `\0`, `\\`, `\t`, `\n`, `\r`, `\"`,
//! `\'`, and `\u{...}` (1 to 8 hex digits) for an arbitrary Unicode scalar
//! value. There are no octal or `\xHH` byte escapes, and raw strings are not
//! supported yet.

/// Parses the text of a `Str` literal token (including its surrounding
/// quotes) into the `String` value it represents.
pub(super) fn parse_string_literal(text: &str) -> Result<String, String> {
    let inner = text
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .ok_or_else(|| format!("Malformed string literal `{text}`"))?;

    let mut result = String::with_capacity(inner.len());
    let mut chars = inner.chars();

    while let Some(c) = chars.next() {
        if c != '\\' {
            result.push(c);
            continue;
        }

        let Some(escape) = chars.next() else {
            return Err("dangling `\\` at the end of a string literal".to_string());
        };

        match escape {
            '0' => result.push('\0'),
            '\\' => result.push('\\'),
            't' => result.push('\t'),
            'n' => result.push('\n'),
            'r' => result.push('\r'),
            '"' => result.push('"'),
            '\'' => result.push('\''),
            'u' => result.push(parse_unicode_escape(&mut chars)?),
            other => return Err(format!("unknown escape sequence `\\{other}`")),
        }
    }

    Ok(result)
}

/// Parses the body of a `\u{...}` escape, assuming the `\u` has already been
/// consumed. Accepts 1 to 8 hex digits between braces (Swift's grammar) and
/// requires the resulting code point to be a valid Unicode scalar value.
fn parse_unicode_escape(chars: &mut std::str::Chars<'_>) -> Result<char, String> {
    if chars.next() != Some('{') {
        return Err("expected `{` after `\\u`".to_string());
    }

    let mut digits = String::new();
    let mut closed = false;
    for c in chars.by_ref() {
        if c == '}' {
            closed = true;
            break;
        }
        digits.push(c);
    }

    if !closed {
        return Err("unterminated `\\u{...}` escape: missing `}`".to_string());
    }
    if digits.is_empty() {
        return Err("`\\u{}` escape must contain at least one hex digit".to_string());
    }
    if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "`\\u{{{digits}}}` may only contain hex digits (0-9, a-f, A-F)"
        ));
    }
    if digits.len() > 8 {
        return Err(format!(
            "`\\u{{{digits}}}` has too many hex digits (at most 8 are allowed)"
        ));
    }

    let value = u32::from_str_radix(&digits, 16)
        .map_err(|e| format!("invalid `\\u{{{digits}}}` escape: {e}"))?;

    char::from_u32(value)
        .ok_or_else(|| format!("`\\u{{{digits}}}` is not a valid Unicode scalar value"))
}

#[cfg(test)]
mod test {
    use super::parse_string_literal;

    // ── plain text ────────────────────────────────────────────────────────

    #[test]
    fn empty_string() {
        assert_eq!(parse_string_literal("\"\""), Ok("".to_string()));
    }

    #[test]
    fn plain_text() {
        assert_eq!(
            parse_string_literal("\"hello world\""),
            Ok("hello world".to_string())
        );
    }

    #[test]
    fn unicode_text_without_escapes() {
        assert_eq!(
            parse_string_literal("\"héllo 🐍\""),
            Ok("héllo 🐍".to_string())
        );
    }

    // ── single-character escapes ─────────────────────────────────────────────

    #[test]
    fn simple_escapes() {
        assert_eq!(
            parse_string_literal("\"\\0\\\\\\t\\n\\r\\\"\\'\""),
            Ok("\0\\\t\n\r\"'".to_string())
        );
    }

    #[test]
    fn escaped_quote_does_not_end_the_string() {
        assert_eq!(parse_string_literal("\"a\\\"b\""), Ok("a\"b".to_string()));
    }

    // ── \u{...} escapes ───────────────────────────────────────────────────

    #[test]
    fn unicode_escape_basic() {
        assert_eq!(parse_string_literal("\"\\u{41}\""), Ok("A".to_string()));
        assert_eq!(parse_string_literal("\"\\u{0}\""), Ok("\0".to_string()));
    }

    #[test]
    fn unicode_escape_min_and_max_digits() {
        assert_eq!(parse_string_literal("\"\\u{9}\""), Ok("\t".to_string()));
        assert_eq!(
            parse_string_literal("\"\\u{0010FFFF}\""),
            Ok("\u{10FFFF}".to_string())
        );
    }

    #[test]
    fn unicode_escape_emoji() {
        assert_eq!(parse_string_literal("\"\\u{1F600}\""), Ok("😀".to_string()));
    }

    #[test]
    fn unicode_escape_mixed_with_text() {
        assert_eq!(parse_string_literal("\"a\\u{42}c\""), Ok("aBc".to_string()));
    }

    // ── errors ─────────────────────────────────────────────────────────────

    #[test]
    fn unknown_escape_is_an_error() {
        assert!(parse_string_literal("\"\\q\"").is_err());
        assert!(parse_string_literal("\"\\x41\"").is_err()); // no \xHH in this language
        assert!(parse_string_literal("\"\\1\"").is_err()); // no octal escapes
    }

    #[test]
    fn dangling_backslash_is_an_error() {
        // The 6-character string: `"abc\"` (quote, a, b, c, backslash, quote).
        // After the outer quotes are stripped, the inner text ends in a
        // trailing, unescaped backslash.
        assert!(parse_string_literal("\"abc\\\"").is_err());
    }

    #[test]
    fn malformed_token_without_quotes_is_an_error() {
        assert!(parse_string_literal("hello").is_err());
    }

    #[test]
    fn unicode_escape_missing_braces_is_an_error() {
        assert!(parse_string_literal("\"\\u41\"").is_err());
        assert!(parse_string_literal("\"\\u{41\"").is_err());
    }

    #[test]
    fn unicode_escape_empty_braces_is_an_error() {
        assert!(parse_string_literal("\"\\u{}\"").is_err());
    }

    #[test]
    fn unicode_escape_non_hex_digit_is_an_error() {
        assert!(parse_string_literal("\"\\u{zz}\"").is_err());
    }

    #[test]
    fn unicode_escape_too_many_digits_is_an_error() {
        assert!(parse_string_literal("\"\\u{123456789}\"").is_err());
    }

    #[test]
    fn unicode_escape_surrogate_is_an_error() {
        // D800..DFFF are surrogate code points: not valid scalar values on
        // their own.
        assert!(parse_string_literal("\"\\u{D800}\"").is_err());
    }

    #[test]
    fn unicode_escape_out_of_range_is_an_error() {
        assert!(parse_string_literal("\"\\u{110000}\"").is_err());
    }
}
