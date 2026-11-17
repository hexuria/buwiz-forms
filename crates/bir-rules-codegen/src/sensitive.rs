use std::sync::LazyLock;

use regex::Regex;

use crate::error::{CodegenError, Result};

static EMAIL_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)[a-z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?\.[a-z]{2,}")
        .expect("sensitive email pattern is valid")
});
static ASSIGNED_CREDENTIAL_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:password|passwd|secret|token|api[_-]?key|client[_-]?secret|authorization|credential)\s*(?:[:=]|\b(?:is|was)\b)\s*\S+",
    )
    .expect("assigned credential pattern is valid")
});
static AWS_ACCESS_KEY_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|[^A-Z0-9])AKIA[A-Z0-9]{16}(?:$|[^A-Z0-9])")
        .expect("AWS access-key pattern is valid")
});

pub(crate) fn reject_sensitive_text(value: &str, label: &str) -> Result<()> {
    if looks_like_email(value)
        || looks_like_tin(value)
        || looks_like_credential(value)
        || looks_like_online_submission(value)
    {
        return Err(CodegenError::new(format!(
            "{label} contains a credential, taxpayer value, email address, or online-submission value"
        )));
    }
    Ok(())
}

pub(crate) fn looks_like_email(value: &str) -> bool {
    EMAIL_PATTERN.is_match(value)
}

pub(crate) fn looks_like_tin(value: &str) -> bool {
    let mut digit_count = 0_usize;
    let mut has_left_boundary = true;
    let mut previous = None;
    for character in value.chars().chain(std::iter::once('\0')) {
        if character.is_ascii_digit() {
            if digit_count == 0 {
                has_left_boundary = previous
                    .map(|character: char| !character.is_ascii_alphanumeric())
                    .unwrap_or(true);
            }
            digit_count += 1;
        } else if is_numeric_separator(character) {
            // Keep a formatted numeric token open across its separators.
        } else {
            if matches!(digit_count, 9 | 12)
                && has_left_boundary
                && !character.is_ascii_alphanumeric()
            {
                return true;
            }
            digit_count = 0;
        }
        previous = Some(character);
    }
    false
}

pub(crate) fn looks_like_credential(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "bearer ",
        "basic ",
        "sk-",
        "ghp_",
        "github_pat_",
        "glpat-",
        "xoxb-",
        "xoxp-",
        "xoxa-",
        "npm_",
    ]
    .iter()
    .any(|prefix| contains_credential_prefix(&lower, prefix))
        || ASSIGNED_CREDENTIAL_PATTERN.is_match(value)
        || AWS_ACCESS_KEY_PATTERN.is_match(value)
        || lower.contains("begin private key")
}

pub(crate) fn looks_like_online_submission(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    ["http://", "https://"].iter().any(|scheme| {
        lower.match_indices(scheme).any(|(start, _)| {
            let candidate = &lower[start..];
            let end = candidate
                .find(|character: char| {
                    character.is_whitespace() || matches!(character, '"' | '\'' | '<' | '>')
                })
                .unwrap_or(candidate.len());
            let url = &candidate[..end];
            ["submit", "file", "transmit", "queue"]
                .iter()
                .any(|marker| url.contains(marker))
        })
    })
}

fn contains_credential_prefix(value: &str, prefix: &str) -> bool {
    value.match_indices(prefix).any(|(index, _)| {
        let boundary = index == 0
            || (!value.as_bytes()[index - 1].is_ascii_alphanumeric()
                && value.as_bytes()[index - 1] != b'_');
        let continuation = value[index + prefix.len()..]
            .chars()
            .take_while(|character| {
                !character.is_whitespace() && !matches!(character, '"' | '\'' | ',' | ';')
            })
            .count();
        boundary && continuation >= 4
    })
}

fn is_numeric_separator(character: char) -> bool {
    character.is_whitespace()
        || matches!(
            character,
            '-' | '\u{2010}'
                | '\u{2011}'
                | '\u{2012}'
                | '\u{2013}'
                | '\u{2014}'
                | '\u{2212}'
                | '\u{FE58}'
                | '\u{FF0D}'
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitive_text_predicate_rejects_each_forbidden_channel() {
        for value in [
            "person@example.test",
            "reviewer person@example.test approved the packet",
            "123-456-789",
            "TIN: 123-456-789",
            "TIN: 123–456–789",
            "password=hunter2",
            "notes password: hunter2",
            "the password is hunter2",
            "sk-example-secret",
            "notes ghp_<secret>",
            "https://example.test/submit",
            "captured endpoint (https://example.test/queue/file)",
        ] {
            assert!(reject_sensitive_text(value, "fixture").is_err(), "{value}");
        }
    }

    #[test]
    fn ordinary_version_and_portable_evidence_text_are_allowed() {
        for value in [
            "bir-rules-codegen 0.1.0",
            "Windows 11 23H2",
            "7.9.6.0",
            "risk-analysis does not contain an sk- credential token",
            "https://example.test/reference",
            "../validation-rules-evidence-input/maps/source-map.json",
            "a123456789b",
            "sha256-a123456789bcdef",
        ] {
            reject_sensitive_text(value, "fixture").unwrap();
        }
    }
}
