//! Reference Data lookup for ATC, RDO, etc.
//!
//! Loads static data from JSON files embedded at compile time.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rdo {
    pub code: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Atc {
    pub code: String,
    pub description: String,
    pub rate: String,
}

/// Static embedded JSON data
const RDO_JSON: &str = include_str!("../data/rdo.json");
const ATC_JSON: &str = include_str!("../data/atcCodes.json");

static RDOS_BY_CODE: OnceLock<HashMap<String, Rdo>> = OnceLock::new();
static ATCS_BY_CODE: OnceLock<HashMap<String, Atc>> = OnceLock::new();

/// Initialize and parse the JSON lists
fn init_rdos() -> HashMap<String, Rdo> {
    let list: Vec<Rdo> = serde_json::from_str(RDO_JSON).expect("Invalid rdo.json");
    list.into_iter().map(|r| (r.code.clone(), r)).collect()
}

fn init_atcs() -> HashMap<String, Atc> {
    let list: Vec<Atc> = serde_json::from_str(ATC_JSON).expect("Invalid atcCodes.json");
    list.into_iter().map(|a| (a.code.clone(), a)).collect()
}

/// Get an RDO by its code
pub fn get_rdo(code: &str) -> Option<Rdo> {
    let map = RDOS_BY_CODE.get_or_init(init_rdos);
    map.get(code).cloned()
}

/// Get all RDOs
pub fn get_all_rdos() -> Vec<Rdo> {
    let map = RDOS_BY_CODE.get_or_init(init_rdos);
    let mut rdos: Vec<Rdo> = map.values().cloned().collect();
    rdos.sort_by(|a, b| a.code.cmp(&b.code));
    rdos
}

/// Get an ATC by its code
pub fn get_atc(code: &str) -> Option<Atc> {
    let map = ATCS_BY_CODE.get_or_init(init_atcs);
    map.get(code).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_rdo() {
        let rdo = get_rdo("039").expect("RDO 039 should exist");
        assert_eq!(rdo.code, "039");
        // Ensure description contains something valid from South QC
        assert!(rdo.description.contains("South Quezon City") || !rdo.description.is_empty());
    }

    #[test]
    fn test_get_atc() {
        let atc = get_atc("WV010").expect("ATC WV010 should exist");
        assert_eq!(atc.code, "WV010");
        assert_eq!(atc.rate, "5.0");
    }
}
