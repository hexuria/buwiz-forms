//! Embedded reference data (RDO, ATC, regions, tax types, treaties).
//!
//! All reference data is compiled into the binary — no external files
//! that users could tamper with.

// TODO: Convert XML reference data to JSON and embed via include_str!()
// TODO: Lazy-static deserialization on first access
// TODO: Lookup functions (by code, by form type, etc.)
