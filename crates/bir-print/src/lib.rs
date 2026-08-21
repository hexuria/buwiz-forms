#![recursion_limit = "256"]

//! Frozen HTML fill/print and validated native PDF output.
//!
//! Rust owns printable values. Frozen HTML in `html-frozen/` is the print
//! surface. Platform WebViews create print/PDF output, validated through
//! [`html_output`].

pub mod certification_observation;
pub mod frozen_html;
pub mod html_output;
pub mod pdf_util;

pub use pdf_util::{append_text_pages_to_pdf, build_simple_confirmation_pdf, PdfUtilityError};
