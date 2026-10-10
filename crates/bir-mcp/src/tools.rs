//! The closed tool table. Each MCP tool maps to exactly one BIR host call,
//! and nothing outside this table can be reached: tool arguments are rebuilt
//! from known keys only, and the host op name always comes from here.
//!
//! Draft-only by construction: there is no submit, queue, file or payment
//! tool, and `tools_list_has_no_submit_queue_file_or_pay_tool` keeps it that way.
use serde_json::{Map, Value, json};

/// What a tool reaches on the BIR host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HostCall {
    /// gpui-agent `hello` (connection + host identity).
    Hello,
    /// gpui-agent `invoke {name, args}` with a fixed, allow-listed name.
    Invoke(&'static str),
}

impl HostCall {
    pub fn op_name(self) -> &'static str {
        match self {
            HostCall::Hello => "hello",
            HostCall::Invoke(name) => name,
        }
    }
}

pub struct Tool {
    pub name: &'static str,
    pub call: HostCall,
    pub description: &'static str,
    schema: fn() -> Value,
    build_args: fn(&Map<String, Value>) -> Result<Value, String>,
}

impl Tool {
    pub fn input_schema(&self) -> Value {
        (self.schema)()
    }

    /// Validate the caller's arguments and rebuild the host args from the
    /// known keys only. Unknown keys are refused so nothing extra (for
    /// example a `confirm` flag) can ride along to the host.
    pub fn host_args(&self, args: &Map<String, Value>) -> Result<Value, String> {
        (self.build_args)(args)
    }

    pub fn descriptor(&self) -> Value {
        json!({
            "name": self.name,
            "description": self.description,
            "inputSchema": self.input_schema(),
        })
    }
}

pub const FORM_CODES: [&str; 2] = ["2551Q", "1601C"];
/// Sources the agent may claim. `user` is deliberately absent: only a person
/// typing in BIR produces a `user` box.
pub const FILL_SOURCES: [&str; 4] = ["profile", "past_return", "document", "ai"];

pub static TOOLS: &[Tool] = &[
    Tool {
        name: "bir_status",
        call: HostCall::Hello,
        description: "Check that the BIR app is running and reachable. Returns which BIR instance was found (desktop window or headless daemon), its address, and host info. Call this first; if it fails, ask the user to open the BIR desktop app with the agent enabled.",
        schema: empty_schema,
        build_args: no_args,
    },
    Tool {
        name: "bir_profiles_list",
        call: HostCall::Invoke("profile.list"),
        description: "List the taxpayer profiles saved in BIR (TIN, branch, registered name, RDO). Use this to let the user pick which taxpayer the return is for; pass the chosen `tin` exactly as returned to bir_form_open.",
        schema: empty_schema,
        build_args: no_args,
    },
    Tool {
        name: "bir_dues_list",
        call: HostCall::Invoke("dues.list"),
        description: "List tax returns that are due, according to BIR's filing calendar. Useful to suggest which form and period to prepare. `filter`: upcoming (default), overdue, or all. `scope`: profile (the currently selected taxpayer) or global (every taxpayer).",
        schema: dues_schema,
        build_args: dues_args,
    },
    Tool {
        name: "bir_form_open",
        call: HostCall::Invoke("form.open"),
        description: "Open a draft return in BIR for one taxpayer and period. Supported forms: 2551Q (quarterly percentage tax; period = quarter 1-4) and 1601C (monthly withholding on compensation; period = month 1-12). Loads the existing draft if one was saved. Only one form can be open at a time: if BIR answers \"form already open\", do NOT retry; ask the user to save the open draft (bir_form_save_draft) or dismiss it (bir_form_dismiss) first.",
        schema: open_schema,
        build_args: open_args,
    },
    Tool {
        name: "bir_form_fields",
        call: HostCall::Invoke("form.fields"),
        description: "Read every box of the open form: key, label, current value, whether it is computed, and where each value came from (profile, past_return, document, user, ai). Use the keys returned here when calling bir_form_fill.",
        schema: empty_schema,
        build_args: no_args,
    },
    Tool {
        name: "bir_form_context",
        call: HostCall::Invoke("form.context"),
        description: "Read what BIR knows that can ground the open form: the taxpayer profile (COR facts), tax elections the user has stored (e.g. 8% option) - these are never inferred, past returns for this TIN, and uploaded documents (OCR) for the period. Fill only what this context supports.",
        schema: empty_schema,
        build_args: no_args,
    },
    Tool {
        name: "bir_form_fill",
        call: HostCall::Invoke("form.fill"),
        description: "Fill boxes of the open form. `fields` maps box keys (from bir_form_fields) to values. `source` says where the values came from: profile, past_return, document, or ai (default; use ai only for values you derived yourself). Call once per source. Boxes the user typed are never overwritten: they come back in `kept_user_boxes`. Never fill a tax election (such as the 8% option) unless bir_form_context shows it stored. Computed boxes cannot be filled.",
        schema: fill_schema,
        build_args: fill_args,
    },
    Tool {
        name: "bir_form_needs_you",
        call: HostCall::Invoke("form.needs_you"),
        description: "List the required boxes of the open form that are still empty (\"needs you\"). Ask the user for each one in chat, or let them type it directly in BIR.",
        schema: empty_schema,
        build_args: no_args,
    },
    Tool {
        name: "bir_form_validate",
        call: HostCall::Invoke("form.validate"),
        description: "Validate the open form with BIR's rules. Returns `ok`, an error count, and `field_errors`: a list of {field, message} per box. Explain each field error to the user in plain words.",
        schema: empty_schema,
        build_args: no_args,
    },
    Tool {
        name: "bir_form_save_draft",
        call: HostCall::Invoke("form.save_draft"),
        description: "Save the open form as a draft in BIR. This never submits, queues, files or pays anything; the user reviews and files from BIR themselves.",
        schema: empty_schema,
        build_args: no_args,
    },
    Tool {
        name: "bir_form_dismiss",
        call: HostCall::Invoke("form.dismiss"),
        description: "Close the open form without saving. BIR refuses with \"unsaved edits\" when the form has changes; in that case tell the user to press Dismiss in BIR (which asks them to confirm) or save the draft first. Never try to work around that refusal.",
        schema: empty_schema,
        build_args: no_args,
    },
];

pub fn find(name: &str) -> Option<&'static Tool> {
    TOOLS.iter().find(|tool| tool.name == name)
}

pub fn list() -> Value {
    json!({ "tools": TOOLS.iter().map(Tool::descriptor).collect::<Vec<_>>() })
}

fn empty_schema() -> Value {
    json!({ "type": "object", "properties": {}, "additionalProperties": false })
}

fn dues_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "filter": {
                "type": "string",
                "enum": ["upcoming", "overdue", "all"],
                "description": "Which dues to list (default upcoming)."
            },
            "scope": {
                "type": "string",
                "enum": ["profile", "global"],
                "description": "profile = selected taxpayer only; global = all taxpayers."
            }
        },
        "additionalProperties": false
    })
}

fn open_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "code": {
                "type": "string",
                "enum": FORM_CODES,
                "description": "BIR form code."
            },
            "year": {
                "type": "integer",
                "minimum": 2000,
                "maximum": 2100,
                "description": "Taxable year, e.g. 2026."
            },
            "period": {
                "type": "integer",
                "minimum": 1,
                "maximum": 12,
                "description": "2551Q: quarter 1-4. 1601C: month 1-12."
            },
            "tin": {
                "type": "string",
                "description": "Taxpayer TIN exactly as returned by bir_profiles_list."
            }
        },
        "required": ["code", "year", "period", "tin"],
        "additionalProperties": false
    })
}

fn fill_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "fields": {
                "type": "object",
                "description": "Box key -> value. Keys come from bir_form_fields. Amounts as plain numbers or strings like \"1234.50\".",
                "additionalProperties": { "type": ["string", "number", "boolean"] },
                "minProperties": 1
            },
            "source": {
                "type": "string",
                "enum": FILL_SOURCES,
                "default": "ai",
                "description": "Where these values came from: profile (COR facts), past_return, document (uploaded OCR), or ai (your own derivation)."
            }
        },
        "required": ["fields"],
        "additionalProperties": false
    })
}

fn reject_unknown(args: &Map<String, Value>, allowed: &[&str]) -> Result<(), String> {
    match args.keys().find(|key| !allowed.contains(&key.as_str())) {
        Some(key) if allowed.is_empty() => {
            Err(format!("this tool takes no arguments (got `{key}`)"))
        }
        Some(key) => Err(format!(
            "unknown argument `{key}` (allowed: {})",
            allowed.join(", ")
        )),
        None => Ok(()),
    }
}

fn no_args(args: &Map<String, Value>) -> Result<Value, String> {
    reject_unknown(args, &[])?;
    Ok(json!({}))
}

fn optional_enum(
    args: &Map<String, Value>,
    key: &str,
    allowed: &[&str],
) -> Result<Option<String>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if allowed.contains(&value.as_str()) => Ok(Some(value.clone())),
        Some(other) => Err(format!(
            "`{key}` must be one of {} (got {other})",
            allowed.join(", ")
        )),
    }
}

fn dues_args(args: &Map<String, Value>) -> Result<Value, String> {
    reject_unknown(args, &["filter", "scope"])?;
    let mut out = Map::new();
    if let Some(filter) = optional_enum(args, "filter", &["upcoming", "overdue", "all"])? {
        out.insert("filter".into(), filter.into());
    }
    if let Some(scope) = optional_enum(args, "scope", &["profile", "global"])? {
        out.insert("scope".into(), scope.into());
    }
    Ok(Value::Object(out))
}

/// Integers may arrive as numbers or numeric strings.
fn integer(args: &Map<String, Value>, key: &str) -> Result<u64, String> {
    let value = args
        .get(key)
        .ok_or_else(|| format!("`{key}` is required"))?;
    let parsed = match value {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    };
    parsed.ok_or_else(|| format!("`{key}` must be a whole number (got {value})"))
}

fn open_args(args: &Map<String, Value>) -> Result<Value, String> {
    reject_unknown(args, &["code", "year", "period", "tin"])?;
    let code = args
        .get("code")
        .and_then(Value::as_str)
        .map(|c| c.trim().to_ascii_uppercase())
        .ok_or("`code` is required (2551Q or 1601C)")?;
    if !FORM_CODES.contains(&code.as_str()) {
        return Err(format!(
            "form `{code}` is not supported here; use 2551Q or 1601C"
        ));
    }
    let year = integer(args, "year")?;
    if !(2000..=2100).contains(&year) {
        return Err(format!("`year` {year} is out of range"));
    }
    let period = integer(args, "period")?;
    let max_period = if code == "2551Q" { 4 } else { 12 };
    if !(1..=max_period).contains(&period) {
        return Err(format!(
            "`period` for {code} must be 1-{max_period} (got {period})"
        ));
    }
    let tin = args
        .get("tin")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|tin| !tin.is_empty())
        .ok_or("`tin` is required: pick the taxpayer with bir_profiles_list")?;
    Ok(json!({ "code": code, "year": year, "period": period, "tin": tin }))
}

fn fill_args(args: &Map<String, Value>) -> Result<Value, String> {
    reject_unknown(args, &["fields", "source"])?;
    let fields = match args.get("fields") {
        Some(Value::Object(fields)) if !fields.is_empty() => fields,
        Some(Value::Object(_)) => return Err("`fields` is empty; nothing to fill".into()),
        _ => return Err("`fields` must be an object of box key -> value".into()),
    };
    if let Some((key, _)) = fields
        .iter()
        .find(|(_, value)| !matches!(value, Value::String(_) | Value::Number(_) | Value::Bool(_)))
    {
        return Err(format!("field `{key}` must be a string, number or boolean"));
    }
    let source = optional_enum(args, "source", &FILL_SOURCES)?.unwrap_or_else(|| "ai".into());
    Ok(json!({ "fields": fields, "source": source }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Op names this adapter may ever reach. Changing this list is a
    /// deliberate, reviewed act.
    const ALLOWED_OPS: [&str; 11] = [
        "hello",
        "profile.list",
        "dues.list",
        "form.open",
        "form.fields",
        "form.context",
        "form.fill",
        "form.needs_you",
        "form.validate",
        "form.save_draft",
        "form.dismiss",
    ];

    fn words(name: &str) -> Vec<String> {
        name.split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(str::to_ascii_lowercase)
            .collect()
    }

    #[test]
    fn tools_list_has_no_submit_queue_file_or_pay_tool() {
        let listed = list();
        let names: Vec<&str> = listed["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        assert_eq!(names.len(), TOOLS.len());
        let ops: Vec<&str> = TOOLS.iter().map(|tool| tool.call.op_name()).collect();
        for name in names.iter().chain(ops.iter()) {
            assert_eq!(banned(name), None, "`{name}`");
        }
        // The checker itself is not vacuous.
        for bad in [
            "filing.submit",
            "form.queue",
            "form.file",
            "bir_file_return",
            "form.mark_paid",
            "payment.mark_paid",
            "bir_pay",
            "form.submit_external",
        ] {
            assert!(banned(bad).is_some(), "`{bad}` slipped through");
        }
        assert_eq!(banned("profile.list"), None);
    }

    fn banned(name: &str) -> Option<String> {
        let lower = name.to_ascii_lowercase();
        if let Some(hit) = ["submit", "queue", "pay", "paid", "filing"]
            .into_iter()
            .find(|banned| lower.contains(banned))
        {
            return Some(hit.into());
        }
        // `profile` legitimately contains "file"; as a word (`file_x`,
        // `x.file`, `x_file`) it must not appear.
        words(name)
            .into_iter()
            .find(|word| ["file", "files", "send", "sftp", "efps"].contains(&word.as_str()))
    }

    #[test]
    fn tool_ops_are_a_closed_allow_list() {
        let ops: HashSet<&str> = TOOLS.iter().map(|tool| tool.call.op_name()).collect();
        let allowed: HashSet<&str> = ALLOWED_OPS.into_iter().collect();
        assert_eq!(ops, allowed);
        let names: HashSet<&str> = TOOLS.iter().map(|tool| tool.name).collect();
        assert_eq!(names.len(), TOOLS.len(), "duplicate tool name");
        assert!(TOOLS.iter().all(|tool| tool.name.starts_with("bir_")));
    }

    #[test]
    fn every_schema_is_an_object_schema() {
        for tool in TOOLS {
            let schema = tool.input_schema();
            assert_eq!(schema["type"], "object", "{}", tool.name);
            assert_eq!(schema["additionalProperties"], false, "{}", tool.name);
            assert!(tool.description.len() > 40, "{}", tool.name);
        }
    }

    fn obj(value: Value) -> Map<String, Value> {
        value.as_object().unwrap().clone()
    }

    #[test]
    fn open_args_validate_code_period_and_tin() {
        let open = find("bir_form_open").unwrap();
        assert_eq!(
            open.host_args(&obj(
                json!({"code": "2551q", "year": "2026", "period": 2, "tin": " 111111114000 "})
            ))
            .unwrap(),
            json!({"code": "2551Q", "year": 2026, "period": 2, "tin": "111111114000"})
        );
        for bad in [
            json!({"code": "1701", "year": 2026, "period": 1, "tin": "1"}),
            json!({"code": "2551Q", "year": 2026, "period": 5, "tin": "1"}),
            json!({"code": "1601C", "year": 2026, "period": 13, "tin": "1"}),
            json!({"code": "1601C", "year": 2026, "period": 1}),
            json!({"code": "1601C", "year": 2026, "period": 1, "tin": "1", "confirm": true}),
        ] {
            assert!(open.host_args(&obj(bad.clone())).is_err(), "{bad}");
        }
    }

    #[test]
    fn fill_args_default_ai_and_refuse_user_source() {
        let fill = find("bir_form_fill").unwrap();
        assert_eq!(
            fill.host_args(&obj(json!({"fields": {"line_14": "100.00"}})))
                .unwrap(),
            json!({"fields": {"line_14": "100.00"}, "source": "ai"})
        );
        assert!(
            fill.host_args(&obj(json!({"fields": {"a": 1}, "source": "user"})))
                .is_err()
        );
        assert!(fill.host_args(&obj(json!({"fields": {}}))).is_err());
        assert!(
            fill.host_args(&obj(json!({"fields": {"a": {"b": 1}}})))
                .is_err()
        );
        assert!(
            fill.host_args(&obj(json!({"fields": {"a": 1}, "confirm": true})))
                .is_err()
        );
    }
}
