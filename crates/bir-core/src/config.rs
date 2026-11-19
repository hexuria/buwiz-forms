//! Process configuration layered over the launch environment.
//!
//! Embedders install overrides before the door starts its database or workers.
//! Updates are atomic as a batch and never mutate the process environment.

use std::collections::HashMap;
use std::env::VarError;
use std::sync::{LazyLock, RwLock};

static OVERRIDES: LazyLock<RwLock<HashMap<String, Option<String>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Reads an override, including an explicitly unset value, or the launch environment.
pub fn var(name: &str) -> Result<String, VarError> {
    let overrides = OVERRIDES.read().unwrap_or_else(|error| error.into_inner());
    match overrides.get(name) {
        Some(Some(value)) => Ok(value.clone()),
        Some(None) => Err(VarError::NotPresent),
        None => std::env::var(name),
    }
}

/// Installs configuration before database/worker startup. `None` hides an environment value.
pub fn set_many(values: impl IntoIterator<Item = (String, Option<String>)>) {
    OVERRIDES
        .write()
        .unwrap_or_else(|error| error.into_inner())
        .extend(values);
}

/// Forces one setting before database/worker startup; `None` treats it as unset.
pub fn force(name: &str, value: Option<&str>) {
    set_many([(name.to_owned(), value.map(str::to_owned))]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overrides_beat_the_environment_and_none_hides_it() {
        const KEY: &str = "BIR_CONFIG_OVERRIDE_TEST";
        temp_env::with_var(KEY, Some("environment"), || {
            assert_eq!(var(KEY).as_deref(), Ok("environment"));
            force(KEY, Some("override"));
            assert_eq!(var(KEY).as_deref(), Ok("override"));
            assert_eq!(std::env::var(KEY).as_deref(), Ok("environment"));
            force(KEY, None);
            assert_eq!(var(KEY), Err(VarError::NotPresent));
            assert_eq!(std::env::var(KEY).as_deref(), Ok("environment"));
        });
    }

    #[test]
    fn empty_values_are_distinct_from_unset_and_batches_keep_other_keys() {
        temp_env::with_var("BIR_CONFIG_MISSING_TEST", None::<&str>, || {
            assert_eq!(var("BIR_CONFIG_MISSING_TEST"), Err(VarError::NotPresent));
        });
        force("BIR_CONFIG_EMPTY_TEST", Some(""));
        set_many([]);
        set_many([("BIR_CONFIG_BATCH_TEST".into(), Some("value".into()))]);
        assert_eq!(var("BIR_CONFIG_EMPTY_TEST").as_deref(), Ok(""));
        assert_eq!(var("BIR_CONFIG_BATCH_TEST").as_deref(), Ok("value"));
    }
}
