#![recursion_limit = "256"]

pub mod editable;
pub mod formtype;
pub mod html;
pub mod html_support;

use bir_core::forms::form_2551q::Form2551QDraft;
use formtype::{Direction, FieldKind, FormField, FormType};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[allow(unused_mut)]
fn build_typst_command() -> Command {
    let mut cmd = Command::new(get_typst_binary());
    #[cfg(windows)]
    {
        // CREATE_NO_WINDOW = 0x08000000
        cmd.creation_flags(0x08000000);
    }
    cmd
}

const FORM_2551Q_ID: &str = "2551Qv2018";
const LAYOUT_2551Q: &str = include_str!("../../../formtypes/2551Qv2018/formtype.json");
const TEMPLATE_2551Q: &str = include_str!("../../../formtypes/2551Qv2018/template.typ");
const PAGE1_SVG_2551Q: &str = include_str!("../../../formtypes/2551Qv2018/pages/page1.svg");
const PAGE2_SVG_2551Q: &str = include_str!("../../../formtypes/2551Qv2018/pages/page2.svg");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaperSize {
    A4,
    Letter,
    Legal,
}

impl PaperSize {
    fn points(self) -> (u32, u32) {
        match self {
            PaperSize::A4 => (595, 842),
            PaperSize::Letter => (612, 792),
            PaperSize::Legal => (612, 1008),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PrintRequest {
    pub form_id: String,
    pub fields: BTreeMap<String, String>,
    pub output_dir: PathBuf,
    /// Base directory containing `{form_id}/formtype.json`, `template.typ`,
    /// and `pages/*.svg`. When `None`, falls back to the embedded 2551Q
    /// constants (backward-compatible).
    pub formtypes_dir: Option<PathBuf>,
}

impl PrintRequest {
    pub fn new(
        form_id: impl Into<String>,
        fields: BTreeMap<String, String>,
        output_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            form_id: form_id.into(),
            fields,
            output_dir: output_dir.into(),
            formtypes_dir: None,
        }
    }

    /// Set the formtypes base directory for data-driven form loading.
    pub fn with_formtypes_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.formtypes_dir = Some(dir.into());
        self
    }
}

#[derive(Debug, Clone)]
pub struct PrintResult {
    pub pdf_path: PathBuf,
    pub preview_png_paths: Vec<PathBuf>,
    pub typ_path: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum PrintError {
    #[error("unsupported form id: {0}")]
    UnsupportedForm(String),
    #[error("layout is invalid: {0}")]
    InvalidLayout(String),
    #[error("missing required print fields: {0:?}")]
    MissingFields(Vec<String>),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("typst compile failed: {0}")]
    TypstCompile(String),
    #[error("embedded typst compiler unavailable: {0}")]
    EmbeddedUnavailable(String),
    #[error("preview export failed: {0}")]
    Preview(String),
    #[error("legacy 2551Q preview would omit form data: {0}")]
    UnsafeLegacy2551Q(Legacy2551QPrintIssue),
}

/// A field that the snapshot-based 2551Q renderer cannot reproduce without
/// dropping data. The semantic HTML renderer has adaptive text boxes for these
/// cases, but the legacy path must fail closed until it can do the same.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Legacy2551QPrintIssue {
    FieldOverflow {
        field: &'static str,
        label: &'static str,
        character_count: usize,
        capacity: usize,
    },
    UnsupportedNonEmptyField {
        field: &'static str,
        label: &'static str,
    },
}

impl std::fmt::Display for Legacy2551QPrintIssue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FieldOverflow {
                label,
                character_count,
                capacity,
                ..
            } => write!(
                formatter,
                "{label} has {character_count} characters, but the legacy form has capacity for {capacity}"
            ),
            Self::UnsupportedNonEmptyField { label, .. } => {
                write!(formatter, "the legacy form does not render {label}")
            }
        }
    }
}

/// Return the first field that would be truncated or omitted by the legacy
/// snapshot-overlay renderer. Page 2 has the tightest taxpayer-name field, so
/// its 26-character capacity governs the repeated name on both pages.
pub fn legacy_2551q_print_issue(draft: &Form2551QDraft) -> Option<Legacy2551QPrintIssue> {
    for (field, label, value, capacity) in [
        (
            "taxpayer_name",
            "the taxpayer name",
            draft.taxpayer_name.as_str(),
            26,
        ),
        (
            "registered_address",
            "the registered address",
            draft.registered_address.as_str(),
            71,
        ),
        (
            "contact_number",
            "the contact number",
            draft.contact_number.as_str(),
            12,
        ),
        ("email", "the email address", draft.email.as_str(), 28),
    ] {
        let character_count = value.chars().count();
        if character_count > capacity {
            return Some(Legacy2551QPrintIssue::FieldOverflow {
                field,
                label,
                character_count,
                capacity,
            });
        }
    }

    if draft.tax_relief && !draft.tax_relief_specification.trim().is_empty() {
        return Some(Legacy2551QPrintIssue::UnsupportedNonEmptyField {
            field: "tax_relief_specification",
            label: "Item 12A tax-relief specification text",
        });
    }
    if !draft.other_tax_credit_description.trim().is_empty() {
        return Some(Legacy2551QPrintIssue::UnsupportedNonEmptyField {
            field: "other_tax_credit_description",
            label: "Item 17 other-credit specification text",
        });
    }

    None
}

pub trait TypstCompiler {
    fn compile_pdf(
        &self,
        typ_path: &Path,
        pdf_path: &Path,
        root_dir: &Path,
    ) -> Result<(), PrintError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct EmbeddedTypstCompiler;

impl TypstCompiler for EmbeddedTypstCompiler {
    fn compile_pdf(
        &self,
        _typ_path: &Path,
        _pdf_path: &Path,
        _root_dir: &Path,
    ) -> Result<(), PrintError> {
        Err(PrintError::EmbeddedUnavailable(
            "full embedded Typst world is not wired yet; falling back to typst CLI".to_string(),
        ))
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct CliTypstCompiler;

fn get_typst_binary() -> std::ffi::OsString {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            let bundled_typst = dir.join(if cfg!(windows) { "typst.exe" } else { "typst" });
            if bundled_typst.exists() {
                return bundled_typst.into_os_string();
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        let packaged_typst = Path::new("/usr/lib/ebirforms/typst");
        if packaged_typst.exists() {
            return packaged_typst.as_os_str().to_os_string();
        }
    }
    std::ffi::OsString::from("typst")
}

impl TypstCompiler for CliTypstCompiler {
    fn compile_pdf(
        &self,
        typ_path: &Path,
        pdf_path: &Path,
        root_dir: &Path,
    ) -> Result<(), PrintError> {
        let output = build_typst_command()
            .arg("compile")
            .arg("--root")
            .arg(root_dir)
            .arg(typ_path)
            .arg(pdf_path)
            .output()
            .map_err(|err| PrintError::TypstCompile(format!("failed to run typst: {err}")))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(PrintError::TypstCompile(command_output(&output)))
        }
    }
}

/// Render a flat (official-looking) PDF for Form 2551Q.
pub fn render_2551q_flat(
    draft: &Form2551QDraft,
    output_dir: impl Into<PathBuf>,
    formtypes_dir: Option<PathBuf>,
) -> Result<PrintResult, PrintError> {
    if let Some(issue) = legacy_2551q_print_issue(draft) {
        return Err(PrintError::UnsafeLegacy2551Q(issue));
    }
    let mut req = PrintRequest::new(FORM_2551Q_ID, draft.to_bir_field_map(), output_dir);
    if let Some(dir) = formtypes_dir {
        req = req.with_formtypes_dir(dir);
    }
    render_flat_pdf(req)
}

/// Backward-compatible alias for [`render_2551q_flat`].
pub fn render_2551q_print(
    draft: &Form2551QDraft,
    output_dir: impl Into<PathBuf>,
    formtypes_dir: Option<PathBuf>,
) -> Result<PrintResult, PrintError> {
    render_2551q_flat(draft, output_dir, formtypes_dir)
}

/// Render a flat (official-looking) PDF from a `PrintRequest`.
///
/// **Rendering priority:**
/// 1. If `form.typ` exists in the formtypes directory → Typst-native path
///    (form.typ draws the entire form: background + dynamic fields).
///    Field data is injected via `data.json` in the output directory.
/// 2. Otherwise → SVG-background path (existing behavior).
///    Loads SVG pages as backgrounds, overlays field values using template.typ macros.
pub fn render_flat_pdf(request: PrintRequest) -> Result<PrintResult, PrintError> {
    fs::create_dir_all(&request.output_dir)?;

    // ── Path 1: Typst-native form (form.typ draws everything) ──
    if let Some(form_typ_content) =
        load_form_typ(&request.form_id, request.formtypes_dir.as_deref())
    {
        let typ_path = request.output_dir.join("form.typ");
        let pdf_path = request.output_dir.join("generated.pdf");
        let data_path = request.output_dir.join("data.json");

        // Write field data as JSON for form.typ to consume
        let data_json = serde_json::to_string_pretty(&request.fields)?;
        fs::write(&data_path, data_json)?;

        // Write the template macros alongside form.typ (it may #import them)
        if let Ok(template) =
            load_template_resolved(&request.form_id, request.formtypes_dir.as_deref())
        {
            fs::write(request.output_dir.join("template.typ"), template)?;
        }

        // Write form.typ
        fs::write(&typ_path, form_typ_content)?;

        // Ensure SVGs are copied so Typst can use them as backgrounds
        write_static_assets_resolved(
            &request.form_id,
            &request.output_dir,
            request.formtypes_dir.as_deref(),
        )?;

        let embedded = EmbeddedTypstCompiler;
        let cli = CliTypstCompiler;
        if let Err(_embedded_err) = embedded.compile_pdf(&typ_path, &pdf_path, &request.output_dir)
        {
            cli.compile_pdf(&typ_path, &pdf_path, &request.output_dir)?;
        }

        // Determine page count from formtype.json if available, else assume 1
        let page_count = load_formtype_resolved(&request.form_id, request.formtypes_dir.as_deref())
            .map(|ft| ft.page_count())
            .unwrap_or(1);

        let preview_png_paths =
            export_preview_pngs(&typ_path, &request.output_dir, page_count).unwrap_or_default();

        return Ok(PrintResult {
            pdf_path,
            preview_png_paths,
            typ_path,
        });
    }

    // ── Path 2: Legacy SVG-background path (existing behavior) ──
    let formtype = load_formtype_resolved(&request.form_id, request.formtypes_dir.as_deref())?;
    validate_fields(&formtype, &request.fields)?;
    write_static_assets_resolved(
        &request.form_id,
        &request.output_dir,
        request.formtypes_dir.as_deref(),
    )?;

    let template = load_template_resolved(&request.form_id, request.formtypes_dir.as_deref())?;
    let typ_path = request.output_dir.join("generated.typ");
    let pdf_path = request.output_dir.join("generated.pdf");
    let typst = generate_typst(&formtype, &request.fields, &template)?;
    fs::write(&typ_path, typst)?;

    let embedded = EmbeddedTypstCompiler;
    let cli = CliTypstCompiler;
    if let Err(_embedded_err) = embedded.compile_pdf(&typ_path, &pdf_path, &request.output_dir) {
        cli.compile_pdf(&typ_path, &pdf_path, &request.output_dir)?;
    }

    let preview_png_paths =
        export_preview_pngs(&typ_path, &request.output_dir, formtype.page_count())
            .unwrap_or_default();

    Ok(PrintResult {
        pdf_path,
        preview_png_paths,
        typ_path,
    })
}

/// Backward-compatible alias for [`render_flat_pdf`].
pub fn render_print(request: PrintRequest) -> Result<PrintResult, PrintError> {
    render_flat_pdf(request)
}

/// Render an editable (AcroForm fillable) PDF for **any** form.
///
/// First renders the flat PDF via Typst, then injects real AcroForm widget
/// annotations so the resulting file can be opened and edited in macOS Preview
/// or Adobe Acrobat as a real form.
pub fn render_editable_pdf(request: PrintRequest) -> Result<PrintResult, PrintError> {
    let flat = render_flat_pdf(request.clone())?;
    let formtype = load_formtype_resolved(&request.form_id, request.formtypes_dir.as_deref())?;
    let editable_path = request.output_dir.join("editable.pdf");
    editable::inject_acroform(&flat.pdf_path, &formtype, &request.fields, &editable_path)?;
    Ok(PrintResult {
        pdf_path: editable_path,
        preview_png_paths: flat.preview_png_paths,
        typ_path: flat.typ_path,
    })
}

/// Render an editable (AcroForm fillable) PDF for Form 2551Q.
///
/// First renders the flat PDF via Typst, then injects real AcroForm widget
/// annotations so the resulting file can be opened and edited in macOS Preview
/// or Adobe Acrobat as a real form.
pub fn render_2551q_editable(
    draft: &Form2551QDraft,
    output_dir: impl Into<PathBuf>,
) -> Result<PrintResult, PrintError> {
    let output_dir = output_dir.into();
    let flat = render_2551q_flat(draft, &output_dir, None)?;
    let formtype = load_formtype(FORM_2551Q_ID)?;
    let editable_path = output_dir.join("editable.pdf");
    editable::inject_acroform(
        &flat.pdf_path,
        &formtype,
        &draft.to_bir_field_map(),
        &editable_path,
    )?;
    Ok(PrintResult {
        pdf_path: editable_path,
        preview_png_paths: flat.preview_png_paths,
        typ_path: flat.typ_path,
    })
}

pub fn write_2551q_pdf(
    draft: &Form2551QDraft,
    _paper_size: PaperSize,
    output_path: impl AsRef<Path>,
) -> std::io::Result<PathBuf> {
    let output_path = output_path.as_ref().to_path_buf();
    let output_dir = output_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(bir_core::platform::temp_dir);
    let result = render_2551q_print(draft, &output_dir, None)
        .map_err(|err| std::io::Error::other(err.to_string()))?;

    if result.pdf_path != output_path {
        fs::copy(&result.pdf_path, &output_path)?;
    }

    Ok(output_path)
}

pub fn render_2551q_pdf(draft: &Form2551QDraft, _paper_size: PaperSize) -> Vec<u8> {
    match tempfile::tempdir()
        .ok()
        .and_then(|dir| render_2551q_print(draft, dir.path(), None).ok())
        .and_then(|result| fs::read(result.pdf_path).ok())
    {
        Some(pdf) => pdf,
        None => render_2551q_fallback_pdf(draft),
    }
}

// FormType, FormField, FieldKind, WidgetSpec, WidgetType
// are defined in formtype.rs and re-imported above.

/// Load and validate the [`FormType`] for the given form ID.
///
/// Tries filesystem first (if `formtypes_dir` is provided), then
/// falls back to the embedded 2551Q layout.
fn load_formtype_resolved(
    form_id: &str,
    formtypes_dir: Option<&Path>,
) -> Result<FormType, PrintError> {
    // Try filesystem first
    if let Some(base) = formtypes_dir {
        let json_path = base.join(form_id).join("formtype.json");
        if json_path.exists() {
            let content = fs::read_to_string(&json_path)?;
            let ft: FormType = serde_json::from_str(&content)?;
            if ft.form_id != form_id {
                return Err(PrintError::InvalidLayout(format!(
                    "formtype form_id {} does not match {form_id}",
                    ft.form_id
                )));
            }
            return Ok(ft);
        }
    }

    // Fallback: embedded constants (only 2551Q)
    match form_id {
        FORM_2551Q_ID => {
            let ft: FormType = serde_json::from_str(LAYOUT_2551Q)?;
            Ok(ft)
        }
        other => Err(PrintError::UnsupportedForm(other.to_string())),
    }
}

/// Public convenience for callers that don't have a `formtypes_dir`.
/// Keeps backward compatibility with existing tests and 2551Q-specific code.
pub fn load_formtype(form_id: &str) -> Result<FormType, PrintError> {
    load_formtype_resolved(form_id, None)
}

fn validate_fields(
    formtype: &FormType,
    fields: &BTreeMap<String, String>,
) -> Result<(), PrintError> {
    let missing = formtype
        .fields
        .iter()
        .filter(|field| !field.optional)
        .filter(|field| !fields.contains_key(&field.key))
        .map(|field| field.key.clone())
        .collect::<Vec<_>>();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(PrintError::MissingFields(missing))
    }
}

/// Write SVG page backgrounds to the output directory.
///
/// Tries the filesystem `{formtypes_dir}/{form_id}/pages/*.svg` first,
/// then falls back to the embedded 2551Q SVGs.
fn write_static_assets_resolved(
    form_id: &str,
    output_dir: &Path,
    formtypes_dir: Option<&Path>,
) -> Result<(), PrintError> {
    let svg_dir = output_dir.join("pages");
    fs::create_dir_all(&svg_dir)?;

    // Try filesystem first — iterate all SVG files in the pages/ directory
    if let Some(base) = formtypes_dir {
        let pages_dir = base.join(form_id).join("pages");
        if pages_dir.is_dir() {
            let mut found_any = false;
            let mut entries: Vec<_> = fs::read_dir(&pages_dir)?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "svg"))
                .collect();
            entries.sort_by_key(|e| e.file_name());

            for entry in entries {
                let file_name = entry.file_name();
                fs::copy(entry.path(), svg_dir.join(&file_name))?;
                found_any = true;
            }
            if found_any {
                return Ok(());
            }
        }
    }

    // Fallback: embedded constants (only 2551Q)
    match form_id {
        FORM_2551Q_ID => {
            fs::write(svg_dir.join("page1.svg"), PAGE1_SVG_2551Q)?;
            fs::write(svg_dir.join("page2.svg"), PAGE2_SVG_2551Q)?;
            Ok(())
        }
        other => Err(PrintError::UnsupportedForm(other.to_string())),
    }
}

/// Load the Typst template for the given form.
///
/// Tries `{formtypes_dir}/{form_id}/template.typ` first, then falls
/// back to the embedded 2551Q template.
fn load_template_resolved(
    form_id: &str,
    formtypes_dir: Option<&Path>,
) -> Result<String, PrintError> {
    if let Some(base) = formtypes_dir {
        let typ_path = base.join(form_id).join("template.typ");
        if typ_path.exists() {
            return Ok(fs::read_to_string(&typ_path)?);
        }
    }
    // Fallback: embedded 2551Q template
    match form_id {
        FORM_2551Q_ID => Ok(TEMPLATE_2551Q.to_string()),
        other => Err(PrintError::UnsupportedForm(other.to_string())),
    }
}

/// Try to load a Typst-native form template (`form.typ`).
///
/// Returns `Some(content)` if `{formtypes_dir}/{form_id}/form.typ` exists,
/// `None` otherwise — which triggers the legacy SVG-background path.
fn load_form_typ(form_id: &str, formtypes_dir: Option<&Path>) -> Option<String> {
    let base = formtypes_dir?;
    let form_path = base.join(form_id).join("form.typ");
    if form_path.exists() {
        fs::read_to_string(&form_path).ok()
    } else {
        None
    }
}

fn generate_typst(
    formtype: &FormType,
    fields: &BTreeMap<String, String>,
    template: &str,
) -> Result<String, PrintError> {
    let mut lines = vec![
        format!(
            "#set page(width: {}pt, height: {}pt, margin: 0pt)",
            fmt_num(formtype.page_width),
            fmt_num(formtype.page_height)
        ),
        template.to_string(),
    ];

    for page in 1..=formtype.page_count() {
        lines.push(format!(
            "#page(background: image(\"pages/page{page}.svg\", width: {}pt, height: {}pt), foreground: {{",
            fmt_num(formtype.page_width),
            fmt_num(formtype.page_height)
        ));

        // Track remaining text per key for multi-box spanning.
        // The first time we see a key, we load its full value.
        // Subsequent boxes for the same key consume from the remainder.
        let mut remaining: BTreeMap<String, String> = BTreeMap::new();
        // Track which box index we're on per key (for Decimal: 0=integer, 1=cents).
        let mut box_index: BTreeMap<String, usize> = BTreeMap::new();

        for field in formtype.fields.iter().filter(|field| field.page == page) {
            let idx = *box_index.get(&field.key).unwrap_or(&0);

            // Initialize remaining text for this key on first encounter
            if !remaining.contains_key(&field.key) {
                let full_value = fields.get(&field.key).cloned().unwrap_or_default();
                if field.kind == FieldKind::Dec {
                    // For Decimal, normalize and split at '.'
                    let normalized = normalize_decimal_value(&full_value);
                    let parts: Vec<&str> = normalized.splitn(2, '.').collect();
                    let int_part = parts.first().copied().unwrap_or("0").to_string();
                    let dec_part = parts.get(1).copied().unwrap_or("00").to_string();
                    // Store as "integer\x00decimal" so we can split later
                    remaining.insert(field.key.clone(), format!("{}\x00{}", int_part, dec_part));
                } else {
                    remaining.insert(field.key.clone(), full_value);
                }
            }

            let current_text = remaining.get(&field.key).cloned().unwrap_or_default();

            if let Some(line) = render_field_spanning(field, &current_text, idx)? {
                lines.push(format!("  {line}"));
            }

            // Consume characters from remaining text based on capacity
            let consumed = consumed_chars(field, &current_text, idx);
            if consumed > 0 {
                let rest = if consumed >= current_text.len() {
                    String::new()
                } else {
                    current_text[consumed..].to_string()
                };
                remaining.insert(field.key.clone(), rest);
            }

            // Advance box index
            box_index.insert(field.key.clone(), idx + 1);
        }

        lines.push("})[]".to_string());
    }

    Ok(lines.join("\n"))
}

/// Normalize a decimal value for rendering.
/// Ensures format "integer.cents" with exactly 2 decimal places.
fn normalize_decimal_value(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "".to_string();
    }
    let cleaned = trimmed.replace(',', "");
    match cleaned.parse::<f64>() {
        Ok(number) => format!("{number:.2}"),
        Err(_) => {
            if cleaned.contains('.') {
                cleaned
            } else {
                format!("{}.00", cleaned)
            }
        }
    }
}

fn byte_index_after_chars(value: &str, character_count: usize) -> usize {
    value
        .char_indices()
        .nth(character_count)
        .map_or(value.len(), |(index, _)| index)
}

fn text_field_consumed_bytes(value: &str, capacity: usize) -> usize {
    if capacity >= value.chars().count() {
        return value.len();
    }

    let boundary = byte_index_after_chars(value, capacity);
    let slice = &value[..boundary];
    match slice.rfind(' ') {
        Some(position) if position > 0 => position + 1,
        _ => boundary,
    }
}

/// Calculate how many characters a field consumes from the input string.
fn consumed_chars(field: &FormField, value: &str, box_idx: usize) -> usize {
    if value.is_empty() {
        return 0;
    }
    match field.kind {
        FieldKind::Bool => 0,
        FieldKind::Dec => {
            if box_idx == 0 {
                // Integer box: consume everything up to and including the \x00 separator
                if let Some(sep) = value.find('\x00') {
                    sep + 1
                } else {
                    value.len()
                }
            } else {
                // Cents box: consume everything remaining
                value.len()
            }
        }
        FieldKind::Int => {
            let capacity = field.char_capacity().unwrap_or(value.len());
            byte_index_after_chars(value, capacity)
        }
        FieldKind::Char => {
            let capacity = field.char_capacity().unwrap_or(value.len());
            if field.cell_w.is_some() {
                byte_index_after_chars(value, capacity)
            } else {
                text_field_consumed_bytes(value, capacity)
            }
        }
    }
}

/// Render a single field box, receiving only its portion of text (after spanning).
fn render_field_spanning(
    field: &FormField,
    value: &str,
    box_idx: usize,
) -> Result<Option<String>, PrintError> {
    match field.kind {
        FieldKind::Bool => {
            if boolish(value) {
                Ok(Some(format!(
                    "mark({}, {}, {}, {})",
                    fmt_num(field.x),
                    fmt_num(field.y),
                    fmt_num(field.display_width()),
                    fmt_num(field.display_height())
                )))
            } else {
                Ok(None)
            }
        }
        FieldKind::Char => {
            let capacity = field.char_capacity().unwrap_or(usize::MAX);

            if field.cell_w.is_some() {
                // Render as cells
                let chunk: String = value.chars().take(capacity).collect();
                if chunk.is_empty() {
                    Ok(None)
                } else {
                    let macro_name = match field.direction {
                        Direction::Rtl => "cells_rtl",
                        Direction::Ltr => "cells",
                    };
                    Ok(Some(format!(
                        r#"{}({}, {}, {}, {}, {}, "{}")"#,
                        macro_name,
                        fmt_num(field.x),
                        fmt_num(field.y),
                        fmt_num(field.display_width()),
                        fmt_num(field.display_height()),
                        field.char_capacity().unwrap_or(chunk.len()),
                        typst_string(&chunk.to_uppercase())
                    )))
                }
            } else {
                // Render as text
                let chunk = if capacity >= value.chars().count() {
                    value.to_string()
                } else {
                    let boundary = byte_index_after_chars(value, capacity);
                    let slice = &value[..boundary];
                    match slice.rfind(' ') {
                        Some(pos) if pos > 0 => slice[..pos].to_string(),
                        _ => slice.to_string(),
                    }
                };
                if chunk.is_empty() {
                    Ok(None)
                } else if let Some(cc) = field.char_count.filter(|&c| c > 0) {
                    // Explicit char_count → render as cells (1 char per box)
                    let macro_name = match field.direction {
                        Direction::Rtl => "cells_rtl",
                        Direction::Ltr => "cells",
                    };
                    Ok(Some(format!(
                        r#"{}({}, {}, {}, {}, {}, "{}")"#,
                        macro_name,
                        fmt_num(field.x),
                        fmt_num(field.y),
                        fmt_num(field.display_width()),
                        fmt_num(field.display_height()),
                        cc,
                        typst_string(&chunk.to_uppercase())
                    )))
                } else {
                    // No char_count → plain text label
                    Ok(Some(format!(
                        r#"label({}, {}, {}, {}, {}, "{}")"#,
                        fmt_num(field.x),
                        fmt_num(field.y),
                        fmt_num(field.display_width()),
                        fmt_num(field.display_height()),
                        fmt_num(field.size.unwrap_or(8.5)),
                        typst_string(&chunk.to_uppercase())
                    )))
                }
            }
        }
        FieldKind::Dec => {
            if box_idx == 0 {
                // Integer box (RTL)
                let int_part = if let Some(sep) = value.find('\x00') {
                    &value[..sep]
                } else {
                    value
                };
                let display = if int_part.is_empty() || int_part == "0" {
                    "0".to_string()
                } else {
                    int_part.to_string()
                };
                let char_count = field.char_capacity().unwrap_or(display.len());
                Ok(Some(format!(
                    r#"cells_rtl({}, {}, {}, {}, {}, "{}")"#,
                    fmt_num(field.x),
                    fmt_num(field.y),
                    fmt_num(field.display_width()),
                    fmt_num(field.display_height()),
                    char_count,
                    typst_string(&display)
                )))
            } else {
                // Cents box (LTR, auto char_count = 2)
                let dec_part = if value.is_empty() { "00" } else { value };
                let display = format!("{:0<2}", &dec_part[..dec_part.len().min(2)]);
                let char_count = field.char_count.unwrap_or(2);
                Ok(Some(format!(
                    r#"cells({}, {}, {}, {}, {}, "{}")"#,
                    fmt_num(field.x),
                    fmt_num(field.y),
                    fmt_num(field.display_width()),
                    fmt_num(field.display_height()),
                    char_count,
                    typst_string(&display)
                )))
            }
        }
        FieldKind::Int => {
            let char_count = field.char_capacity().unwrap_or(1);
            let cleaned = value.trim().replace(',', "");
            let num: i64 = cleaned.parse().unwrap_or(0);
            let display = format!("{:0>width$}", num, width = char_count);
            Ok(Some(format!(
                r#"cells_rtl({}, {}, {}, {}, {}, "{}")"#,
                fmt_num(field.x),
                fmt_num(field.y),
                fmt_num(field.display_width()),
                fmt_num(field.display_height()),
                char_count,
                typst_string(&display)
            )))
        }
    }
}

fn export_preview_pngs(
    typ_path: &Path,
    output_dir: &Path,
    page_count: usize,
) -> Result<Vec<PathBuf>, PrintError> {
    let pattern = output_dir.join("preview-{p}.png");
    let output = build_typst_command()
        .arg("compile")
        .arg("--root")
        .arg(output_dir)
        .arg("--format")
        .arg("png")
        .arg("--ppi")
        .arg("144")
        .arg(typ_path)
        .arg(&pattern)
        .output()
        .map_err(|err| PrintError::Preview(format!("failed to run typst: {err}")))?;

    if !output.status.success() {
        return Err(PrintError::Preview(command_output(&output)));
    }

    let mut paths = Vec::new();
    for page in 1..=page_count {
        let path = output_dir.join(format!("preview-{page}.png"));
        if path.exists() {
            paths.push(path);
        }
    }

    if paths.is_empty() {
        return Err(PrintError::Preview(
            "typst did not produce preview PNG files".to_string(),
        ));
    }

    Ok(paths)
}

fn command_output(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    format!("stdout:\n{stdout}\nstderr:\n{stderr}")
}

fn fmt_num(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

fn typst_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn boolish(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "y" | "x"
    )
}

fn render_2551q_fallback_pdf(draft: &Form2551QDraft) -> Vec<u8> {
    let mut rows = vec![
        "BIR Form No. 2551Q".to_string(),
        "Quarterly Percentage Tax Return".to_string(),
        format!("TIN: {}", draft.tin),
        format!("Taxpayer: {}", draft.taxpayer_name),
        format!("RDO: {}", draft.rdo_code),
        format!("Address: {}", draft.registered_address),
        format!(
            "ZIP: {}    Contact: {}",
            draft.zip_code, draft.contact_number
        ),
        format!(
            "Taxable Year: {}    Quarter: Q{}",
            draft.taxable_year, draft.quarter
        ),
        format!(
            "Amended: {}    Tax Relief: {}",
            yes_no(draft.is_amended),
            yes_no(draft.tax_relief)
        ),
        "".to_string(),
        "Schedule 1 - Computation of Tax".to_string(),
    ];

    for row in &draft.schedule_1 {
        rows.push(format!(
            "{}  {}  Taxable: {:.2}  Rate: {:.2}%  Due: {:.2}",
            row.atc,
            row.atc_description,
            row.taxable_amount,
            row.tax_rate * 100.0,
            row.tax_due
        ));
    }

    rows.extend([
        "".to_string(),
        "Part II - Computation of Tax".to_string(),
        format!("Total Tax Due: {:.2}", draft.total_tax_due),
        format!(
            "Creditable Percentage Tax Withheld: {:.2}",
            draft.creditable_tax_withheld
        ),
        format!(
            "Tax Paid in Previously Filed Return: {:.2}",
            draft.tax_paid_previous
        ),
        format!("Other Tax Credit/Payment: {:.2}", draft.other_tax_credit),
        format!("Total Tax Credits/Payments: {:.2}", draft.total_tax_credits),
        format!("Tax Still Payable/(Overpayment): {:.2}", draft.tax_payable),
        format!("Surcharge: {:.2}", draft.surcharge),
        format!("Interest: {:.2}", draft.interest),
        format!("Compromise: {:.2}", draft.compromise),
        format!("Total Penalties: {:.2}", draft.total_penalties),
        format!(
            "Total Amount Payable/(Overpayment): {:.2}",
            draft.total_amount_payable
        ),
        format!("Status: {:?}", draft.status),
    ]);

    let (width, height) = PaperSize::A4.points();
    build_simple_pdf(width, height, &rows)
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "Yes"
    } else {
        "No"
    }
}

fn build_simple_pdf(width: u32, height: u32, lines: &[String]) -> Vec<u8> {
    let lines_per_page = 42usize;
    let pages: Vec<&[String]> = lines.chunks(lines_per_page).collect();
    let mut objects = Vec::<String>::new();
    let page_count = pages.len().max(1);
    let pages_id = 2;
    let font_id = 3;
    let first_page_id = 4;
    let first_content_id = first_page_id + page_count;

    objects.push("<< /Type /Catalog /Pages 2 0 R >>".to_string());

    let kids = (0..page_count)
        .map(|i| format!("{} 0 R", first_page_id + i))
        .collect::<Vec<_>>()
        .join(" ");
    objects.push(format!(
        "<< /Type /Pages /Kids [{}] /Count {} >>",
        kids, page_count
    ));
    objects.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string());

    for i in 0..page_count {
        objects.push(format!(
            "<< /Type /Page /Parent {} 0 R /MediaBox [0 0 {} {}] /Resources << /Font << /F1 {} 0 R >> >> /Contents {} 0 R >>",
            pages_id,
            width,
            height,
            font_id,
            first_content_id + i
        ));
    }

    for (i, page_lines) in pages.iter().enumerate() {
        let stream = page_stream(height, page_lines, i + 1, page_count);
        objects.push(format!(
            "<< /Length {} >>\nstream\n{}\nendstream",
            stream.len(),
            stream
        ));
    }

    let mut pdf = String::from("%PDF-1.4\n");
    let mut offsets = Vec::new();
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.push_str(&format!("{} 0 obj\n{}\nendobj\n", i + 1, obj));
    }
    let xref_offset = pdf.len();
    pdf.push_str(&format!(
        "xref\n0 {}\n0000000000 65535 f \n",
        objects.len() + 1
    ));
    for offset in offsets {
        pdf.push_str(&format!("{:010} 00000 n \n", offset));
    }
    pdf.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
        objects.len() + 1,
        xref_offset
    ));
    pdf.into_bytes()
}

fn page_stream(height: u32, lines: &[String], page: usize, page_count: usize) -> String {
    let mut out = String::from("BT\n/F1 11 Tf\n14 TL\n");
    out.push_str(&format!("50 {} Td\n", height.saturating_sub(60)));
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            out.push_str("T*\n");
        }
        out.push_str(&format!("({}) Tj\n", escape_pdf_text(line)));
    }
    out.push_str("T*\n");
    out.push_str(&format!("(Page {} of {}) Tj\n", page, page_count));
    out.push_str("ET");
    out
}

fn escape_pdf_text(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '(' => "\\(".to_string(),
            ')' => "\\)".to_string(),
            '\\' => "\\\\".to_string(),
            c if c.is_ascii() => c.to_string(),
            _ => " ".to_string(),
        })
        .collect()
}

/// Build a simple PDF from a list of text lines (used for confirmation receipts).
pub fn build_simple_confirmation_pdf(lines: &[String]) -> Vec<u8> {
    let wrapped_lines = wrap_lines(lines, 90);
    let (width, height) = PaperSize::A4.points();
    build_simple_pdf(width, height, &wrapped_lines)
}

/// Append text pages to an existing PDF document.
///
/// Reads the PDF from `pdf_bytes`, adds new pages containing `lines` of text,
/// and returns the combined PDF bytes. This avoids the fragility of merging two
/// separate PDF documents by building new page objects directly.
pub fn append_text_pages_to_pdf(pdf_bytes: &[u8], lines: &[String]) -> Result<Vec<u8>, PrintError> {
    let wrapped_lines = wrap_lines(lines, 90);
    use lopdf::{dictionary, Document, Object, Stream};

    let mut doc = Document::load_mem(pdf_bytes).map_err(|e| {
        PrintError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("failed to load PDF: {e}"),
        ))
    })?;

    if lines.is_empty() {
        let mut buf = Vec::new();
        doc.save_to(&mut buf)
            .map_err(|e| PrintError::Io(std::io::Error::other(format!("save PDF: {e}"))))?;
        return Ok(buf);
    }

    // Read page dimensions from the first existing page's MediaBox so the
    // appended confirmation pages match the form's page size exactly.
    let (width, height) = {
        let pages = doc.get_pages();
        let mut first_page_id = None;
        if let Some(min_key) = pages.keys().min() {
            first_page_id = pages.get(min_key).copied();
        }
        if let Some(page_id) = first_page_id {
            if let Ok(lopdf::Object::Dictionary(ref dict)) = doc.get_object(page_id) {
                if let Ok(lopdf::Object::Array(ref mb)) = dict.get(b"MediaBox") {
                    if mb.len() == 4 {
                        let w = match &mb[2] {
                            lopdf::Object::Integer(v) => *v as u32,
                            lopdf::Object::Real(v) => *v as u32,
                            _ => 612,
                        };
                        let h = match &mb[3] {
                            lopdf::Object::Integer(v) => *v as u32,
                            lopdf::Object::Real(v) => *v as u32,
                            _ => 936,
                        };
                        (w, h)
                    } else {
                        (612, 936)
                    }
                } else {
                    (612, 936)
                }
            } else {
                (612, 936)
            }
        } else {
            (612, 936)
        }
    };
    let lines_per_page: usize = 55;

    // Find the Pages dictionary reference from the catalog
    let catalog = doc.catalog().map_err(|e| {
        PrintError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("no catalog: {e}"),
        ))
    })?;
    let pages_ref = catalog
        .get(b"Pages")
        .and_then(|o| o.as_reference())
        .map_err(|e| {
            PrintError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("no Pages ref: {e}"),
            ))
        })?;

    // Create a font dictionary object for the new pages
    let font_obj = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    };
    let font_id = doc.add_object(font_obj);

    let font_bold_obj = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica-Bold",
    };
    let font_bold_id = doc.add_object(font_bold_obj);

    // Split lines into page-sized chunks
    let page_chunks: Vec<&[String]> = wrapped_lines.chunks(lines_per_page).collect();
    let total_new_pages = page_chunks.len();

    let mut new_page_ids = Vec::new();

    for (i, chunk) in page_chunks.iter().enumerate() {
        // Build content stream
        let mut stream_content = String::from("BT\n");
        stream_content.push_str("/F1 10 Tf\n12 TL\n");
        stream_content.push_str(&format!("40 {} Td\n", height.saturating_sub(50)));

        for (j, line) in chunk.iter().enumerate() {
            if j > 0 {
                stream_content.push_str("T*\n");
            }
            stream_content.push_str(&format!("({}) Tj\n", escape_pdf_text(line)));
        }

        // Page footer
        stream_content.push_str("T*\n");
        stream_content.push_str(&format!(
            "(Confirmation - Page {} of {}) Tj\n",
            i + 1,
            total_new_pages
        ));
        stream_content.push_str("ET");

        let content_stream = Stream::new(dictionary! {}, stream_content.into_bytes());
        let content_id = doc.add_object(content_stream);

        let page_dict = dictionary! {
            "Type" => "Page",
            "Parent" => Object::Reference(pages_ref),
            "MediaBox" => vec![
                Object::Integer(0),
                Object::Integer(0),
                Object::Integer(width as i64),
                Object::Integer(height as i64),
            ],
            "Resources" => dictionary! {
                "Font" => dictionary! {
                    "F1" => Object::Reference(font_id),
                    "F2" => Object::Reference(font_bold_id),
                },
            },
            "Contents" => Object::Reference(content_id),
        };
        let page_id = doc.add_object(page_dict);
        new_page_ids.push(page_id);
    }

    // Update the Pages dictionary: append our new pages to Kids and bump Count
    if let Ok(Object::Dictionary(ref mut dict)) = doc.get_object_mut(pages_ref) {
        // Get existing Kids array
        if let Ok(Object::Array(ref mut kids)) = dict.get_mut(b"Kids") {
            for pid in &new_page_ids {
                kids.push(Object::Reference(*pid));
            }
        }
        // Update Count
        if let Ok(Object::Integer(ref mut count)) = dict.get_mut(b"Count") {
            *count += new_page_ids.len() as i64;
        }
    }

    let mut buf = Vec::new();
    doc.save_to(&mut buf).map_err(|e| {
        PrintError::Io(std::io::Error::other(format!(
            "save PDF with appended pages: {e}"
        )))
    })?;
    Ok(buf)
}

fn wrap_lines(lines: &[String], max_chars: usize) -> Vec<String> {
    let mut wrapped = Vec::new();
    for line in lines {
        if line.is_empty() {
            wrapped.push(String::new());
            continue;
        }

        let mut current_line = String::new();
        for word in line.split_whitespace() {
            let space = if current_line.is_empty() { "" } else { " " };

            if current_line.len() + space.len() + word.len() <= max_chars {
                current_line.push_str(space);
                current_line.push_str(word);
            } else {
                if !current_line.is_empty() {
                    wrapped.push(current_line);
                    current_line = String::new();
                }

                if word.len() > max_chars {
                    let mut chars = word.chars().peekable();
                    while chars.peek().is_some() {
                        let chunk: String = chars.by_ref().take(max_chars).collect();
                        if chars.peek().is_some() {
                            wrapped.push(chunk);
                        } else {
                            current_line = chunk;
                        }
                    }
                } else {
                    current_line = word.to_string();
                }
            }
        }
        if !current_line.is_empty() {
            wrapped.push(current_line);
        }
    }
    wrapped
}

#[cfg(test)]
mod tests {
    use super::*;
    use bir_core::forms::form_2551q::Item13Election;
    use bir_core::naming::Tin;
    use bir_core::profile::{TaxpayerProfile, TaxpayerType};

    fn sample_draft() -> Form2551QDraft {
        let profile = TaxpayerProfile {
            id: None,
            full_name: "Goldcoders Corp".into(),
            tin: Tin {
                segment1: "779".into(),
                segment2: "025".into(),
                segment3: "068".into(),
                branch: "000".into(),
            },
            rdo_code: "018".into(),
            line_of_business: "Software".into(),
            registered_address: "Olongapo City".into(),
            zip_code: "2200".into(),
            phone: "09156837000".into(),
            email: "tax@example.com".into(),
            default_form_type: "2551Qv2018".into(),
            taxpayer_type: TaxpayerType::Corporation,
            is_vat_registered: false,
            business_start_date: None,
            birth_date: None,
            email_tracking_enabled: false,
            email_auth_method: Default::default(),
            imap_email: None,
            imap_host: None,

            test_notification_enabled: false,
            imap_app_password: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
            profile_versions: vec![],
            compliance_source_mode: Default::default(),
            tax_classification: None,
            eopt_tier: None,
            is_bmbe: false,
            is_gpp_partner: false,
            is_create_msme: false,
            is_expanded_withholding_agent: false,
            atc_codes: vec![],
            excise_tax_categories: vec![],
            tax_elections: vec![],
            has_employees: false,
            is_dormant: false,
            has_single_employer: false,
            withholds_compensation: false,
            withholds_expanded: false,
            withholds_final: false,
            is_top_withholding_agent: false,
            is_government_withholding_entity: false,
            registration_activity_status: Default::default(),
            is_archived: false,
            profile_pin_hash: None,
            totp_secret: None,
            per_year_forms: Default::default(),
        };
        let mut draft = Form2551QDraft::new_from_profile(&profile, 2026, 1);
        draft.item_13_election = Item13Election::NotApplicable;
        draft.schedule_1[0].taxable_amount = 10_000.0;
        draft.creditable_tax_withheld = 25.0;
        draft.recompute(None);
        draft
    }

    #[test]
    fn loads_2551q_formtype() {
        let layout = load_formtype(FORM_2551Q_ID).expect("formtype should load");
        assert_eq!(layout.page_width, 612.0);
        assert_eq!(layout.page_height, 936.0);
        assert_eq!(layout.page_count(), 2);
        assert!(layout
            .fields
            .iter()
            .any(|field| field.key == "txtTotalSched1"));
    }

    #[test]
    fn loads_generated_1702_formtype_shells_from_filesystem() {
        let formtypes_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("formtypes");

        let rt = load_formtype_resolved("1702RTv2018C", Some(&formtypes_dir))
            .expect("1702RT draft formtype should load from formtypes/");
        assert_eq!(rt.page_width, 612.0);
        assert_eq!(rt.page_height, 936.0);
        assert_eq!(rt.page_count(), 4);
        assert_eq!(rt.fields.len(), 427);
        assert!(
            formtypes_dir
                .join("1702RTv2018C")
                .join("template.typ")
                .exists(),
            "1702RT should have a template file"
        );

        let mx = load_formtype_resolved("1702MXv2018C", Some(&formtypes_dir))
            .expect("1702MX draft formtype should load from formtypes/");
        assert_eq!(mx.page_width, 612.0);
        assert_eq!(mx.page_height, 936.0);
        assert_eq!(mx.page_count(), 4);
        assert_eq!(mx.fields.len(), 779);
        assert!(
            formtypes_dir
                .join("1702MXv2018C")
                .join("template.typ")
                .exists(),
            "1702MX should have a template file"
        );
    }

    #[test]
    fn loads_priority_formtype_shells_from_filesystem() {
        let formtypes_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("formtypes");

        for (form_id, page_width, page_height, page_count, field_count) in [
            ("1601Cv2018", 612.0, 936.0, 2, 123),
            ("1701v2018", 612.0, 936.0, 4, 955),
            ("1701Qv2018", 612.0, 936.0, 2, 504),
            ("2550Qv2024", 612.0, 1008.0, 2, 404),
            ("1702RTv2018C", 612.0, 936.0, 4, 427),
            ("1702MXv2018C", 612.0, 936.0, 4, 779),
        ] {
            let layout = load_formtype_resolved(form_id, Some(&formtypes_dir))
                .unwrap_or_else(|err| panic!("{form_id} formtype should load: {err}"));
            assert_eq!(layout.page_width, page_width, "{form_id} page width");
            assert_eq!(layout.page_height, page_height, "{form_id} page height");
            assert_eq!(layout.page_count(), page_count, "{form_id} page count");
            assert_eq!(layout.fields.len(), field_count, "{form_id} field count");
        }
    }

    #[test]
    fn sample_data_covers_required_layout_fields() {
        let layout = load_formtype(FORM_2551Q_ID).expect("formtype should load");
        let fields = sample_draft().to_bir_field_map();
        validate_fields(&layout, &fields).expect("sample fields should cover layout");
    }

    #[test]
    fn generated_typst_uses_official_page_size() {
        let layout = load_formtype(FORM_2551Q_ID).expect("formtype should load");
        let fields = sample_draft().to_bir_field_map();
        let typst = generate_typst(&layout, &fields, TEMPLATE_2551Q).expect("typst should render");

        assert!(typst.contains("#set page(width: 612pt, height: 936pt, margin: 0pt)"));
        assert!(typst.contains("pages/page1.svg"));
        assert!(typst.contains("pages/page2.svg"));
    }

    #[test]
    fn legacy_2551q_preview_fails_closed_before_truncating_text() {
        let mut draft = sample_draft();
        draft.taxpayer_name = "N".repeat(27);

        assert_eq!(
            legacy_2551q_print_issue(&draft),
            Some(Legacy2551QPrintIssue::FieldOverflow {
                field: "taxpayer_name",
                label: "the taxpayer name",
                character_count: 27,
                capacity: 26,
            })
        );

        let output = tempfile::tempdir().expect("temp dir");
        let error = render_2551q_print(&draft, output.path(), None)
            .expect_err("legacy renderer must not truncate a repeated taxpayer name");
        assert!(matches!(error, PrintError::UnsafeLegacy2551Q(_)));
    }

    #[test]
    fn legacy_2551q_preview_rejects_fields_missing_from_the_snapshot_overlay() {
        let mut tax_relief = sample_draft();
        tax_relief.tax_relief = true;
        tax_relief.tax_relief_specification = "Special Law 123".to_string();
        assert!(matches!(
            legacy_2551q_print_issue(&tax_relief),
            Some(Legacy2551QPrintIssue::UnsupportedNonEmptyField {
                field: "tax_relief_specification",
                ..
            })
        ));

        let mut other_credit = sample_draft();
        other_credit.other_tax_credit_description = "Validated prior payment".to_string();
        assert!(matches!(
            legacy_2551q_print_issue(&other_credit),
            Some(Legacy2551QPrintIssue::UnsupportedNonEmptyField {
                field: "other_tax_credit_description",
                ..
            })
        ));
    }

    #[test]
    fn legacy_text_spanning_uses_unicode_character_boundaries() {
        let layout = load_formtype(FORM_2551Q_ID).expect("formtype should load");
        let field = layout
            .fields
            .iter()
            .find(|field| field.key == "frm2551Qv2018:txtPg2TaxpayerName")
            .expect("page-two taxpayer name field");
        let value = "Ñ".repeat(26);

        let rendered = render_field_spanning(field, &value, 0)
            .expect("unicode field should render")
            .expect("unicode field should produce Typst");

        assert!(rendered.contains(&value));
        assert_eq!(consumed_chars(field, &value, 0), value.len());
    }

    #[test]
    fn cli_fallback_compiles_pdf_and_preview_pngs() {
        if Command::new("typst").arg("--version").output().is_err() {
            eprintln!("typst CLI not installed; skipping");
            return;
        }

        let temp_dir = tempfile::tempdir().expect("temp dir");
        let result =
            render_2551q_print(&sample_draft(), temp_dir.path(), None).expect("render print");

        let pdf = fs::read(&result.pdf_path).expect("pdf should exist");
        assert!(pdf.starts_with(b"%PDF-"));
        assert!(pdf.len() > 100_000);
        assert_eq!(result.preview_png_paths.len(), 2);
        assert!(result.preview_png_paths.iter().all(|path| path.exists()));
        assert!(result.typ_path.exists());
        assert_preview_content_starts_near_top(&result.preview_png_paths[0], 80);
        assert_preview_content_starts_near_top(&result.preview_png_paths[1], 100);
    }

    #[test]
    fn embedded_compiler_returns_controlled_fallback_error() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let typ_path = temp_dir.path().join("test.typ");
        let pdf_path = temp_dir.path().join("test.pdf");
        fs::write(&typ_path, "#set page(width: 612pt, height: 936pt)").expect("typ write");

        let err = EmbeddedTypstCompiler
            .compile_pdf(&typ_path, &pdf_path, temp_dir.path())
            .expect_err("embedded compiler is intentionally unavailable in v1");

        assert!(matches!(err, PrintError::EmbeddedUnavailable(_)));
    }

    #[test]
    fn renders_non_empty_pdf() {
        let pdf = render_2551q_pdf(&sample_draft(), PaperSize::A4);
        assert!(pdf.starts_with(b"%PDF-"));
        assert!(pdf.len() > 1000);
    }

    #[test]
    fn layout_keys_are_valid() {
        let layout = load_formtype(FORM_2551Q_ID).expect("formtype should load");
        // Keys may repeat for multi-box spanning, but duplicates must:
        // 1. Be on the same page
        // 2. Share the same FieldKind
        let mut seen: BTreeMap<String, (usize, formtype::FieldKind)> = BTreeMap::new();
        for field in &layout.fields {
            if let Some((_page, kind)) = seen.get(&field.key) {
                assert_eq!(
                    *kind, field.kind,
                    "duplicate key '{}' has mismatched kind",
                    field.key
                );
            }
            seen.insert(field.key.clone(), (field.page, field.kind));
        }
    }

    #[test]
    fn editable_pdf_contains_acroform() {
        let temp = tempfile::tempdir().unwrap();
        let result = render_2551q_editable(&sample_draft(), temp.path())
            .expect("editable render should succeed");
        assert!(result.pdf_path.exists(), "editable PDF should exist");
        let pdf_bytes = std::fs::read(&result.pdf_path).unwrap();
        let pdf_str = String::from_utf8_lossy(&pdf_bytes);
        assert!(
            pdf_str.contains("/AcroForm"),
            "editable PDF should contain /AcroForm"
        );
        assert!(
            pdf_str.contains("/Widget"),
            "editable PDF should contain /Widget annotations"
        );
    }

    #[test]
    fn editable_pdf_has_text_and_checkbox_fields() {
        let temp = tempfile::tempdir().unwrap();
        let result = render_2551q_editable(&sample_draft(), temp.path())
            .expect("editable render should succeed");
        let doc =
            lopdf::Document::load(&result.pdf_path).expect("editable PDF should load with lopdf");

        let mut has_text = false;
        let mut has_btn = false;
        for (_id, obj) in doc.objects.iter() {
            if let lopdf::Object::Dictionary(dict) = obj {
                if let Ok(lopdf::Object::Name(ft)) = dict.get(b"FT") {
                    if ft == b"Tx" {
                        has_text = true;
                    }
                    if ft == b"Btn" {
                        has_btn = true;
                    }
                }
            }
        }
        assert!(
            has_text,
            "editable PDF should contain text fields (/FT /Tx)"
        );
        assert!(
            has_btn,
            "editable PDF should contain checkbox fields (/FT /Btn)"
        );
    }

    #[test]
    fn editable_pdf_field_names_present() {
        let temp = tempfile::tempdir().unwrap();
        let result = render_2551q_editable(&sample_draft(), temp.path())
            .expect("editable render should succeed");
        let pdf_bytes = std::fs::read(&result.pdf_path).unwrap();
        let pdf_str = String::from_utf8_lossy(&pdf_bytes);
        // TIN field should be present as a widget
        assert!(
            pdf_str.contains("frm2551Qv2018:txtTIN1"),
            "TIN1 field name should appear in editable PDF"
        );
        // A checkbox field should be present
        assert!(
            pdf_str.contains("frm2551Qv2018:qtr_"),
            "quarter checkbox field name should appear in editable PDF"
        );
    }

    #[test]
    fn formtype_widget_specs_deserialize() {
        let formtype = load_formtype(FORM_2551Q_ID).expect("formtype should load");
        let text_widgets = formtype
            .fields
            .iter()
            .filter(|f| {
                f.widget
                    .as_ref()
                    .is_some_and(|w| w.widget_type == formtype::WidgetType::Text)
            })
            .count();
        let checkbox_widgets = formtype
            .fields
            .iter()
            .filter(|f| {
                f.widget
                    .as_ref()
                    .is_some_and(|w| w.widget_type == formtype::WidgetType::Checkbox)
            })
            .count();
        assert!(text_widgets > 0, "should have text widgets");
        assert!(checkbox_widgets > 0, "should have checkbox widgets");
    }

    fn assert_preview_content_starts_near_top(path: &Path, max_top: u32) {
        let image = image::ImageReader::open(path)
            .expect("preview png should open")
            .decode()
            .expect("preview png should decode")
            .to_rgba8();
        assert_eq!(image.width(), 1224);
        assert_eq!(image.height(), 1872);

        let top = image
            .enumerate_pixels()
            .filter(|(_, _, pixel)| {
                let [r, g, b, a] = pixel.0;
                a > 0 && (r < 245 || g < 245 || b < 245)
            })
            .map(|(_, y, _)| y)
            .min()
            .expect("preview should contain non-white content");

        assert!(
            top <= max_top,
            "content in {} starts at y={top}, expected <= {max_top}",
            path.display()
        );
    }
}
