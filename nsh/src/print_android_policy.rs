//! Android print policy: which preview is dismissed (submitted jobs remain owned by the spooler), and the text
//! settings of the WebView that prints.

pub(crate) fn should_dismiss(revoked: bool, job_state: Option<i32>) -> bool {
    const STATE_CREATED: i32 = 1;
    revoked && job_state == Some(STATE_CREATED)
}

/// `android.webkit.WebSettings` setters (each takes one `int`) and the value the printing WebView gets, so that
/// text prints at the size the sheet gives it. Android's defaults are 8, 8 and 100: the two minimums are lowered
/// to 1, the smallest value Android accepts, and the zoom is said outright so it cannot be anything else.
pub(crate) const TEXT_SETTINGS: [(&str, i32); 3] = [("setMinimumFontSize", 1), ("setMinimumLogicalFontSize", 1), ("setTextZoom", 100)];

#[cfg(test)]
mod tests {
    use super::{TEXT_SETTINGS, should_dismiss};

    #[test]
    fn the_printing_webview_has_no_text_floor_and_no_text_zoom() {
        let value = |setter: &str| TEXT_SETTINGS.iter().find(|(name, _)| *name == setter).map(|(_, v)| *v);
        // 1 is the smallest minimum WebSettings accepts (1 to 72): the sheets print captions at 5.2 pt and fitted
        // values smaller still, on a page that is scaled down as well.
        assert_eq!(value("setMinimumFontSize"), Some(1));
        assert_eq!(value("setMinimumLogicalFontSize"), Some(1));
        assert_eq!(value("setTextZoom"), Some(100));
        assert_eq!(TEXT_SETTINGS.len(), 3);
    }

    const JOB_STATES: [(&str, i32); 7] = [
        ("created", 1),
        ("queued", 2),
        ("started", 3),
        ("blocked", 4),
        ("completed", 5),
        ("failed", 6),
        ("cancelled", 7),
    ];

    #[test]
    fn revocation_dismisses_only_the_unsubmitted_preview() {
        for (name, state) in JOB_STATES {
            assert_eq!(should_dismiss(true, Some(state)), state == 1, "{name}");
        }
    }

    #[test]
    fn replacement_leaves_every_job_alone() {
        for (name, state) in JOB_STATES {
            assert!(!should_dismiss(false, Some(state)), "{name}");
        }
    }

    #[test]
    fn missing_or_unknown_jobs_are_left_alone() {
        for revoked in [false, true] {
            for state in [None, Some(-1), Some(0), Some(8), Some(i32::MAX)] {
                assert!(!should_dismiss(revoked, state), "revoked={revoked}, state={state:?}");
            }
        }
    }
}
