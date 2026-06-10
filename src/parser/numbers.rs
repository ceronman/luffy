//! Parsing of numeric literal text into concrete `i64` / `f64` values.
//!
//! The lexer only guarantees that an `Int` / `Float` token *looks* like a
//! number (it records a span and nothing else). Turning that text into a value
//! — interpreting the base prefix, validating digits, stripping `_` separators,
//! and detecting overflow — happens here.

/// Parses the text of an `Int` literal token into an `i64`.
///
/// Supports decimal as well as hexadecimal (`0x`), octal (`0o`) and binary
/// (`0b`) literals. Any of them may use `_` as a separator between digits
/// (e.g. `1_000`, `0xDE_AD`). The base prefix is case-insensitive, as are
/// hexadecimal digits.
///
/// All validation (illegal digits for the base, misplaced separators, overflow)
/// happens here and is reported via the `Err` string, which the parser turns
/// into a parse error.
pub(super) fn parse_int_literal(text: &str) -> Result<i64, String> {
    let (radix, digits, base_name) = if let Some(rest) = strip_base_prefix(text, ['x', 'X']) {
        (16u32, rest, "hexadecimal")
    } else if let Some(rest) = strip_base_prefix(text, ['o', 'O']) {
        (8, rest, "octal")
    } else if let Some(rest) = strip_base_prefix(text, ['b', 'B']) {
        (2, rest, "binary")
    } else {
        (10, text, "decimal")
    };

    let cleaned = clean_digits(digits, |c| c.is_digit(radix))
        .map_err(|msg| format!("Invalid {base_name} integer literal `{text}`: {msg}"))?;

    i64::from_str_radix(&cleaned, radix)
        .map_err(|e| format!("Unable to parse integer value `{text}`: {e}"))
}

/// Parses the text of a `Float` literal token into an `f64`.
///
/// Floats are decimal only and follow Python's grammar:
///   - a fractional part with digits before and/or after the dot
///     (`3.14`, `10.`, `.5`); at least one side must have digits, and
///   - an optional exponent `(e|E)(+|-)?digits` (`1e10`, `1.5e-3`, `.5e9`).
///
/// `_` may be used as a separator between digits in the integer, fractional and
/// exponent parts (e.g. `1_000.000_5e1_0`). The exponent marker is
/// case-insensitive. Underscores are stripped before the value is parsed.
pub(super) fn parse_float_literal(text: &str) -> Result<f64, String> {
    let invalid = |msg: String| format!("Invalid float literal `{text}`: {msg}");

    // Split off the exponent. The mantissa never contains `e`/`E`, so the first
    // one is unambiguously the exponent marker.
    let (mantissa, exponent) = match text.find(['e', 'E']) {
        Some(i) => (&text[..i], Some(&text[i + 1..])),
        None => (text, None),
    };

    let mut cleaned = String::with_capacity(text.len());

    if let Some(dot) = mantissa.find('.') {
        let int_part = &mantissa[..dot];
        let frac_part = &mantissa[dot + 1..];
        if int_part.is_empty() && frac_part.is_empty() {
            return Err(invalid(
                "a float must have digits before or after the `.`".to_string(),
            ));
        }
        if !int_part.is_empty() {
            cleaned.push_str(&clean_digits(int_part, |c| c.is_ascii_digit()).map_err(invalid)?);
        }
        cleaned.push('.');
        if !frac_part.is_empty() {
            cleaned.push_str(&clean_digits(frac_part, |c| c.is_ascii_digit()).map_err(invalid)?);
        }
    } else {
        // No dot: the mantissa is all integer digits. The lexer only produces
        // such a `Float` token when an exponent follows (e.g. `1e10`).
        cleaned.push_str(&clean_digits(mantissa, |c| c.is_ascii_digit()).map_err(invalid)?);
    }

    if let Some(exp) = exponent {
        cleaned.push('e');
        let digits = if let Some(rest) = exp.strip_prefix('-') {
            cleaned.push('-');
            rest
        } else {
            exp.strip_prefix('+').unwrap_or(exp)
        };
        cleaned.push_str(&clean_digits(digits, |c| c.is_ascii_digit()).map_err(invalid)?);
    }

    cleaned
        .parse::<f64>()
        .map_err(|e| format!("Unable to parse float value `{text}`: {e}"))
}

/// If `text` begins with `0` followed by one of `letters`, returns the digits
/// following the two-character base prefix. Otherwise returns `None`.
fn strip_base_prefix(text: &str, letters: [char; 2]) -> Option<&str> {
    let bytes = text.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'0' && letters.iter().any(|&l| bytes[1] == l as u8) {
        Some(&text[2..])
    } else {
        None
    }
}

/// Validates digit-separator placement in `digits` and returns the digits with
/// the `_` separators removed. `is_digit` recognizes a legal digit for the
/// literal's base.
///
/// A `_` may only appear between two digits: it cannot be leading, trailing, or
/// adjacent to another `_`. The digit sequence must be non-empty.
fn clean_digits(digits: &str, is_digit: impl Fn(char) -> bool) -> Result<String, String> {
    if digits.is_empty() {
        return Err("missing digits".to_string());
    }

    let chars: Vec<char> = digits.chars().collect();
    let mut cleaned = String::with_capacity(chars.len());
    let mut prev_underscore = false;

    for (i, &c) in chars.iter().enumerate() {
        if c == '_' {
            if i == 0 || i == chars.len() - 1 {
                return Err("`_` separators must appear between digits".to_string());
            }
            if prev_underscore {
                return Err("consecutive `_` separators are not allowed".to_string());
            }
            prev_underscore = true;
        } else if is_digit(c) {
            cleaned.push(c);
            prev_underscore = false;
        } else {
            return Err(format!("invalid digit `{c}`"));
        }
    }

    Ok(cleaned)
}

#[cfg(test)]
mod test {
    use super::{clean_digits, parse_float_literal, parse_int_literal, strip_base_prefix};

    // ── parse_int_literal: decimal ────────────────────────────────────────────

    #[test]
    fn decimal_basic() {
        assert_eq!(parse_int_literal("0"), Ok(0));
        assert_eq!(parse_int_literal("42"), Ok(42));
        assert_eq!(parse_int_literal("1000000"), Ok(1_000_000));
    }

    #[test]
    fn decimal_with_underscores() {
        assert_eq!(parse_int_literal("1_000"), Ok(1000));
        assert_eq!(parse_int_literal("1_000_000"), Ok(1_000_000));
    }

    #[test]
    fn decimal_max_i64() {
        assert_eq!(parse_int_literal("9223372036854775807"), Ok(i64::MAX));
    }

    // ── parse_int_literal: hexadecimal ────────────────────────────────────────

    #[test]
    fn hexadecimal_basic() {
        assert_eq!(parse_int_literal("0xFF"), Ok(255));
        assert_eq!(parse_int_literal("0x10"), Ok(16));
    }

    #[test]
    fn hexadecimal_is_case_insensitive() {
        // Both the prefix letter and the hex digits accept either case.
        assert_eq!(parse_int_literal("0xff"), Ok(255));
        assert_eq!(parse_int_literal("0XFF"), Ok(255));
        assert_eq!(parse_int_literal("0xDeAd"), Ok(0xDEAD));
    }

    #[test]
    fn hexadecimal_with_underscores() {
        assert_eq!(parse_int_literal("0xDE_AD"), Ok(0xDEAD));
        assert_eq!(parse_int_literal("0x7FFF_FFFF_FFFF_FFFF"), Ok(i64::MAX));
    }

    // ── parse_int_literal: octal ──────────────────────────────────────────────

    #[test]
    fn octal_basic() {
        assert_eq!(parse_int_literal("0o17"), Ok(15));
        assert_eq!(parse_int_literal("0o755"), Ok(0o755));
        assert_eq!(parse_int_literal("0O17"), Ok(15));
    }

    #[test]
    fn octal_with_underscores() {
        assert_eq!(parse_int_literal("0o1_7"), Ok(15));
    }

    // ── parse_int_literal: binary ─────────────────────────────────────────────

    #[test]
    fn binary_basic() {
        assert_eq!(parse_int_literal("0b101"), Ok(5));
        assert_eq!(parse_int_literal("0B11"), Ok(3));
    }

    #[test]
    fn binary_with_underscores() {
        assert_eq!(parse_int_literal("0b1010_0101"), Ok(0b1010_0101));
    }

    // ── parse_int_literal: errors ─────────────────────────────────────────────

    #[test]
    fn invalid_digit_for_base() {
        assert!(parse_int_literal("0o18").is_err());
        assert!(parse_int_literal("0xGG").is_err());
        assert!(parse_int_literal("0b12").is_err());
    }

    #[test]
    fn missing_digits_after_prefix() {
        assert!(parse_int_literal("0x").is_err());
        assert!(parse_int_literal("0o").is_err());
        assert!(parse_int_literal("0b").is_err());
    }

    #[test]
    fn misplaced_underscores_are_rejected() {
        assert!(parse_int_literal("_1").is_err()); // leading
        assert!(parse_int_literal("1_").is_err()); // trailing
        assert!(parse_int_literal("1__0").is_err()); // consecutive
        assert!(parse_int_literal("0x_FF").is_err()); // adjacent to prefix
        assert!(parse_int_literal("0xFF_").is_err()); // trailing in digits
    }

    #[test]
    fn overflow_is_an_error() {
        // One past i64::MAX, in two different bases.
        assert!(parse_int_literal("9223372036854775808").is_err());
        assert!(parse_int_literal("0xFFFF_FFFF_FFFF_FFFF").is_err());
    }

    // ── parse_float_literal ───────────────────────────────────────────────────

    #[test]
    fn float_basic() {
        assert_eq!(parse_float_literal("2.14"), Ok(2.14));
        assert_eq!(parse_float_literal("0.1"), Ok(0.1));
    }

    #[test]
    fn float_trailing_dot() {
        assert_eq!(parse_float_literal("1."), Ok(1.0));
    }

    #[test]
    fn float_with_underscores() {
        assert_eq!(parse_float_literal("1_000.5"), Ok(1000.5));
        assert_eq!(parse_float_literal("1_000.000_1"), Ok(1000.0001));
    }

    #[test]
    fn float_misplaced_underscores_are_rejected() {
        assert!(parse_float_literal("1_.5").is_err());
        assert!(parse_float_literal("1._5").is_err());
        assert!(parse_float_literal("_1.5").is_err());
    }

    #[test]
    fn float_leading_dot() {
        assert_eq!(parse_float_literal(".5"), Ok(0.5));
        assert_eq!(parse_float_literal(".25"), Ok(0.25));
        assert_eq!(parse_float_literal(".000_5"), Ok(0.0005));
    }

    #[test]
    fn float_scientific_notation() {
        assert_eq!(parse_float_literal("1e10"), Ok(1e10));
        assert_eq!(parse_float_literal("1E10"), Ok(1e10));
        assert_eq!(parse_float_literal("1.5e-3"), Ok(1.5e-3));
        assert_eq!(parse_float_literal("2E+8"), Ok(2e8));
        assert_eq!(parse_float_literal("6.022e23"), Ok(6.022e23));
    }

    #[test]
    fn float_scientific_with_leading_and_trailing_dot() {
        assert_eq!(parse_float_literal(".5e3"), Ok(500.0));
        assert_eq!(parse_float_literal("10.e3"), Ok(10000.0));
    }

    #[test]
    fn float_scientific_with_underscores() {
        assert_eq!(parse_float_literal("1_000.000_5e1_0"), Ok(1_000.000_5e10));
    }

    #[test]
    fn float_bare_dot_is_rejected() {
        // No digits on either side of the dot.
        assert!(parse_float_literal(".").is_err());
    }

    #[test]
    fn float_empty_exponent_is_rejected() {
        assert!(parse_float_literal("1.5e").is_err());
        assert!(parse_float_literal("1e+").is_err());
    }

    #[test]
    fn float_misplaced_underscores_in_exponent_are_rejected() {
        assert!(parse_float_literal("1e1_").is_err());
        assert!(parse_float_literal("1e_1").is_err());
    }

    // ── strip_base_prefix ─────────────────────────────────────────────────────

    #[test]
    fn strip_base_prefix_matches_only_with_leading_zero() {
        assert_eq!(strip_base_prefix("0xFF", ['x', 'X']), Some("FF"));
        assert_eq!(strip_base_prefix("0XFF", ['x', 'X']), Some("FF"));
        assert_eq!(strip_base_prefix("0x", ['x', 'X']), Some(""));
        // Wrong leading char, wrong letter, or too short: no match.
        assert_eq!(strip_base_prefix("1xFF", ['x', 'X']), None);
        assert_eq!(strip_base_prefix("0oFF", ['x', 'X']), None);
        assert_eq!(strip_base_prefix("0", ['x', 'X']), None);
    }

    // ── clean_digits ──────────────────────────────────────────────────────────

    #[test]
    fn clean_digits_removes_separators() {
        assert_eq!(
            clean_digits("1_000", |c| c.is_ascii_digit()),
            Ok("1000".to_string())
        );
    }

    #[test]
    fn clean_digits_rejects_empty_and_misplaced() {
        assert!(clean_digits("", |c| c.is_ascii_digit()).is_err());
        assert!(clean_digits("_1", |c| c.is_ascii_digit()).is_err());
        assert!(clean_digits("1_", |c| c.is_ascii_digit()).is_err());
        assert!(clean_digits("1__2", |c| c.is_ascii_digit()).is_err());
    }
}
