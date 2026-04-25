//! Alphanumeric Tax Code (ATC) reference data for percentage tax forms.

pub struct AtcEntry {
    pub code: &'static str,
    pub description: &'static str,
    pub rate: f64,
}

pub const ATC_TABLE_2551Q: &[AtcEntry] = &[
    AtcEntry {
        code: "PT010",
        description: "Persons exempt from VAT under Sec. 109(BB) [Sec. 116]",
        rate: 0.03,
    },
    AtcEntry {
        code: "PT040",
        description: "Domestic carriers and keepers of garages [Sec. 117]",
        rate: 0.03,
    },
    AtcEntry {
        code: "PT050",
        description: "International carriers doing business in PH [Sec. 118]",
        rate: 0.03,
    },
    AtcEntry {
        code: "PT060",
        description: "Franchise grantees – radio/TV broadcasting [Sec. 119]",
        rate: 0.03,
    },
    AtcEntry {
        code: "PT070",
        description: "Franchise grantees – gas/water utilities [Sec. 119]",
        rate: 0.02,
    },
    AtcEntry {
        code: "PT080",
        description: "Banks and non-bank financial intermediaries [Sec. 121]",
        rate: 0.05,
    },
    AtcEntry {
        code: "PT090",
        description: "Other non-bank financial intermediaries [Sec. 122]",
        rate: 0.05,
    },
    AtcEntry {
        code: "PT100",
        description: "Life insurance premiums [Sec. 123]",
        rate: 0.02,
    },
    AtcEntry {
        code: "PT110",
        description: "Agents of foreign insurance companies [Sec. 124]",
        rate: 0.04,
    },
    AtcEntry {
        code: "PT120",
        description: "Amusement places [Sec. 125]",
        rate: 0.18,
    },
    AtcEntry {
        code: "PT130",
        description: "Winners – horse racing [Sec. 126]",
        rate: 0.10,
    },
    AtcEntry {
        code: "PT140",
        description: "Sale, barter, exchange of shares of stock [Sec. 127]",
        rate: 0.006,
    },
    AtcEntry {
        code: "PT150",
        description: "Initial Public Offerings (IPO) [Sec. 127A]",
        rate: 0.04,
    },
    AtcEntry {
        code: "PT160",
        description: "Sale of real property as ordinary asset [Sec. 127B]",
        rate: 0.06,
    },
];

/// Looks up an ATC entry by code. Returns None if not found.
pub fn find_atc(code: &str) -> Option<&'static AtcEntry> {
    ATC_TABLE_2551Q.iter().find(|e| e.code == code)
}
