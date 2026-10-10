//! Region / province / city options of the excise returns' "Place of
//! Production" and "Place of Removal" items (2200S, 2200AN, ...).
//!
//! `data/excise-places.json` is the official `xml/region.xml`,
//! `province.xml` and `city.xml` (eBIRForms 7.9.6.2.1). Each page keeps the
//! rows whose form-type column contains its own code (`indexOf('2200S')`)
//! and fills its selects with `value=<code>` behind a `00` placeholder.

use std::sync::OnceLock;

use serde::Deserialize;

#[derive(Deserialize)]
struct Places {
    regions: Vec<(String, String, String)>,
    provinces: Vec<(String, String, String, String)>,
    cities: Vec<(String, String, String, String, String)>,
}

fn places() -> &'static Places {
    static PLACES: OnceLock<Places> = OnceLock::new();
    PLACES.get_or_init(|| {
        serde_json::from_str(include_str!("../../data/excise-places.json"))
            .expect("excise-places.json is valid")
    })
}

/// `(code, name)` of the regions a form's page offers.
pub fn regions(form: &str) -> Vec<(&'static str, &'static str)> {
    places()
        .regions
        .iter()
        .filter(|(_, _, forms)| forms.contains(form))
        .map(|(code, name, _)| (code.as_str(), name.as_str()))
        .collect()
}

/// `(code, name)` of a region's provinces (`getProvince`).
pub fn provinces(form: &str, region: &str) -> Vec<(&'static str, &'static str)> {
    places()
        .provinces
        .iter()
        .filter(|(r, _, _, forms)| r == region && forms.contains(form))
        .map(|(_, code, name, _)| (code.as_str(), name.as_str()))
        .collect()
}

/// `(code, name)` of a province's cities (`getCity`).
pub fn cities(form: &str, region: &str, province: &str) -> Vec<(&'static str, &'static str)> {
    places()
        .cities
        .iter()
        .filter(|(r, p, _, _, forms)| r == region && p == province && forms.contains(form))
        .map(|(_, _, code, name, _)| (code.as_str(), name.as_str()))
        .collect()
}

/// A region, province and city the page's three selects can hold together.
pub fn is_official_place(form: &str, region: &str, province: &str, city: &str) -> bool {
    regions(form).iter().any(|(code, _)| *code == region)
        && provinces(form, region)
            .iter()
            .any(|(code, _)| *code == province)
        && cities(form, region, province)
            .iter()
            .any(|(code, _)| *code == city)
}

/// The name shown for a code, if any.
pub fn place_name(code: &str) -> Option<&'static str> {
    let p = places();
    p.regions
        .iter()
        .find(|r| r.0 == code)
        .map(|r| r.1.as_str())
        .or_else(|| {
            p.provinces
                .iter()
                .find(|r| r.1 == code)
                .map(|r| r.2.as_str())
        })
        .or_else(|| p.cities.iter().find(|r| r.2 == code).map(|r| r.3.as_str()))
}

/// Region, province and city codes of one place item.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct ExcisePlace {
    pub region: String,
    pub province: String,
    pub city: String,
}

impl ExcisePlace {
    /// The select values: `00` for an unselected level.
    pub fn values(&self) -> [String; 3] {
        let value = |code: &str| {
            if code.is_empty() {
                "00".to_string()
            } else {
                code.to_string()
            }
        };
        [
            value(&self.region),
            value(&self.province),
            value(&self.city),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn place_lists_follow_the_official_xml() {
        assert_eq!(regions("2200S").len(), 17);
        let ncr = regions("2200S")
            .into_iter()
            .find(|(_, name)| name.contains("NCR"))
            .map(|(code, _)| code)
            .expect("NCR");
        let provinces = provinces("2200S", ncr);
        assert!(!provinces.is_empty());
        let (province, _) = provinces[0];
        let (city, _) = cities("2200S", ncr, province)[0];
        assert!(is_official_place("2200S", ncr, province, city));
        assert!(!is_official_place("2200S", ncr, province, "999999999"));
    }
}
