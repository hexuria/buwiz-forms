#![recursion_limit = "256"]

//! HTML form rendering contracts and validated native PDF output.
//!
//! Rust owns printable values and validation; the bundled semantic HTML/CSS
//! renderer owns page layout. Platform WebViews create print/PDF output, which
//! is validated and written atomically through [`html_output`].

pub mod certification_observation;
pub mod html;
pub mod html_forms;
pub mod html_output;
#[cfg(any(test, feature = "native-output-evidence"))]
pub mod html_output_evidence;
pub mod html_support;
pub mod pdf_util;

pub use pdf_util::{append_text_pages_to_pdf, build_simple_confirmation_pdf, PdfUtilityError};
