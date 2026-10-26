//! The closed tool table. Each MCP tool maps to one fixed [`Action`], and
//! nothing outside this table can be reached: tool arguments are rebuilt
//! from known keys only, and every host op name comes from here.
//!
//! Draft-only by construction: there is no submit, queue, file or payment
//! tool, and `tools_list_has_no_submit_queue_file_or_pay_tool` keeps it that way.
//! `bir_form_review` only brings a validated draft up in BIR with its Submit
//! button highlighted; the user's own click is what files it.
use bir_core::forms::taxform::{self, TaxformArgs};
use serde_json::{Map, Value, json};

/// What a tool reaches on a BIR host.
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

/// Read-only app identity (`{app, mode, database, profiles}`), used to list
/// and to connect.
pub const APP_INFO: HostCall = HostCall::Invoke("app.info");

/// What a tool does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// List running BIR apps; `app.info` on each one with a known token.
    ListApps,
    /// Select one app for this `bir-mcp` process, after `app.info`.
    Connect,
    /// Report the selection; `hello` on the selected app.
    Status,
    /// One host op on the selected app. Refused until an app is selected.
    Host(HostCall),
}

impl Action {
    /// Every host op this action can reach.
    pub fn ops(self) -> Vec<&'static str> {
        match self {
            Action::ListApps | Action::Connect => vec![APP_INFO.op_name()],
            Action::Status => vec![HostCall::Hello.op_name()],
            Action::Host(call) => vec![call.op_name()],
        }
    }
}

pub struct Tool {
    pub name: &'static str,
    pub action: Action,
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

/// Sources the agent may claim. `user` is deliberately absent: only a person
/// typing in BIR produces a `user` box, or the user's own answer on the chat
/// form card (`from_card`, see [`USER_SOURCE`]).
pub const FILL_SOURCES: [&str; 4] = ["profile", "past_return", "document", "ai"];
/// Allowed only with `from_card: true`, for the user's own answers to a
/// `bir_form_card` card; BIR also checks each box is on that card.
pub const USER_SOURCE: &str = "user";

pub static TOOLS: &[Tool] = &[
    Tool {
        name: "bir_list_apps",
        action: Action::ListApps,
        description: "List every running BIR app (desktop windows and headless daemons): its id, pid, address, mode, whether bir-mcp holds its agent token, and, when it does, whether it uses a test or the REAL taxpayer database and which taxpayers it holds. Call this first. Show the list to the user and ask which app to use; never pick one yourself, even when only one is listed.",
        schema: empty_schema,
        build_args: no_args,
    },
    Tool {
        name: "bir_connect",
        action: Action::Connect,
        description: "Select the BIR app the user chose, by `id` exactly as bir_list_apps returned it. Every other bir_* tool works only on the selected app. An app using the REAL taxpayer database is refused unless `confirm_real` is true: set it only after the user has explicitly confirmed, in this chat, that they want to work on real taxpayer data. A failed bir_connect leaves no app selected.",
        schema: connect_schema,
        build_args: connect_args,
    },
    Tool {
        name: "bir_status",
        action: Action::Status,
        description: "Report which BIR app is selected (id, mode, test or real database) and check it is still reachable. With no app selected it says so; then call bir_list_apps, ask the user which app, and bir_connect.",
        schema: empty_schema,
        build_args: no_args,
    },
    Tool {
        name: "bir_profiles_list",
        action: Action::Host(HostCall::Invoke("profile.list")),
        description: "List the taxpayer profiles saved in BIR (TIN, branch, registered name, RDO). Use this to let the user pick which taxpayer the return is for; pass the chosen `tin` exactly as returned to bir_form_open.",
        schema: empty_schema,
        build_args: no_args,
    },
    Tool {
        name: "bir_dues_list",
        action: Action::Host(HostCall::Invoke("dues.list")),
        description: "List tax returns that are due, according to BIR's filing calendar. Useful to suggest which form and period to prepare. `filter`: upcoming (default), overdue, or all. `scope`: profile (the currently selected taxpayer) or global (every taxpayer).",
        schema: dues_schema,
        build_args: dues_args,
    },
    Tool {
        name: "bir_form_open",
        action: Action::Host(HostCall::Invoke("form.open")),
        description: "Open a draft return in BIR for one taxpayer and period, exactly as the form's page in BIR opens it (prefilled from the taxpayer profile; the saved draft if there is one). Any of BIR's 43 forms, by code (e.g. 1601C, 2551Q, 2550Q, 0619E, 1701, 1702RT). `period`: month 1-12 for monthly forms, quarter 1-4 for quarterly forms, 0 (or omitted) for annual returns, the return number for as-needed forms (0605, 1707); or a period as text (`2026-Q3`, `Q3 2026`, `2026-01`, `jan 2026`, `2025`). `code` also takes a return token's value as the user wrote it (`000000000/2551Q-2026-Q3`, with or without `@bir:taxform:` before it): its TIN and period then need no `tin`, `year` or `period`. With no year and no period, the return to file now opens (the last closed month or quarter, the previous year, today for an as-needed form). Only one form can be open at a time: if BIR answers \"form already open\", do NOT retry; ask the user to save the open draft (bir_form_save_draft) or dismiss it (bir_form_dismiss) first.",
        schema: open_schema,
        build_args: open_args,
    },
    Tool {
        name: "bir_form_fields",
        action: Action::Host(HostCall::Invoke("form.fields")),
        description: "Read every box of the open form that you may fill, from the form's official definition: official key, item number, label (as printed on the form), kind (text, radio, checkbox, select), options, current value and where it came from (profile, past_return, document, user, ai). A radio item is one box (its group key, e.g. frm0619E:optWithheld) whose options are its choices and whose value is the chosen option. Computed boxes, hidden controls and boxes owned by the profile or the period are left out (counted in `read_only`). Use these keys with bir_form_fill. 1601C and 2551Q also list their older short keys under `boxes`.",
        schema: empty_schema,
        build_args: no_args,
    },
    Tool {
        name: "bir_form_context",
        action: Action::Host(HostCall::Invoke("form.context")),
        description: "Read what BIR knows that can ground the open form: the taxpayer profile (COR facts), tax elections the user has stored (e.g. 8% option) - these are never inferred, past returns for this TIN, and uploaded documents (OCR) for the period. Fill only what this context supports.",
        schema: empty_schema,
        build_args: no_args,
    },
    Tool {
        name: "bir_form_fill",
        action: Action::Host(HostCall::Invoke("form.fill")),
        description: "Fill boxes of the open form. `fields` maps official box keys (from bir_form_fields) to values: amounts like \"1234.50\", dates as MM/DD/YYYY, a radio item by its group key with one option value or label (e.g. {\"frm0619E:optWithheld\": \"Y\"}), checkboxes as true/false, selects as one of their options. BIR recomputes the form after each fill. `source` says where the values came from: profile, past_return, document, or ai (default; use ai only for values you derived yourself). Call once per source. When the user answered a bir_form_card card, send their answers with `source: \"user\"` and `from_card: true`. Boxes the user typed are never overwritten: they come back in `kept_user_boxes`. Boxes the form does not show yet (an item must be answered first) come back in `not_shown`. Never fill a tax election (such as the 8% option) unless bir_form_context shows it stored. Computed boxes cannot be filled.",
        schema: fill_schema,
        build_args: fill_args,
    },
    Tool {
        name: "bir_form_needs_you",
        action: Action::Host(HostCall::Invoke("form.needs_you")),
        description: "List the boxes of the open form that still need an answer (\"needs you\"): every fillable box BIR's own validation names (with its message) and, while the form does not validate, the required fillable boxes still empty. A form that validates needs nothing. Errors on boxes nobody can fill here come back in `other_errors`. Ask the user for each one in chat (bir_form_card shows them as a form), or let them type it directly in BIR.",
        schema: empty_schema,
        build_args: no_args,
    },
    Tool {
        name: "bir_form_validate",
        action: Action::Host(HostCall::Invoke("form.validate")),
        description: "Validate the open form exactly as its page's Validate button does (BIR's official rules for that form, after recomputing). Returns `ok`, an error count, and `field_errors`: a list of {field, message} per box. Explain each field error to the user in plain words.",
        schema: empty_schema,
        build_args: no_args,
    },
    Tool {
        name: "bir_form_send",
        action: Action::Host(HostCall::Invoke("form.send")),
        description: "Queue the open form for upload from this computer, after the same validation as bir_form_validate. Requires a standing OK for this taxpayer and form, which only the user grants (in the Forms app or on the taxpayer page). Without that OK the result is approval_required and pending, nothing is queued, and the user either presses Submit in the Forms app themselves or grants the assistant permission: tell them so. An invalid form is not queued: field_errors are the page's Validate errors. A dry-run build queues without contacting BIR.",
        schema: empty_schema,
        build_args: no_args,
    },
    Tool {
        name: "bir_form_preview",
        action: Action::Host(HostCall::Invoke("form.preview")),
        description: "Render the open draft on BIR's official printed sheet, filled, as a local HTML file (offline; no PDF is made). In the BIR desktop app it also opens in the browser the way the page's Print preview does; headless, only `path` comes back. Show the user the path so they can look before anything is filed. It changes nothing.",
        schema: empty_schema,
        build_args: no_args,
    },
    Tool {
        name: "bir_form_card",
        action: Action::Host(HostCall::Invoke("form.card")),
        description: "Get the open form's open questions as structured data for a chat form card (AG-UI request_user_form): the needs-you boxes and every box with a validation error, each with key, item number, label, kind, options, current value and error text. Render it as a form for the user; send their answers back with bir_form_fill using `source: \"user\"` and `from_card: true`.",
        schema: empty_schema,
        build_args: no_args,
    },
    Tool {
        name: "bir_form_review",
        action: Action::Host(HostCall::Invoke("form.review_in_app")),
        description: "Hand a finished draft to the user in the BIR desktop app: only when bir_form_validate shows no errors. BIR saves the draft, brings its window to that form's page (still a Draft), runs the page's Validate, highlights the Submit button and shows \"Prepared by the assistant. Review every box, then click Submit yourself.\" It never files anything: the user reviews and clicks Submit in BIR themselves. Refused by a headless BIR app.",
        schema: empty_schema,
        build_args: no_args,
    },
    Tool {
        name: "bir_form_save_draft",
        action: Action::Host(HostCall::Invoke("form.save_draft")),
        description: "Save the open form as a draft in BIR. This never submits, queues, files or pays anything; the user reviews and files from BIR themselves.",
        schema: empty_schema,
        build_args: no_args,
    },
    Tool {
        name: "bir_form_dismiss",
        action: Action::Host(HostCall::Invoke("form.dismiss")),
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

fn connect_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "id": {
                "type": "string",
                "description": "App id exactly as bir_list_apps returned it (a connect id such as `bir-demo`, or `pid:<pid>`)."
            },
            "confirm_real": {
                "type": "boolean",
                "default": false,
                "description": "Required true for an app on the REAL taxpayer database. Only after the user explicitly confirmed it in this chat."
            }
        },
        "required": ["id"],
        "additionalProperties": false
    })
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
                "description": "BIR form code, e.g. 1601C, 2551Q, 2550Q, 0619E, 1701, 1702RT (any of the 43 forms BIR has), or a return token's value such as 000000000/2551Q-2026-Q3 (TIN, form and period)."
            },
            "year": {
                "type": "integer",
                "minimum": 2000,
                "maximum": 2100,
                "description": "Taxable year, e.g. 2026. Not needed when `code` or `period` names the period."
            },
            "period": {
                "type": ["integer", "string"],
                "minimum": 0,
                "maximum": 999,
                "description": "Monthly forms: month 1-12. Quarterly forms: quarter 1-4. Annual returns: 0 (or omit). As-needed forms: the return number (1 for the first). Or a period as text: 2026-Q3, Q3 2026, 2026-01, jan 2026, 2025."
            },
            "tin": {
                "type": "string",
                "description": "Taxpayer TIN exactly as returned by bir_profiles_list. Not needed when `code` is a token value naming the TIN."
            }
        },
        "required": ["code"],
        "additionalProperties": false
    })
}

fn fill_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "fields": {
                "type": "object",
                "description": "Official box key -> value. Keys come from bir_form_fields. Amounts as plain numbers or strings like \"1234.50\".",
                "additionalProperties": { "type": ["string", "number", "boolean"] },
                "minProperties": 1
            },
            "source": {
                "type": "string",
                "enum": ["profile", "past_return", "document", "ai", "user"],
                "default": "ai",
                "description": "Where these values came from: profile (COR facts), past_return, document (uploaded OCR), or ai (your own derivation). `user` only with from_card: true, for the user's own answers to bir_form_card."
            },
            "from_card": {
                "type": "boolean",
                "default": false,
                "description": "True when these are the user's answers to a bir_form_card card (then source is `user`)."
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

fn connect_args(args: &Map<String, Value>) -> Result<Value, String> {
    reject_unknown(args, &["id", "confirm_real"])?;
    let id = args
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .ok_or("`id` is required: pick it with the user from bir_list_apps")?;
    let confirm_real = match args.get("confirm_real") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(confirm)) => *confirm,
        Some(other) => {
            return Err(format!(
                "`confirm_real` must be true or false (got {other})"
            ));
        }
    };
    Ok(json!({ "id": id, "confirm_real": confirm_real }))
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

/// `1601-C`, `form 1601c` -> `1601C`. BIR checks the code is one of its forms.
fn form_code(raw: &str) -> Option<String> {
    let upper = raw.trim().to_ascii_uppercase();
    let upper = upper.strip_prefix("FORM").unwrap_or(&upper);
    let code: String = upper.chars().filter(char::is_ascii_alphanumeric).collect();
    let digits = code.chars().take_while(char::is_ascii_digit).count();
    (digits == 4 && code.len() <= 7).then_some(code)
}

fn open_args(args: &Map<String, Value>) -> Result<Value, String> {
    reject_unknown(args, &["code", "year", "period", "tin"])?;
    let raw = args.get("code").and_then(Value::as_str).ok_or(
        "`code` is required (a BIR form code such as 1601C or 2550Q, or a return token such as 000000000/2551Q-2026-Q3)",
    )?;
    let today = taxform::today();
    // A plain code as before; otherwise a token value naming the TIN and
    // the period too (`000000000/2551Q-2026-Q3`).
    let (code, named) = match form_code(raw) {
        Some(code) => (code, TaxformArgs::default()),
        None => {
            let named = taxform::parse(raw, today);
            match named.form {
                Some(code) if named.unknown.is_empty() => (code.to_string(), named),
                _ => {
                    return Err(format!(
                        "`{raw}` is not a BIR form code (e.g. 1601C, 2550Q)"
                    ));
                }
            }
        }
    };
    // A number up to 999 is the page's period; `2025` is a year.
    let text_period = match args.get("period") {
        Some(Value::String(text)) if text.trim().parse::<u64>().map_or(true, |n| n > 999) => {
            let read = taxform::parse(text, today);
            match read.period {
                Some(period) if read.unknown.is_empty() && read.form.is_none() => Some(period),
                _ => {
                    return Err(format!(
                        "`period` `{text}` is not a period (e.g. 2026-Q3, jan 2026, 2025)"
                    ));
                }
            }
        }
        _ => None,
    };
    let numbered = match args.get("period") {
        None | Some(Value::Null) => None,
        Some(_) if text_period.is_some() => None,
        Some(_) => Some(integer(args, "period")?),
    };
    let year_arg = match args.get("year") {
        None | Some(Value::Null) => None,
        Some(_) => Some(integer(args, "year")?),
    };
    // The named period, read as this form files it (a month of a
    // quarterly form is its quarter).
    let period = match text_period.or(named.period) {
        Some(period) => Some(
            period
                .for_form(&code, today)
                .map_err(|why| format!("`{raw}`: {why}"))?,
        ),
        None if year_arg.is_none() && numbered.is_none() => taxform::period_to_file(&code, today),
        None => None,
    };
    let (year, period) = match (period, year_arg, numbered) {
        (Some(period), year, None) => {
            let (named_year, number) = period.open_args();
            if let Some(year) = year.filter(|year| *year != u64::from(named_year)) {
                return Err(format!(
                    "`year` {year} and the period {} disagree",
                    period.label()
                ));
            }
            (u64::from(named_year), u64::from(number))
        }
        (_, Some(year), number) => (year, number.unwrap_or(0)),
        (_, None, _) => {
            return Err("`year` is required (or name the period, e.g. 2026-Q3)".into());
        }
    };
    if !(2000..=2100).contains(&year) {
        return Err(format!("`year` {year} is out of range"));
    }
    if period > 999 {
        return Err(format!("`period` {period} is out of range"));
    }
    let tin_arg = args
        .get("tin")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|tin| !tin.is_empty());
    // A token's 9-digit TIN is the head office.
    let named_tin = named.tin.map(|tin| {
        if tin.len() == 9 {
            format!("{tin}00000")
        } else {
            tin
        }
    });
    let tin = match (tin_arg, named_tin) {
        (Some(tin), Some(named)) => {
            let digits: String = tin.chars().filter(char::is_ascii_digit).collect();
            if !named.starts_with(&digits) {
                return Err(format!("`tin` {tin} is not the TIN `{raw}` names"));
            }
            tin.to_string()
        }
        (Some(tin), None) => tin.to_string(),
        (None, Some(named)) => named,
        (None, None) => {
            return Err("`tin` is required: pick the taxpayer with bir_profiles_list".into());
        }
    };
    Ok(json!({ "code": code, "year": year, "period": period, "tin": tin }))
}

fn fill_args(args: &Map<String, Value>) -> Result<Value, String> {
    reject_unknown(args, &["fields", "source", "from_card"])?;
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
    let from_card = match args.get("from_card") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(flag)) => *flag,
        Some(other) => return Err(format!("`from_card` must be true or false (got {other})")),
    };
    if from_card {
        // The user's own answers to a chat form card, and nothing else.
        return match args.get("source").and_then(Value::as_str) {
            Some(USER_SOURCE) => {
                Ok(json!({ "fields": fields, "source": USER_SOURCE, "from_card": true }))
            }
            _ => Err("answers from a form card are sent with `source: \"user\"`".into()),
        };
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
    const ALLOWED_OPS: [&str; 16] = [
        "hello",
        "app.info",
        "profile.list",
        "dues.list",
        "form.open",
        "form.fields",
        "form.context",
        "form.fill",
        "form.needs_you",
        "form.validate",
        "form.send",
        "form.preview",
        "form.card",
        "form.review_in_app",
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
        // The app-selection tools are listed and checked like every other.
        for selection_tool in ["bir_list_apps", "bir_connect", "bir_status"] {
            assert!(
                names.contains(&selection_tool),
                "{selection_tool} not listed"
            );
        }
        let ops: Vec<&str> = TOOLS.iter().flat_map(|tool| tool.action.ops()).collect();
        assert!(ops.contains(&"app.info"), "{ops:?}");
        for name in names.iter().chain(ops.iter()) {
            assert_eq!(banned(name), None, "`{name}`");
        }
        // Argument names of every tool, too (e.g. `confirm_real`).
        for tool in TOOLS {
            let schema = tool.input_schema();
            for key in schema["properties"].as_object().unwrap().keys() {
                assert_eq!(banned(key), None, "{}.{key}", tool.name);
            }
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
        // The one send the user can grant. Every other send/file/queue name
        // stays banned, including form.file and form.queue.
        if name == "bir_form_send" || name == "form.send" {
            return None;
        }
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
        let ops: HashSet<&str> = TOOLS.iter().flat_map(|tool| tool.action.ops()).collect();
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
        // Any of the 43 forms by code; BIR checks the code and the period.
        assert_eq!(
            open.host_args(&obj(
                json!({"code": "form 1702-rt", "year": 2025, "tin": "1"})
            ))
            .unwrap(),
            json!({"code": "1702RT", "year": 2025, "period": 0, "tin": "1"})
        );
        for bad in [
            json!({"code": "17O1", "year": 2026, "period": 1, "tin": "1"}),
            json!({"code": "2551Q", "year": 2026, "period": 1000, "tin": "1"}),
            json!({"code": "1601C", "year": 1999, "period": 12, "tin": "1"}),
            json!({"code": "1601C", "year": 2026, "period": 1}),
            json!({"code": "1601C", "year": 2026, "period": 1, "tin": "1", "confirm": true}),
        ] {
            assert!(open.host_args(&obj(bad.clone())).is_err(), "{bad}");
        }
    }

    #[test]
    fn open_args_take_a_return_token_and_a_period_as_text() {
        let open = find("bir_form_open").unwrap();
        let expected = json!({"code": "2551Q", "year": 2026, "period": 3, "tin": "00000000000000"});
        for code in [
            "000000000/2551Q-2026-Q3",
            "@bir:taxform:000000000/2551Q-2026-Q3",
        ] {
            assert_eq!(
                open.host_args(&obj(json!({ "code": code }))).unwrap(),
                expected,
                "{code}"
            );
        }
        // The TIN may also be given, if it is the token's.
        assert_eq!(
            open.host_args(&obj(
                json!({"code": "000000000/2551Q-2026-Q3", "tin": "00000000000000"})
            ))
            .unwrap(),
            expected
        );
        assert!(
            open.host_args(&obj(
                json!({"code": "000000000/2551Q-2026-Q3", "tin": "111111111"})
            ))
            .is_err()
        );
        assert!(
            open.host_args(&obj(
                json!({"code": "000000000/2551Q-2026-Q3", "year": 2025})
            ))
            .is_err()
        );
        // A period as text, read as the form files it.
        assert_eq!(
            open.host_args(&obj(
                json!({"code": "2551Q", "period": "Q3 2026", "tin": "1"})
            ))
            .unwrap(),
            json!({"code": "2551Q", "year": 2026, "period": 3, "tin": "1"})
        );
        assert_eq!(
            open.host_args(&obj(
                json!({"code": "1601-C", "period": "jan 2026", "tin": "1"})
            ))
            .unwrap(),
            json!({"code": "1601C", "year": 2026, "period": 1, "tin": "1"})
        );
        assert_eq!(
            open.host_args(&obj(json!({"code": "1701", "period": "2025", "tin": "1"})))
                .unwrap(),
            json!({"code": "1701", "year": 2025, "period": 0, "tin": "1"})
        );
        // Numbers stay numbers, numeric strings included.
        assert_eq!(
            open.host_args(&obj(
                json!({"code": "2551Q", "year": 2026, "period": "2", "tin": "1"})
            ))
            .unwrap()["period"],
            2
        );
        assert!(
            open.host_args(&obj(
                json!({"code": "1601C", "period": "Q3 2026", "tin": "1"})
            ))
            .is_err(),
            "a quarter of a monthly form"
        );
        assert!(
            open.host_args(&obj(json!({"code": "2551Q", "period": "soon", "tin": "1"})))
                .is_err()
        );
        // Neither year nor period: the return to file now.
        let now = open
            .host_args(&obj(json!({"code": "2551Q", "tin": "1"})))
            .unwrap();
        assert!(
            now["year"].as_u64().is_some() && (1..=4).contains(&now["period"].as_u64().unwrap())
        );
    }

    #[test]
    fn connect_args_need_an_id_and_a_boolean_confirm_real() {
        let connect = find("bir_connect").unwrap();
        assert_eq!(
            connect
                .host_args(&obj(json!({"id": " bir-demo "})))
                .unwrap(),
            json!({"id": "bir-demo", "confirm_real": false})
        );
        assert_eq!(
            connect
                .host_args(&obj(json!({"id": "pid:42", "confirm_real": true})))
                .unwrap(),
            json!({"id": "pid:42", "confirm_real": true})
        );
        for bad in [
            json!({}),
            json!({"id": ""}),
            json!({"id": 42}),
            json!({"id": "bir-demo", "confirm_real": "yes"}),
            json!({"id": "bir-demo", "token": "x"}),
        ] {
            assert!(connect.host_args(&obj(bad.clone())).is_err(), "{bad}");
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
        // `user` only for the user's answers to a form card.
        assert_eq!(
            fill.host_args(&obj(
                json!({"fields": {"a": "1"}, "source": "user", "from_card": true})
            ))
            .unwrap(),
            json!({"fields": {"a": "1"}, "source": "user", "from_card": true})
        );
        assert!(
            fill.host_args(&obj(
                json!({"fields": {"a": 1}, "source": "ai", "from_card": true})
            ))
            .is_err()
        );
        assert!(
            fill.host_args(&obj(json!({"fields": {"a": 1}, "from_card": "yes"})))
                .is_err()
        );
    }

    #[test]
    fn review_tool_hands_over_and_never_files() {
        let review = find("bir_form_review").unwrap();
        assert_eq!(review.action.ops(), vec!["form.review_in_app"]);
        assert_eq!(banned(review.name), None);
        assert!(review.description.contains("click Submit"));
        assert!(review.host_args(&obj(json!({"confirm": true}))).is_err());
        for name in ["bir_form_preview", "bir_form_card"] {
            assert!(find(name).is_some(), "{name}");
        }
    }
}
