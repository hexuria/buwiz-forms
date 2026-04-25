use bir_core::forms::form_2551q::Form2551QDraft;
use std::fs;
use std::path::{Path, PathBuf};

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

pub fn write_2551q_pdf(
    draft: &Form2551QDraft,
    paper_size: PaperSize,
    output_path: impl AsRef<Path>,
) -> std::io::Result<PathBuf> {
    let pdf = render_2551q_pdf(draft, paper_size);
    let output_path = output_path.as_ref().to_path_buf();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output_path, pdf)?;
    Ok(output_path)
}

pub fn render_2551q_pdf(draft: &Form2551QDraft, paper_size: PaperSize) -> Vec<u8> {
    let (width, height) = paper_size.points();
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

#[cfg(test)]
mod tests {
    use super::*;
    use bir_core::forms::form_2551q::Form2551QDraft;
    use bir_core::naming::Tin;
    use bir_core::profile::{TaxpayerProfile, TaxpayerType};

    #[test]
    fn renders_non_empty_pdf() {
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
        };
        let draft = Form2551QDraft::new_from_profile(&profile, 2026, 1);
        let pdf = render_2551q_pdf(&draft, PaperSize::A4);
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.len() > 1000);
    }
}
