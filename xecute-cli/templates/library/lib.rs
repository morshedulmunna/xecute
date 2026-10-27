//! Reusable application code. Keep HTTP adapters in the consuming application.
//!
//! ```
//! assert_eq!(__CRATE__::greet("  Munna  "), "Hello, Munna!");
//! ```

/// Return a greeting with surrounding whitespace removed.
/// Empty or whitespace-only names use a neutral fallback.
#[must_use]
pub fn greet(name: &str) -> String {
    let name = name.trim();
    let name = if name.is_empty() { "world" } else { name };
    format!("Hello, {name}!")
}

#[cfg(test)]
mod tests {
    use super::greet;

    #[test]
    fn trims_input_and_preserves_unicode() {
        assert_eq!(greet("  Munna  "), "Hello, Munna!");
        assert_eq!(greet(" বাংলা "), "Hello, বাংলা!");
    }

    #[test]
    fn blank_names_use_the_fallback() {
        for name in ["", " ", "\t\n"] {
            assert_eq!(greet(name), "Hello, world!");
        }
    }
}
