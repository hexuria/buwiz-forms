pub mod editable;
pub mod formtype;

use bir_core::forms::form_2551q::Form2551QDraft;
use formtype::{FieldKind, FormField, FormType};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const FORM_2551Q_ID: &str = "2551Qv2018";
const LAYOUT_2551Q: &str = include_str!("../../../formtypes/2551Qv2018/formtype.json");
const TEMPLATE_2551Q: &str = include_str!("../../../formtypes/2551Qv2018/template.typ");
const PAGE1_SVG_2551Q: &str = include_str!("../../../formtypes/2551Qv2018/pages/page1.svg");
const PAGE2_SVG_2551Q: &str = include_str!("../../../formtypes/2551Qv2018/pages/page2.svg");

#[cfg(target_os = "macos")]
pub fn print_html_mac(_html_content: &str) {
    /*
    let mtm = unsafe { MainThreadMarker::new_unchecked() };

    unsafe {
        let _app = NSApplication::sharedApplication(mtm);

        let web_view = WKWebView::new_mtm(mtm);
        let ns_string = NSString::from_str(html_content);

        web_view.loadHTMLString_baseURL(&ns_string, None);

        let print_info = NSPrintInfo::sharedPrintInfo();
        let print_op = web_view.printOperationWithPrintInfo(&print_info);

        // This opens the native macOS print dialog!
        print_op.runOperation();
    }
    */
    println!("Print function temporarily disabled to fix compilation");
}

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
        }
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
        let _typst_link = std::any::type_name::<typst::diag::FileError>();
        Err(PrintError::EmbeddedUnavailable(
            "full embedded Typst world is not wired yet; falling back to typst CLI".to_string(),
        ))
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct CliTypstCompiler;

impl TypstCompiler for CliTypstCompiler {
    fn compile_pdf(
        &self,
        typ_path: &Path,
        pdf_path: &Path,
        root_dir: &Path,
    ) -> Result<(), PrintError> {
        let output = Command::new("typst")
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
) -> Result<PrintResult, PrintError> {
    render_flat_pdf(PrintRequest::new(
        FORM_2551Q_ID,
        draft.to_bir_field_map(),
        output_dir,
    ))
}

/// Backward-compatible alias for [`render_2551q_flat`].
pub fn render_2551q_print(
    draft: &Form2551QDraft,
    output_dir: impl Into<PathBuf>,
) -> Result<PrintResult, PrintError> {
    render_2551q_flat(draft, output_dir)
}

/// Render a flat (official-looking) PDF from a `PrintRequest`.
pub fn render_flat_pdf(request: PrintRequest) -> Result<PrintResult, PrintError> {
    let formtype = load_formtype(&request.form_id)?;
    validate_fields(&formtype, &request.fields)?;
    fs::create_dir_all(&request.output_dir)?;
    write_static_assets(&request.form_id, &request.output_dir)?;

    let typ_path = request.output_dir.join("generated.typ");
    let pdf_path = request.output_dir.join("generated.pdf");
    let typst = generate_typst(&formtype, &request.fields)?;
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
    let flat = render_2551q_flat(draft, &output_dir)?;
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
    let result = render_2551q_print(draft, &output_dir)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err.to_string()))?;

    if result.pdf_path != output_path {
        fs::copy(&result.pdf_path, &output_path)?;
    }

    Ok(output_path)
}

pub fn render_2551q_pdf(draft: &Form2551QDraft, _paper_size: PaperSize) -> Vec<u8> {
    match tempfile::tempdir()
        .ok()
        .and_then(|dir| render_2551q_print(draft, dir.path()).ok())
        .and_then(|result| fs::read(result.pdf_path).ok())
    {
        Some(pdf) => pdf,
        None => render_2551q_fallback_pdf(draft),
    }
}

// FormType, FormField, FieldKind, WidgetSpec, WidgetType
// are defined in formtype.rs and re-imported above.

/// Load and validate the [`FormType`] for the given form ID.
pub fn load_formtype(form_id: &str) -> Result<FormType, PrintError> {
    match form_id {
        FORM_2551Q_ID => {
            let ft: FormType = serde_json::from_str(LAYOUT_2551Q)?;
            if ft.form_id != form_id {
                return Err(PrintError::InvalidLayout(format!(
                    "formtype form_id {} does not match {form_id}",
                    ft.form_id
                )));
            }
            if (ft.page_width - 612.0).abs() > f64::EPSILON
                || (ft.page_height - 936.0).abs() > f64::EPSILON
            {
                return Err(PrintError::InvalidLayout(format!(
                    "expected 612 x 936pt, got {} x {}",
                    ft.page_width, ft.page_height
                )));
            }
            Ok(ft)
        }
        other => Err(PrintError::UnsupportedForm(other.to_string())),
    }
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

fn write_static_assets(form_id: &str, output_dir: &Path) -> Result<(), PrintError> {
    match form_id {
        FORM_2551Q_ID => {
            let svg_dir = output_dir.join("svgbase");
            fs::create_dir_all(&svg_dir)?;
            fs::write(svg_dir.join("page1.svg"), PAGE1_SVG_2551Q)?;
            fs::write(svg_dir.join("page2.svg"), PAGE2_SVG_2551Q)?;
            Ok(())
        }
        other => Err(PrintError::UnsupportedForm(other.to_string())),
    }
}

fn generate_typst(
    formtype: &FormType,
    fields: &BTreeMap<String, String>,
) -> Result<String, PrintError> {
    let mut lines = vec![
        format!(
            "#set page(width: {}pt, height: {}pt, margin: 0pt)",
            fmt_num(formtype.page_width),
            fmt_num(formtype.page_height)
        ),
        TEMPLATE_2551Q.to_string(),
    ];

    for page in 1..=formtype.page_count() {
        lines.push(format!(
            "#page(background: image(\"svgbase/page{page}.svg\", width: {}pt, height: {}pt), foreground: {{",
            fmt_num(formtype.page_width),
            fmt_num(formtype.page_height)
        ));

        for field in formtype.fields.iter().filter(|field| field.page == page) {
            if let Some(line) = render_field(field, fields)? {
                lines.push(format!("  {line}"));
            }
        }

        lines.push("})[]".to_string());
    }

    Ok(lines.join("\n"))
}

fn render_field(
    field: &FormField,
    fields: &BTreeMap<String, String>,
) -> Result<Option<String>, PrintError> {
    let value = fields.get(&field.key).cloned().unwrap_or_default();
    match field.kind {
        FieldKind::Checkbox => {
            if boolish(&value) {
                Ok(Some(format!(
                    "mark({}, {})",
                    fmt_num(field.x),
                    fmt_num(field.y)
                )))
            } else {
                Ok(None)
            }
        }
        FieldKind::Text => {
            if value.is_empty() {
                Ok(None)
            } else {
                Ok(Some(format!(
                    "label({}, {}, {}, \"{}\")",
                    fmt_num(field.x),
                    fmt_num(field.y),
                    fmt_num(field.size.unwrap_or(8.5)),
                    typst_string(&value.to_uppercase())
                )))
            }
        }
        FieldKind::Cells => {
            if value.is_empty() {
                Ok(None)
            } else {
                Ok(Some(format!(
                    "cells({}, {}, {}, \"{}\")",
                    fmt_num(field.x),
                    fmt_num(field.y),
                    fmt_num(required(field.cell_w, &field.key, "cell_w")?),
                    typst_string(&value.to_uppercase())
                )))
            }
        }
        FieldKind::Amount => Ok(Some(format!(
            "amount({}, {}, {}, {}, {}, \"{}\")",
            fmt_num(field.x),
            fmt_num(field.y),
            fmt_num(required(field.cell_w, &field.key, "cell_w")?),
            required(field.int_cells, &field.key, "int_cells")?,
            fmt_num(required(field.dec_x, &field.key, "dec_x")?),
            typst_string(&normalize_amount(&value))
        ))),
    }
}

fn required<T: Copy>(value: Option<T>, key: &str, field: &str) -> Result<T, PrintError> {
    value.ok_or_else(|| PrintError::InvalidLayout(format!("{key} missing {field}")))
}

fn export_preview_pngs(
    typ_path: &Path,
    output_dir: &Path,
    page_count: usize,
) -> Result<Vec<PathBuf>, PrintError> {
    let pattern = output_dir.join("preview-{p}.png");
    let output = Command::new("typst")
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

fn normalize_amount(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "0.00".to_string();
    }

    let cleaned = trimmed.replace(',', "");
    match cleaned.parse::<f64>() {
        Ok(number) => format!("{number:.2}"),
        Err(_) => cleaned,
    }
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
        format!("Total Amount Payable: {:.2}", draft.tax_payable),
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
    let (width, height) = PaperSize::A4.points();
    build_simple_pdf(width, height, lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bir_core::naming::Tin;
    use bir_core::profile::{TaxpayerProfile, TaxpayerType};
    use std::collections::BTreeSet;

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
            email_tracking_enabled: false,
            email_auth_method: Default::default(),
            imap_email: None,
            imap_host: None,
            _imap_enabled_compat: None,

            test_notification_enabled: false,
            imap_app_password: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
        };
        let mut draft = Form2551QDraft::new_from_profile(&profile, 2026, 1);
        draft.schedule_1[0].taxable_amount = 10_000.0;
        draft.creditable_tax_withheld = 25.0;
        draft.recompute();
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
    fn sample_data_covers_required_layout_fields() {
        let layout = load_formtype(FORM_2551Q_ID).expect("formtype should load");
        let fields = sample_draft().to_bir_field_map();
        validate_fields(&layout, &fields).expect("sample fields should cover layout");
    }

    #[test]
    fn generated_typst_uses_official_page_size() {
        let layout = load_formtype(FORM_2551Q_ID).expect("formtype should load");
        let fields = sample_draft().to_bir_field_map();
        let typst = generate_typst(&layout, &fields).expect("typst should render");

        assert!(typst.contains("#set page(width: 612pt, height: 936pt, margin: 0pt)"));
        assert!(typst.contains("svgbase/page1.svg"));
        assert!(typst.contains("svgbase/page2.svg"));
    }

    #[test]
    fn cli_fallback_compiles_pdf_and_preview_pngs() {
        if Command::new("typst").arg("--version").output().is_err() {
            eprintln!("typst CLI not installed; skipping");
            return;
        }

        let temp_dir = tempfile::tempdir().expect("temp dir");
        let result = render_2551q_print(&sample_draft(), temp_dir.path()).expect("render print");

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
    fn layout_keys_are_unique() {
        let layout = load_formtype(FORM_2551Q_ID).expect("formtype should load");
        let mut seen = BTreeSet::new();
        for field in layout.fields {
            assert!(seen.insert(field.key), "duplicate layout field");
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
