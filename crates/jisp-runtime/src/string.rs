pub fn cat(parts: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    let parts = parts.into_iter();
    let (lower, _) = parts.size_hint();
    let mut output = String::with_capacity(lower.saturating_mul(8));
    for part in parts {
        output.push_str(part.as_ref());
    }
    output
}

pub fn lines(parts: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    let mut output = String::new();
    for (index, part) in parts.into_iter().enumerate() {
        if index > 0 {
            output.push('\n');
        }
        output.push_str(part.as_ref());
    }
    output
}

pub fn slice(value: &str, start: usize, end: usize) -> Option<String> {
    let chars: Vec<char> = value.chars().collect();
    if start > end || end > chars.len() {
        return None;
    }
    Some(chars[start..end].iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_lines_without_a_trailing_newline() {
        assert_eq!(lines(["a", "b"]), "a\nb");
    }
}
