//! Android preview dismissal policy; submitted jobs remain owned by the spooler.

pub(crate) fn should_dismiss(revoked: bool, job_state: Option<i32>) -> bool {
    const STATE_CREATED: i32 = 1;
    revoked && job_state == Some(STATE_CREATED)
}

#[cfg(test)]
mod tests {
    use super::should_dismiss;

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
