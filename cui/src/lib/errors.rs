//! `errorText` from store.js: the words a person reads for a host error. Pure.

/// The text of a host error: a message that starts with a function name in backticks and a colon arrives; the
/// person needs only the text. `e` is the error's message or Display text already.
pub fn error_text(message: &str) -> String {
    let mut text = message;
    if let Some(rest) = text.strip_prefix("Error: ") {
        text = rest;
    }
    // `func_name`: the rest.
    if text.starts_with('`')
        && let Some(end) = text[1..].find('`').map(|i| i + 1)
        && text[end + 1..].starts_with(": ")
    {
        return text[end + 3..].to_owned();
    }
    text.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_the_error_prefix_and_the_function_name() {
        assert_eq!(
            error_text("Error: `system_info`: no such function"),
            "no such function"
        );
        assert_eq!(
            error_text("`queue_form_draft`: the draft is stale"),
            "the draft is stale"
        );
        assert_eq!(error_text("plain words"), "plain words");
        assert_eq!(error_text("Error: plain"), "plain");
    }
}
