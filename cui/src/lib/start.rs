//! The dev start route (port of shell/start.ts): "tp=123456788;theme=dark;view=library;form=2551Q/2026/3".
//! Only where the door reports a test environment — the route's access steps take secrets.

/// The start route a launch may use: only where the door reports a test environment.
pub fn allowed_start(raw: Option<&str>, test_environment: bool) -> Option<String> {
    if test_environment {
        raw.map(str::to_owned)
    } else {
        None
    }
}

/// One "key=value" step of a start route.
#[derive(Debug, Clone, PartialEq)]
pub struct StartStep {
    pub key: String,
    pub value: String,
}

/// The steps of a route, in order ("a=1;b=2"); a step without "=" has an empty value; blanks skipped.
pub fn parse_start(raw: Option<&str>) -> Vec<StartStep> {
    let mut steps = Vec::new();
    for part in raw.unwrap_or("").split(';') {
        let (key, value) = match part.find('=') {
            Some(at) => (&part[..at], &part[at + 1..]),
            None => (part, ""),
        };
        let key = key.trim();
        if !key.is_empty() {
            steps.push(StartStep {
                key: key.to_owned(),
                value: value.trim().to_owned(),
            });
        }
    }
    steps
}

/// A return named by a start route: form=CODE/YEAR/PERIOD[/KEY].
#[derive(Debug, Clone, PartialEq)]
pub struct StartForm {
    pub code: String,
    pub year: i64,
    pub period: i64,
    pub key: Option<i64>,
}

/// Parses "2551Q/2026/3" or "2551Q/2026/3/1201"; `None` when the code, the year or the period is
/// missing or not a number.
pub fn parse_start_form(value: &str) -> Option<StartForm> {
    let parts: Vec<&str> = value.split('/').collect();
    if parts.len() < 3 || parts.len() > 4 {
        return None;
    }
    let (code, year, period) = (parts[0], parts[1], parts[2]);
    if code.is_empty() || year.is_empty() || period.is_empty() {
        return None;
    }
    let y: i64 = year.parse().ok()?;
    let p: i64 = period.parse().ok()?;
    let key = match parts.get(3) {
        Some(k) if !k.is_empty() => k.parse().ok(),
        _ => None,
    };
    Some(StartForm {
        code: code.to_owned(),
        year: y,
        period: p,
        key,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps() {
        let steps = parse_start(Some("tp=123; theme=dark ;view=library"));
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0].key, "tp");
        assert_eq!(steps[1].value, "dark");
    }

    #[test]
    fn forms() {
        assert_eq!(
            parse_start_form("2551Q/2026/3/1201"),
            Some(StartForm {
                code: "2551Q".into(),
                year: 2026,
                period: 3,
                key: Some(1201)
            })
        );
        assert_eq!(parse_start_form("2551Q/2026"), None);
        assert_eq!(parse_start_form("2551Q/2026/x"), None);
        assert!(allowed_start(Some("view=home"), false).is_none());
        assert_eq!(
            allowed_start(Some("view=home"), true).as_deref(),
            Some("view=home")
        );
    }
}
