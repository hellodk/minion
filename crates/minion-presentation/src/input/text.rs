pub fn process_text(content: &str) -> String {
    content.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn returns_unchanged() { assert_eq!(process_text("abc"), "abc"); }
    #[test]
    fn handles_empty() { assert_eq!(process_text(""), ""); }
}
