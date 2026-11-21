//! Parse `key=value` lines.

/// Why a line was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The key before `=` is empty.
    EmptyKey,
    /// The line has no `=`.
    MissingSeparator,
}

/// One `key=value` line, both sides trimmed. A line without `=` is refused with
/// [`ParseError::MissingSeparator`], an empty key with [`ParseError::EmptyKey`].
pub fn parse_line(line: &str) -> Result<(String, String), ParseError> {
    let (key, value) = match line.split_once('=') {
        Some(kv) => kv,
        None => (line, ""),
    };
    let key = key.trim();
    if key.is_empty() {
        return Err(ParseError::EmptyKey);
    }
    Ok((key.to_string(), value.trim().to_string()))
}

/// Every non-empty line that is not a `#` comment, parsed; the first refusal ends it.
pub fn parse_all(text: &str) -> Result<Vec<(String, String)>, ParseError> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(parse_line)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_pair() {
        assert_eq!(parse_line("a=1"), Ok(("a".into(), "1".into())));
    }

    #[test]
    fn trims_both_sides() {
        assert_eq!(parse_line(" a = 1 "), Ok(("a".into(), "1".into())));
    }

    #[test]
    fn empty_value_is_allowed() {
        assert_eq!(parse_line("a="), Ok(("a".into(), "".into())));
    }

    #[test]
    fn empty_key_is_refused() {
        assert_eq!(parse_line("=1"), Err(ParseError::EmptyKey));
    }

    #[test]
    fn comments_are_skipped() {
        assert_eq!(parse_all("# c\na=1\n"), Ok(vec![("a".into(), "1".into())]));
    }
}
