//! Opt-in one-line request log for `bir-headless` (and the painted mailbox drain).
//!
//! Switch: `GPUI_AGENT_LOG_REQUESTS=1` (`true`/`yes`/`on`). Off by default.
//! Not tied to `RUST_LOG` so normal `serve` stays quiet. Never logs the
//! automation token, invoke args, `set_value` values, typed text, or paths.

use gpui_agent::protocol::{Op, Request, Response};

pub const LOG_REQUESTS_ENV: &str = "GPUI_AGENT_LOG_REQUESTS";

pub fn log_requests_enabled() -> bool {
    gpui_agent::security::truthy_env(LOG_REQUESTS_ENV)
}

pub fn emit_request_log(req: &Request, resp: &Response) {
    if let Some(line) = request_log_line(req, resp) {
        eprintln!("{line}");
    }
}

pub fn handle_request_logged(
    host: &mut dyn gpui_agent::AgentHost,
    req: Request,
    expected_token: Option<&str>,
    session_nonce: Option<&[u8]>,
) -> Response {
    if !log_requests_enabled() {
        return gpui_agent::handle_request(host, req, expected_token, session_nonce);
    }
    let log_req = req.clone();
    let resp = gpui_agent::handle_request(host, req, expected_token, session_nonce);
    emit_request_log(&log_req, &resp);
    resp
}

pub fn request_log_line(req: &Request, resp: &Response) -> Option<String> {
    if !log_requests_enabled() {
        return None;
    }
    Some(format_request_log_line(
        &utc_timestamp(),
        &req.id,
        &req.op,
        resp.ok,
        resp.error.as_deref(),
    ))
}

fn utc_timestamp() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

pub fn format_request_log_line(
    timestamp: &str,
    id: &str,
    op: &Op,
    ok: bool,
    error: Option<&str>,
) -> String {
    let kind = op_kind(op);
    let extra = op_extra(op);
    let status = if ok { "ok=true" } else { "ok=false" };
    let mut line = format!("{timestamp} gpui-agent id={id} op={kind}");
    if let Some(extra) = extra {
        line.push(' ');
        line.push_str(&extra);
    }
    line.push(' ');
    line.push_str(status);
    if !ok && let Some(error) = error {
        let short = short_error(error);
        if !short.is_empty() {
            line.push_str(" error=");
            line.push_str(&short);
        }
    }
    line
}

fn op_kind(op: &Op) -> &'static str {
    match op {
        Op::Hello => "hello",
        Op::Snapshot => "snapshot",
        Op::Click { .. } => "click",
        Op::Type { .. } => "type",
        Op::SetValue { .. } => "set_value",
        Op::Key { .. } => "key",
        Op::Invoke { .. } => "invoke",
        Op::Wait { .. } => "wait",
        Op::Shutdown => "shutdown",
        Op::Screenshot { .. } => "screenshot",
        Op::Assert { .. } => "assert",
    }
}

fn op_extra(op: &Op) -> Option<String> {
    match op {
        Op::Invoke { name, .. } => Some(format!("name={name}")),
        Op::Click { target, .. }
        | Op::Type { target, .. }
        | Op::SetValue { target, .. }
        | Op::Key { target, .. } => Some(format!("target={target}")),
        Op::Assert { spec } => Some(format!("target={}", spec.target)),
        _ => None,
    }
}

fn short_error(error: &str) -> String {
    let one_line: String = error
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let trimmed = one_line.trim();
    let mut out: String = trimmed.chars().take(80).collect();
    if trimmed.chars().count() > 80 {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_agent::protocol::{AssertSpec, Op, Request, Response};
    use serde_json::json;

    fn invoke_req() -> Request {
        Request::new(
            "t1",
            Op::Invoke {
                name: "profile.list".into(),
                args: json!({ "tin": "00000000000002", "name": "Secret Person" }),
            },
        )
        .with_token("super-secret-token")
    }

    #[test]
    fn disabled_by_default() {
        temp_env::with_var("GPUI_AGENT_LOG_REQUESTS", None::<&str>, || {
            let req = invoke_req();
            let resp = Response::ok("t1");
            assert!(request_log_line(&req, &resp).is_none());
        });
    }

    #[test]
    fn enabled_line_has_id_op_name_ok_not_token_or_args() {
        temp_env::with_var("GPUI_AGENT_LOG_REQUESTS", Some("1"), || {
            let req = invoke_req();
            let resp = Response::ok("t1");
            let line = request_log_line(&req, &resp).expect("enabled");
            assert!(line.contains("id=t1"), "{line}");
            assert!(line.contains("op=invoke"), "{line}");
            assert!(line.contains("name=profile.list"), "{line}");
            assert!(line.contains("ok=true"), "{line}");
            assert!(!line.contains("super-secret-token"), "{line}");
            assert!(!line.contains("00000000000002"), "{line}");
            assert!(!line.contains("Secret Person"), "{line}");
        });
    }

    #[test]
    fn set_value_logs_target_not_value() {
        let line = format_request_log_line(
            "ts",
            "id2",
            &Op::SetValue {
                target: "profile-tin".into(),
                value: "00000000000002".into(),
            },
            true,
            None,
        );
        assert!(line.contains("op=set_value"), "{line}");
        assert!(line.contains("target=profile-tin"), "{line}");
        assert!(!line.contains("00000000000002"), "{line}");
        assert!(line.contains("ok=true"), "{line}");
    }

    #[test]
    fn type_and_screenshot_omit_payloads() {
        let typed = format_request_log_line(
            "ts",
            "id3",
            &Op::Type {
                target: "profile-name".into(),
                text: "Juan Dela Cruz".into(),
                delivery: Default::default(),
            },
            true,
            None,
        );
        assert!(typed.contains("op=type target=profile-name"), "{typed}");
        assert!(!typed.contains("Juan"), "{typed}");

        let shot = format_request_log_line(
            "ts",
            "id4",
            &Op::Screenshot {
                path: Some("/Users/uri/secret.png".into()),
            },
            false,
            Some("screenshot_unavailable"),
        );
        assert!(shot.contains("op=screenshot"), "{shot}");
        assert!(!shot.contains("/Users/uri"), "{shot}");
        assert!(shot.contains("ok=false"), "{shot}");
        assert!(shot.contains("error=screenshot_unavailable"), "{shot}");
    }

    #[test]
    fn assert_logs_target_not_expected_name() {
        let line = format_request_log_line(
            "ts",
            "id5",
            &Op::Assert {
                spec: AssertSpec {
                    target: "page-global-dashboard".into(),
                    name: Some("Acme Trading".into()),
                    ..AssertSpec::default()
                },
            },
            true,
            None,
        );
        assert!(line.contains("target=page-global-dashboard"), "{line}");
        assert!(!line.contains("Acme"), "{line}");
    }
}
