//! What the 30 s poll tells the person when filing events appeared since the last look (shell/events.ts). Pure.

/// The part of a notification the toast needs.
#[derive(Debug, Clone)]
pub struct FreshEvent {
    pub title: String,
    pub icon: String,
}

/// One new event is announced by its own title; several by a count that points at the notifications;
/// none say nothing.
pub fn event_toast(fresh: &[FreshEvent]) -> Option<(String, String)> {
    let first = fresh.first()?;
    let message = if fresh.len() == 1 {
        first.title.clone()
    } else {
        format!(
            "{} filing updates. Open notifications to see them.",
            fresh.len()
        )
    };
    Some((message, first.icon.clone()))
}

/// The events of `all` that `known` has not seen; `known` learns them.
pub fn take_fresh<T: HasId>(known: &mut std::collections::HashSet<String>, all: &[T]) -> Vec<T>
where
    T: Clone,
{
    let fresh: Vec<T> = all
        .iter()
        .filter(|n| !known.contains(n.id()))
        .cloned()
        .collect();
    for n in &fresh {
        known.insert(n.id().to_owned());
    }
    fresh
}

/// Anything with an `id` the freshness check can key on.
pub trait HasId {
    fn id(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toasts() {
        assert!(event_toast(&[]).is_none());
        let one = [FreshEvent {
            title: "BIR confirmed 1601C".into(),
            icon: "circle-check".into(),
        }];
        assert_eq!(
            event_toast(&one),
            Some(("BIR confirmed 1601C".into(), "circle-check".into()))
        );
        let two = [
            FreshEvent {
                title: "a".into(),
                icon: "send".into(),
            },
            FreshEvent {
                title: "b".into(),
                icon: "circle-check".into(),
            },
        ];
        assert_eq!(
            event_toast(&two),
            Some((
                "2 filing updates. Open notifications to see them.".into(),
                "send".into()
            ))
        );
    }
}
