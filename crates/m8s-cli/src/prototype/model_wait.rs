//! Bounded discovery only. A listed model is not identity or readiness evidence.
use std::time::{Duration, Instant};

#[derive(serde::Deserialize)]
struct Models {
    object: String,
    data: Vec<Model>,
    error: Option<serde_json::Value>,
}
#[derive(serde::Deserialize)]
struct Model {
    id: String,
}

fn listed(bytes: &[u8], expected: &str) -> bool {
    if bytes.len() > 65536 {
        return false;
    }
    let Ok(models) = serde_json::from_slice::<Models>(bytes) else {
        return false;
    };
    models.object == "list"
        && models.error.is_none()
        && models.data.len() <= 256
        && models.data.iter().filter(|m| m.id == expected).count() == 1
}

#[allow(dead_code)] // Used by the standalone prototype when prototype-vast is disabled.
pub(super) fn wait(port: u16, model: &str, deadline: Instant) -> Result<(), &'static str> {
    wait_exact(port, &[model], deadline).map(|_| ())
}

// SDK versions may expose the pinned HF alias or its exact native source identity.
// Return the identity actually discovered so the one inference request uses it.
pub(super) fn wait_exact<'a>(
    port: u16,
    models: &[&'a str],
    deadline: Instant,
) -> Result<&'a str, &'static str> {
    loop {
        let now = Instant::now();
        if now >= deadline {
            return Err("prototype-model-unavailable");
        }
        let attempt_deadline = (now + Duration::from_secs(1)).min(deadline);
        if let Ok(bytes) = super::http::models(port, attempt_deadline)
            && Instant::now() < deadline
            && let Some(model) = select(&bytes, models)
        {
            return Ok(model);
        }
        // Only GET is retried. The caller sends at most one inference POST.
        if let Some(left) = deadline.checked_duration_since(Instant::now()) {
            std::thread::sleep(left.min(Duration::from_millis(100)));
        }
    }
}

fn select<'a>(bytes: &[u8], models: &[&'a str]) -> Option<&'a str> {
    models.iter().copied().find(|model| listed(bytes, model))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_unique_model_required() {
        assert!(listed(
            br#"{"object":"list","data":[{"id":"selected"}]}"#,
            "selected"
        ));
        for bytes in [
            br#"{"object":"list","data":[]}"#.as_slice(),
            br#"{"object":"list","data":[{"id":"other"}]}"#,
            br#"{"object":"list","data":[{"id":"selected"},{"id":"selected"}]}"#,
            br#"{"object":"list","data":[{"id":"selected"}],"error":{}}"#,
            br#"{"object":"model","data":[{"id":"selected"}]}"#,
            br#"{"object":"list","data":[{"id":"selected"}],"data":[]}"#,
        ] {
            assert!(!listed(bytes, "selected"));
        }
    }
    #[test]
    fn discovery_selects_only_a_pinned_identity() {
        let candidates = ["pinned-hf-alias", "exact-native-source"];
        assert_eq!(
            select(
                br#"{"object":"list","data":[{"id":"exact-native-source"}]}"#,
                &candidates
            ),
            Some("exact-native-source")
        );
        assert_eq!(
            select(
                br#"{"object":"list","data":[{"id":"pinned-hf-alias"}]}"#,
                &candidates
            ),
            Some("pinned-hf-alias")
        );
        assert_eq!(
            select(
                br#"{"object":"list","data":[{"id":"other-source"}]}"#,
                &candidates
            ),
            None
        );
    }
    #[test]
    fn expired_wait_never_connects() {
        assert_eq!(
            wait(0, "selected", Instant::now()),
            Err("prototype-model-unavailable")
        );
    }
}
