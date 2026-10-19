//! Words for `bash -c` commands (the launcher's actions).

/// Quotes a word for the shell when it holds anything but safe characters.
pub fn word(s: &str) -> String {
    if !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "_./:-+@".contains(c))
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::word;

    #[test]
    fn quotes_only_when_needed() {
        assert_eq!(word("/home/u/a-b_c.txt"), "/home/u/a-b_c.txt");
        assert_eq!(word(""), "''");
        assert_eq!(word("a b"), "'a b'");
        assert_eq!(word("it's"), "'it'\\''s'");
        assert_eq!(word("$(x)"), "'$(x)'");
    }
}
