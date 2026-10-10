//! Sample RDO / form-code catalog — **non-authoritative** (q8).

use core::fmt;

use serde::Deserialize;

const CATALOG_TOML: &str = include_str!("../data/catalog.toml");

/// Kind of catalog entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeKind {
    /// Form code (e.g. 2303 COR).
    Form,
    /// Revenue district office.
    Rdo,
}

/// One catalog hit.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CatalogHit {
    code: Box<str>,
    kind: CodeKind,
    label: Box<str>,
}

impl CatalogHit {
    /// Matched code.
    pub fn code(&self) -> &str {
        &self.code
    }
    /// Kind.
    pub const fn kind(&self) -> CodeKind {
        self.kind
    }
    /// Human label.
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// Why a catalog could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogError {
    /// TOML / validation failure.
    Invalid(String),
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(e) => write!(f, "catalog: {e}"),
        }
    }
}

impl std::error::Error for CatalogError {}

#[derive(Debug, Deserialize)]
struct File {
    codes: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
struct Entry {
    code: String,
    kind: CodeKind,
    label: String,
}

/// Longest-exact catalog. Sample data only — not an authority.
#[derive(Debug, Clone)]
pub struct Catalog {
    entries: Vec<(Box<str>, CodeKind, Box<str>)>,
}

impl Catalog {
    /// Loads the embedded sample catalog.
    ///
    /// # Errors
    /// [`CatalogError`].
    pub fn load() -> Result<Self, CatalogError> {
        Self::from_toml(CATALOG_TOML)
    }

    /// Parses a TOML catalog.
    ///
    /// # Errors
    /// [`CatalogError`].
    pub fn from_toml(toml_src: &str) -> Result<Self, CatalogError> {
        let file: File =
            toml::from_str(toml_src).map_err(|e| CatalogError::Invalid(e.to_string()))?;
        if file.codes.is_empty() {
            return Err(CatalogError::Invalid("empty".into()));
        }
        let mut entries: Vec<_> = file
            .codes
            .into_iter()
            .map(|e| (e.code.into_boxed_str(), e.kind, e.label.into_boxed_str()))
            .collect();
        entries.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then(a.0.cmp(&b.0)));
        Ok(Self { entries })
    }

    /// Longest exact match of `needle` against catalog codes (case-sensitive on the code).
    pub fn longest_exact(&self, needle: &str) -> Option<CatalogHit> {
        // Entries are sorted longest-first, then lexical.
        self.entries
            .iter()
            .find(|(code, _, _)| needle == code.as_ref())
            .map(|(code, kind, label)| CatalogHit {
                code: code.clone(),
                kind: *kind,
                label: label.clone(),
            })
    }

    /// Lookup by exact code.
    pub fn get(&self, code: &str) -> Option<CatalogHit> {
        self.entries
            .iter()
            .find(|(c, _, _)| c.as_ref() == code)
            .map(|(code, kind, label)| CatalogHit {
                code: code.clone(),
                kind: *kind,
                label: label.clone(),
            })
    }

    /// Fingerprint of the catalog contents.
    pub fn fingerprint(&self) -> Vec<u8> {
        let mut h = blake3::Hasher::new();
        h.update(b"bir-catalog-v1");
        for (c, k, l) in &self.entries {
            h.update(c.as_bytes());
            h.update(&[match k {
                CodeKind::Form => 1,
                CodeKind::Rdo => 2,
            }]);
            h.update(l.as_bytes());
        }
        h.finalize().as_bytes().to_vec()
    }
}
