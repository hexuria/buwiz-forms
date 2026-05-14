//! BIR Form 0605 — Typed draft struct and computation logic.
//!
//! Generated from savefile: 00000000000000-0605-12312025143024.xml
//! Total BIR fields: 235
//! Form-specific fields: 208
//!
//! ⚠️ ScaffoldOnly — formula evidence not yet verified

use crate::forms::{FilingStatus, FormValidator};
use crate::profile::TaxpayerProfile;
use serde::{Deserialize, Serialize};

/// Complete draft for Form 0605.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Form0605Draft {
    /// Database row ID (None before first save)
    pub id: Option<i64>,

    // === Filing Period ===
    pub tin: String,
    pub taxable_year: u16,
    pub month: u8,

    // === Header / Options ===

    // === Profile Fields (pre-filled) ===
    pub rdo_code: String,
    pub taxpayer_name: String,
    pub registered_address: String,
    pub zip_code: String,
    pub contact_number: String,
    pub email: String,

    // === itemApprovedYN ===
    /// BIR: `frm0605:itemApprovedYN:_1` (sample: `false`)
    pub item_approved_yn_1: bool,
    /// BIR: `frm0605:itemApprovedYN:_2` (sample: `false`)
    pub item_approved_yn_2: bool,

    // === itemMannerOfPayment ===
    /// BIR: `frm0605:itemMannerOfPayment:_1` (sample: `false`)
    pub item_manner_of_payment_1: bool,
    /// BIR: `frm0605:itemMannerOfPayment:_2` (sample: `false`)
    pub item_manner_of_payment_2: bool,
    /// BIR: `frm0605:itemMannerOfPayment:_3` (sample: `false`)
    pub item_manner_of_payment_3: bool,
    /// BIR: `frm0605:itemMannerOfPayment:_4` (sample: `false`)
    pub item_manner_of_payment_4: bool,
    /// BIR: `frm0605:itemMannerOfPayment:_5` (sample: `true`)
    pub item_manner_of_payment_5: bool,

    // === itemMannerOfPaymentB ===
    /// BIR: `frm0605:itemMannerOfPaymentB:_1` (sample: `false`)
    pub item_manner_of_payment_b_1: bool,
    /// BIR: `frm0605:itemMannerOfPaymentB:_2` (sample: `false`)
    pub item_manner_of_payment_b_2: bool,

    // === itemModeOfPayment ===
    /// BIR: `frm0605:itemModeOfPayment:_1` (sample: `true`)
    pub item_mode_of_payment_1: bool,
    /// BIR: `frm0605:itemModeOfPayment:_2` (sample: `false`)
    pub item_mode_of_payment_2: bool,
    /// BIR: `frm0605:itemModeOfPayment:_3` (sample: `false`)
    pub item_mode_of_payment_3: bool,

    // === other ===
    /// BIR: `AtcCode1` (sample: `false`)
    pub atc_code1: bool,
    /// BIR: `AtcCode10` (sample: `false`)
    pub atc_code10: bool,
    /// BIR: `AtcCode100` (sample: `false`)
    pub atc_code100: bool,
    /// BIR: `AtcCode101` (sample: `false`)
    pub atc_code101: bool,
    /// BIR: `AtcCode102` (sample: `false`)
    pub atc_code102: bool,
    /// BIR: `AtcCode103` (sample: `false`)
    pub atc_code103: bool,
    /// BIR: `AtcCode104` (sample: `false`)
    pub atc_code104: bool,
    /// BIR: `AtcCode105` (sample: `false`)
    pub atc_code105: bool,
    /// BIR: `AtcCode106` (sample: `false`)
    pub atc_code106: bool,
    /// BIR: `AtcCode107` (sample: `false`)
    pub atc_code107: bool,
    /// BIR: `AtcCode108` (sample: `false`)
    pub atc_code108: bool,
    /// BIR: `AtcCode109` (sample: `false`)
    pub atc_code109: bool,
    /// BIR: `AtcCode11` (sample: `false`)
    pub atc_code11: bool,
    /// BIR: `AtcCode110` (sample: `false`)
    pub atc_code110: bool,
    /// BIR: `AtcCode111` (sample: `false`)
    pub atc_code111: bool,
    /// BIR: `AtcCode112` (sample: `false`)
    pub atc_code112: bool,
    /// BIR: `AtcCode113` (sample: `false`)
    pub atc_code113: bool,
    /// BIR: `AtcCode114` (sample: `false`)
    pub atc_code114: bool,
    /// BIR: `AtcCode115` (sample: `false`)
    pub atc_code115: bool,
    /// BIR: `AtcCode116` (sample: `false`)
    pub atc_code116: bool,
    /// BIR: `AtcCode117` (sample: `false`)
    pub atc_code117: bool,
    /// BIR: `AtcCode118` (sample: `false`)
    pub atc_code118: bool,
    /// BIR: `AtcCode119` (sample: `false`)
    pub atc_code119: bool,
    /// BIR: `AtcCode12` (sample: `false`)
    pub atc_code12: bool,
    /// BIR: `AtcCode120` (sample: `false`)
    pub atc_code120: bool,
    /// BIR: `AtcCode121` (sample: `false`)
    pub atc_code121: bool,
    /// BIR: `AtcCode122` (sample: `false`)
    pub atc_code122: bool,
    /// BIR: `AtcCode123` (sample: `false`)
    pub atc_code123: bool,
    /// BIR: `AtcCode124` (sample: `false`)
    pub atc_code124: bool,
    /// BIR: `AtcCode125` (sample: `false`)
    pub atc_code125: bool,
    /// BIR: `AtcCode126` (sample: `false`)
    pub atc_code126: bool,
    /// BIR: `AtcCode127` (sample: `false`)
    pub atc_code127: bool,
    /// BIR: `AtcCode128` (sample: `false`)
    pub atc_code128: bool,
    /// BIR: `AtcCode129` (sample: `false`)
    pub atc_code129: bool,
    /// BIR: `AtcCode13` (sample: `false`)
    pub atc_code13: bool,
    /// BIR: `AtcCode130` (sample: `false`)
    pub atc_code130: bool,
    /// BIR: `AtcCode131` (sample: `false`)
    pub atc_code131: bool,
    /// BIR: `AtcCode132` (sample: `false`)
    pub atc_code132: bool,
    /// BIR: `AtcCode133` (sample: `false`)
    pub atc_code133: bool,
    /// BIR: `AtcCode134` (sample: `false`)
    pub atc_code134: bool,
    /// BIR: `AtcCode135` (sample: `false`)
    pub atc_code135: bool,
    /// BIR: `AtcCode136` (sample: `false`)
    pub atc_code136: bool,
    /// BIR: `AtcCode137` (sample: `false`)
    pub atc_code137: bool,
    /// BIR: `AtcCode138` (sample: `false`)
    pub atc_code138: bool,
    /// BIR: `AtcCode139` (sample: `false`)
    pub atc_code139: bool,
    /// BIR: `AtcCode14` (sample: `false`)
    pub atc_code14: bool,
    /// BIR: `AtcCode140` (sample: `false`)
    pub atc_code140: bool,
    /// BIR: `AtcCode141` (sample: `false`)
    pub atc_code141: bool,
    /// BIR: `AtcCode142` (sample: `false`)
    pub atc_code142: bool,
    /// BIR: `AtcCode15` (sample: `false`)
    pub atc_code15: bool,
    /// BIR: `AtcCode16` (sample: `false`)
    pub atc_code16: bool,
    /// BIR: `AtcCode17` (sample: `false`)
    pub atc_code17: bool,
    /// BIR: `AtcCode18` (sample: `false`)
    pub atc_code18: bool,
    /// BIR: `AtcCode19` (sample: `false`)
    pub atc_code19: bool,
    /// BIR: `AtcCode2` (sample: `false`)
    pub atc_code2: bool,
    /// BIR: `AtcCode20` (sample: `false`)
    pub atc_code20: bool,
    /// BIR: `AtcCode21` (sample: `false`)
    pub atc_code21: bool,
    /// BIR: `AtcCode22` (sample: `false`)
    pub atc_code22: bool,
    /// BIR: `AtcCode23` (sample: `false`)
    pub atc_code23: bool,
    /// BIR: `AtcCode24` (sample: `true`)
    pub atc_code24: bool,
    /// BIR: `AtcCode25` (sample: `false`)
    pub atc_code25: bool,
    /// BIR: `AtcCode26` (sample: `false`)
    pub atc_code26: bool,
    /// BIR: `AtcCode27` (sample: `false`)
    pub atc_code27: bool,
    /// BIR: `AtcCode28` (sample: `false`)
    pub atc_code28: bool,
    /// BIR: `AtcCode29` (sample: `false`)
    pub atc_code29: bool,
    /// BIR: `AtcCode3` (sample: `false`)
    pub atc_code3: bool,
    /// BIR: `AtcCode30` (sample: `false`)
    pub atc_code30: bool,
    /// BIR: `AtcCode31` (sample: `false`)
    pub atc_code31: bool,
    /// BIR: `AtcCode32` (sample: `false`)
    pub atc_code32: bool,
    /// BIR: `AtcCode33` (sample: `false`)
    pub atc_code33: bool,
    /// BIR: `AtcCode34` (sample: `false`)
    pub atc_code34: bool,
    /// BIR: `AtcCode35` (sample: `false`)
    pub atc_code35: bool,
    /// BIR: `AtcCode36` (sample: `false`)
    pub atc_code36: bool,
    /// BIR: `AtcCode37` (sample: `false`)
    pub atc_code37: bool,
    /// BIR: `AtcCode38` (sample: `false`)
    pub atc_code38: bool,
    /// BIR: `AtcCode39` (sample: `false`)
    pub atc_code39: bool,
    /// BIR: `AtcCode4` (sample: `false`)
    pub atc_code4: bool,
    /// BIR: `AtcCode40` (sample: `false`)
    pub atc_code40: bool,
    /// BIR: `AtcCode41` (sample: `false`)
    pub atc_code41: bool,
    /// BIR: `AtcCode42` (sample: `false`)
    pub atc_code42: bool,
    /// BIR: `AtcCode43` (sample: `false`)
    pub atc_code43: bool,
    /// BIR: `AtcCode44` (sample: `false`)
    pub atc_code44: bool,
    /// BIR: `AtcCode45` (sample: `false`)
    pub atc_code45: bool,
    /// BIR: `AtcCode46` (sample: `false`)
    pub atc_code46: bool,
    /// BIR: `AtcCode47` (sample: `false`)
    pub atc_code47: bool,
    /// BIR: `AtcCode48` (sample: `false`)
    pub atc_code48: bool,
    /// BIR: `AtcCode49` (sample: `false`)
    pub atc_code49: bool,
    /// BIR: `AtcCode5` (sample: `false`)
    pub atc_code5: bool,
    /// BIR: `AtcCode50` (sample: `false`)
    pub atc_code50: bool,
    /// BIR: `AtcCode51` (sample: `false`)
    pub atc_code51: bool,
    /// BIR: `AtcCode52` (sample: `false`)
    pub atc_code52: bool,
    /// BIR: `AtcCode53` (sample: `false`)
    pub atc_code53: bool,
    /// BIR: `AtcCode54` (sample: `false`)
    pub atc_code54: bool,
    /// BIR: `AtcCode55` (sample: `false`)
    pub atc_code55: bool,
    /// BIR: `AtcCode56` (sample: `false`)
    pub atc_code56: bool,
    /// BIR: `AtcCode57` (sample: `false`)
    pub atc_code57: bool,
    /// BIR: `AtcCode58` (sample: `false`)
    pub atc_code58: bool,
    /// BIR: `AtcCode59` (sample: `false`)
    pub atc_code59: bool,
    /// BIR: `AtcCode6` (sample: `false`)
    pub atc_code6: bool,
    /// BIR: `AtcCode60` (sample: `false`)
    pub atc_code60: bool,
    /// BIR: `AtcCode61` (sample: `false`)
    pub atc_code61: bool,
    /// BIR: `AtcCode62` (sample: `false`)
    pub atc_code62: bool,
    /// BIR: `AtcCode63` (sample: `false`)
    pub atc_code63: bool,
    /// BIR: `AtcCode64` (sample: `false`)
    pub atc_code64: bool,
    /// BIR: `AtcCode65` (sample: `false`)
    pub atc_code65: bool,
    /// BIR: `AtcCode66` (sample: `false`)
    pub atc_code66: bool,
    /// BIR: `AtcCode67` (sample: `false`)
    pub atc_code67: bool,
    /// BIR: `AtcCode68` (sample: `false`)
    pub atc_code68: bool,
    /// BIR: `AtcCode69` (sample: `false`)
    pub atc_code69: bool,
    /// BIR: `AtcCode7` (sample: `false`)
    pub atc_code7: bool,
    /// BIR: `AtcCode70` (sample: `false`)
    pub atc_code70: bool,
    /// BIR: `AtcCode71` (sample: `false`)
    pub atc_code71: bool,
    /// BIR: `AtcCode72` (sample: `false`)
    pub atc_code72: bool,
    /// BIR: `AtcCode73` (sample: `false`)
    pub atc_code73: bool,
    /// BIR: `AtcCode74` (sample: `false`)
    pub atc_code74: bool,
    /// BIR: `AtcCode75` (sample: `false`)
    pub atc_code75: bool,
    /// BIR: `AtcCode76` (sample: `false`)
    pub atc_code76: bool,
    /// BIR: `AtcCode77` (sample: `false`)
    pub atc_code77: bool,
    /// BIR: `AtcCode78` (sample: `false`)
    pub atc_code78: bool,
    /// BIR: `AtcCode79` (sample: `false`)
    pub atc_code79: bool,
    /// BIR: `AtcCode8` (sample: `false`)
    pub atc_code8: bool,
    /// BIR: `AtcCode80` (sample: `false`)
    pub atc_code80: bool,
    /// BIR: `AtcCode81` (sample: `false`)
    pub atc_code81: bool,
    /// BIR: `AtcCode82` (sample: `false`)
    pub atc_code82: bool,
    /// BIR: `AtcCode83` (sample: `false`)
    pub atc_code83: bool,
    /// BIR: `AtcCode84` (sample: `false`)
    pub atc_code84: bool,
    /// BIR: `AtcCode85` (sample: `false`)
    pub atc_code85: bool,
    /// BIR: `AtcCode86` (sample: `false`)
    pub atc_code86: bool,
    /// BIR: `AtcCode87` (sample: `false`)
    pub atc_code87: bool,
    /// BIR: `AtcCode88` (sample: `false`)
    pub atc_code88: bool,
    /// BIR: `AtcCode89` (sample: `false`)
    pub atc_code89: bool,
    /// BIR: `AtcCode9` (sample: `false`)
    pub atc_code9: bool,
    /// BIR: `AtcCode90` (sample: `false`)
    pub atc_code90: bool,
    /// BIR: `AtcCode91` (sample: `false`)
    pub atc_code91: bool,
    /// BIR: `AtcCode92` (sample: `false`)
    pub atc_code92: bool,
    /// BIR: `AtcCode93` (sample: `false`)
    pub atc_code93: bool,
    /// BIR: `AtcCode94` (sample: `false`)
    pub atc_code94: bool,
    /// BIR: `AtcCode95` (sample: `false`)
    pub atc_code95: bool,
    /// BIR: `AtcCode96` (sample: `false`)
    pub atc_code96: bool,
    /// BIR: `AtcCode97` (sample: `false`)
    pub atc_code97: bool,
    /// BIR: `AtcCode98` (sample: `false`)
    pub atc_code98: bool,
    /// BIR: `AtcCode99` (sample: `false`)
    pub atc_code99: bool,
    /// BIR: `TaxTypeCode1` (sample: `false`)
    pub tax_type_code1: bool,
    /// BIR: `TaxTypeCode10` (sample: `false`)
    pub tax_type_code10: bool,
    /// BIR: `TaxTypeCode11` (sample: `false`)
    pub tax_type_code11: bool,
    /// BIR: `TaxTypeCode12` (sample: `false`)
    pub tax_type_code12: bool,
    /// BIR: `TaxTypeCode13` (sample: `false`)
    pub tax_type_code13: bool,
    /// BIR: `TaxTypeCode14` (sample: `false`)
    pub tax_type_code14: bool,
    /// BIR: `TaxTypeCode15` (sample: `false`)
    pub tax_type_code15: bool,
    /// BIR: `TaxTypeCode16` (sample: `false`)
    pub tax_type_code16: bool,
    /// BIR: `TaxTypeCode17` (sample: `false`)
    pub tax_type_code17: bool,
    /// BIR: `TaxTypeCode18` (sample: `false`)
    pub tax_type_code18: bool,
    /// BIR: `TaxTypeCode19` (sample: `false`)
    pub tax_type_code19: bool,
    /// BIR: `TaxTypeCode2` (sample: `false`)
    pub tax_type_code2: bool,
    /// BIR: `TaxTypeCode20` (sample: `false`)
    pub tax_type_code20: bool,
    /// BIR: `TaxTypeCode21` (sample: `false`)
    pub tax_type_code21: bool,
    /// BIR: `TaxTypeCode22` (sample: `false`)
    pub tax_type_code22: bool,
    /// BIR: `TaxTypeCode23` (sample: `false`)
    pub tax_type_code23: bool,
    /// BIR: `TaxTypeCode24` (sample: `false`)
    pub tax_type_code24: bool,
    /// BIR: `TaxTypeCode25` (sample: `false`)
    pub tax_type_code25: bool,
    /// BIR: `TaxTypeCode26` (sample: `false`)
    pub tax_type_code26: bool,
    /// BIR: `TaxTypeCode27` (sample: `false`)
    pub tax_type_code27: bool,
    /// BIR: `TaxTypeCode28` (sample: `false`)
    pub tax_type_code28: bool,
    /// BIR: `TaxTypeCode29` (sample: `false`)
    pub tax_type_code29: bool,
    /// BIR: `TaxTypeCode3` (sample: `false`)
    pub tax_type_code3: bool,
    /// BIR: `TaxTypeCode30` (sample: `false`)
    pub tax_type_code30: bool,
    /// BIR: `TaxTypeCode31` (sample: `false`)
    pub tax_type_code31: bool,
    /// BIR: `TaxTypeCode32` (sample: `false`)
    pub tax_type_code32: bool,
    /// BIR: `TaxTypeCode33` (sample: `false`)
    pub tax_type_code33: bool,
    /// BIR: `TaxTypeCode34` (sample: `false`)
    pub tax_type_code34: bool,
    /// BIR: `TaxTypeCode35` (sample: `false`)
    pub tax_type_code35: bool,
    /// BIR: `TaxTypeCode36` (sample: `false`)
    pub tax_type_code36: bool,
    /// BIR: `TaxTypeCode37` (sample: `false`)
    pub tax_type_code37: bool,
    /// BIR: `TaxTypeCode4` (sample: `false`)
    pub tax_type_code4: bool,
    /// BIR: `TaxTypeCode5` (sample: `false`)
    pub tax_type_code5: bool,
    /// BIR: `TaxTypeCode6` (sample: `false`)
    pub tax_type_code6: bool,
    /// BIR: `TaxTypeCode7` (sample: `false`)
    pub tax_type_code7: bool,
    /// BIR: `TaxTypeCode8` (sample: `false`)
    pub tax_type_code8: bool,
    /// BIR: `TaxTypeCode9` (sample: `true`)
    pub tax_type_code9: bool,

    // === schedule_atc ===
    /// BIR: `txtATCCode` (sample: `II011`)
    pub txt_atccode: String,

    // === shared_text ===
    /// BIR: `txtTaxTypeCode` (sample: `IT`)
    pub txt_tax_type_code: String,

    // === text_fields ===
    /// BIR: `frm0605:txtAddress` (sample: `OLONGAPO`)
    pub txt_address: String,
    /// BIR: `frm0605:txtDueDateDay` (sample: `31`)
    pub txt_due_date_day: u32,
    /// BIR: `frm0605:txtLineBus` (sample: `SOFTWARE DEVELOPMENT`)
    pub txt_line_bus: String,
    /// BIR: `frm0605:txtNoOfSheets` (sample: `10`)
    pub txt_no_of_sheets: u32,
    /// BIR: `frm0605:txtNumOfInstallment` (sample: `10`)
    pub txt_num_of_installment: u32,
    /// BIR: `frm0605:txtOthersName` (sample: `CANT CHOOSE PRELIMINARY OR ACC`)
    pub txt_others_name: String,
    /// BIR: `frm0605:txtReturnPeriodDay` (sample: `31`)
    pub txt_return_period_day: u32,
    /// BIR: `frm0605:txtTax19` (sample: `1,000.00`)
    pub txt_tax19: f64,
    /// BIR: `frm0605:txtTax20A` (sample: `10.00`)
    pub txt_tax20a: f64,
    /// BIR: `frm0605:txtTax20B` (sample: `20.00`)
    pub txt_tax20b: f64,
    /// BIR: `frm0605:txtTax20C` (sample: `1,000.00`)
    pub txt_tax20c: f64,
    /// BIR: `frm0605:txtTax20D` (sample: `1,030.00`)
    pub txt_tax20d: f64,
    /// BIR: `frm0605:txtTax21` (sample: `2,030.00`)
    pub txt_tax21: f64,

    // === txtClassification ===
    /// BIR: `frm0605:txtClassification:_1` (sample: `false`)
    pub txt_classification_1: bool,
    /// BIR: `frm0605:txtClassification:_2` (sample: `true`)
    pub txt_classification_2: bool,

    // === Lifecycle ===
    pub status: FilingStatus,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub submitted_at: Option<String>,
    #[serde(default)]
    pub confirmed_at: Option<String>,
    #[serde(default)]
    pub submission_filename: Option<String>,
    #[serde(default)]
    pub receipt_id: Option<i64>,
    #[serde(default)]
    pub submission_attempts: u32,
    #[serde(default)]
    pub next_retry_at: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
}

impl FormValidator for Form0605Draft {
    fn validate(&self) -> Vec<(String, String)> {
        let mut errors = Vec::new();
        if self.tin.is_empty() {
            errors.push(("tin".into(), "TIN is required".into()));
        }
        if self.taxpayer_name.is_empty() {
            errors.push(("taxpayer_name".into(), "Taxpayer name is required".into()));
        }
        // TODO: Add form-specific validation rules
        errors
    }
}

impl Form0605Draft {
    /// Create a new draft from a taxpayer profile.
    pub fn new_from_profile(profile: &TaxpayerProfile, year: u16, month: u8) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: None,
            tin: profile.tin.full(),
            taxable_year: year,
            month,
            rdo_code: profile.rdo_code.clone(),
            taxpayer_name: profile.full_name.clone(),
            registered_address: profile.registered_address.clone(),
            zip_code: profile.zip_code.clone(),
            contact_number: profile.phone.clone(),
            email: profile.email.clone(),
            item_approved_yn_1: false,
            item_approved_yn_2: false,
            item_manner_of_payment_1: false,
            item_manner_of_payment_2: false,
            item_manner_of_payment_3: false,
            item_manner_of_payment_4: false,
            item_manner_of_payment_5: true,
            item_manner_of_payment_b_1: false,
            item_manner_of_payment_b_2: false,
            item_mode_of_payment_1: true,
            item_mode_of_payment_2: false,
            item_mode_of_payment_3: false,
            atc_code1: false,
            atc_code10: false,
            atc_code100: false,
            atc_code101: false,
            atc_code102: false,
            atc_code103: false,
            atc_code104: false,
            atc_code105: false,
            atc_code106: false,
            atc_code107: false,
            atc_code108: false,
            atc_code109: false,
            atc_code11: false,
            atc_code110: false,
            atc_code111: false,
            atc_code112: false,
            atc_code113: false,
            atc_code114: false,
            atc_code115: false,
            atc_code116: false,
            atc_code117: false,
            atc_code118: false,
            atc_code119: false,
            atc_code12: false,
            atc_code120: false,
            atc_code121: false,
            atc_code122: false,
            atc_code123: false,
            atc_code124: false,
            atc_code125: false,
            atc_code126: false,
            atc_code127: false,
            atc_code128: false,
            atc_code129: false,
            atc_code13: false,
            atc_code130: false,
            atc_code131: false,
            atc_code132: false,
            atc_code133: false,
            atc_code134: false,
            atc_code135: false,
            atc_code136: false,
            atc_code137: false,
            atc_code138: false,
            atc_code139: false,
            atc_code14: false,
            atc_code140: false,
            atc_code141: false,
            atc_code142: false,
            atc_code15: false,
            atc_code16: false,
            atc_code17: false,
            atc_code18: false,
            atc_code19: false,
            atc_code2: false,
            atc_code20: false,
            atc_code21: false,
            atc_code22: false,
            atc_code23: false,
            atc_code24: true,
            atc_code25: false,
            atc_code26: false,
            atc_code27: false,
            atc_code28: false,
            atc_code29: false,
            atc_code3: false,
            atc_code30: false,
            atc_code31: false,
            atc_code32: false,
            atc_code33: false,
            atc_code34: false,
            atc_code35: false,
            atc_code36: false,
            atc_code37: false,
            atc_code38: false,
            atc_code39: false,
            atc_code4: false,
            atc_code40: false,
            atc_code41: false,
            atc_code42: false,
            atc_code43: false,
            atc_code44: false,
            atc_code45: false,
            atc_code46: false,
            atc_code47: false,
            atc_code48: false,
            atc_code49: false,
            atc_code5: false,
            atc_code50: false,
            atc_code51: false,
            atc_code52: false,
            atc_code53: false,
            atc_code54: false,
            atc_code55: false,
            atc_code56: false,
            atc_code57: false,
            atc_code58: false,
            atc_code59: false,
            atc_code6: false,
            atc_code60: false,
            atc_code61: false,
            atc_code62: false,
            atc_code63: false,
            atc_code64: false,
            atc_code65: false,
            atc_code66: false,
            atc_code67: false,
            atc_code68: false,
            atc_code69: false,
            atc_code7: false,
            atc_code70: false,
            atc_code71: false,
            atc_code72: false,
            atc_code73: false,
            atc_code74: false,
            atc_code75: false,
            atc_code76: false,
            atc_code77: false,
            atc_code78: false,
            atc_code79: false,
            atc_code8: false,
            atc_code80: false,
            atc_code81: false,
            atc_code82: false,
            atc_code83: false,
            atc_code84: false,
            atc_code85: false,
            atc_code86: false,
            atc_code87: false,
            atc_code88: false,
            atc_code89: false,
            atc_code9: false,
            atc_code90: false,
            atc_code91: false,
            atc_code92: false,
            atc_code93: false,
            atc_code94: false,
            atc_code95: false,
            atc_code96: false,
            atc_code97: false,
            atc_code98: false,
            atc_code99: false,
            tax_type_code1: false,
            tax_type_code10: false,
            tax_type_code11: false,
            tax_type_code12: false,
            tax_type_code13: false,
            tax_type_code14: false,
            tax_type_code15: false,
            tax_type_code16: false,
            tax_type_code17: false,
            tax_type_code18: false,
            tax_type_code19: false,
            tax_type_code2: false,
            tax_type_code20: false,
            tax_type_code21: false,
            tax_type_code22: false,
            tax_type_code23: false,
            tax_type_code24: false,
            tax_type_code25: false,
            tax_type_code26: false,
            tax_type_code27: false,
            tax_type_code28: false,
            tax_type_code29: false,
            tax_type_code3: false,
            tax_type_code30: false,
            tax_type_code31: false,
            tax_type_code32: false,
            tax_type_code33: false,
            tax_type_code34: false,
            tax_type_code35: false,
            tax_type_code36: false,
            tax_type_code37: false,
            tax_type_code4: false,
            tax_type_code5: false,
            tax_type_code6: false,
            tax_type_code7: false,
            tax_type_code8: false,
            tax_type_code9: true,
            txt_atccode: String::new(),
            txt_tax_type_code: String::new(),
            txt_address: String::new(),
            txt_due_date_day: 0,
            txt_line_bus: String::new(),
            txt_no_of_sheets: 0,
            txt_num_of_installment: 0,
            txt_others_name: String::new(),
            txt_return_period_day: 0,
            txt_tax19: 0.0,
            txt_tax20a: 0.0,
            txt_tax20b: 0.0,
            txt_tax20c: 0.0,
            txt_tax20d: 0.0,
            txt_tax21: 0.0,
            txt_classification_1: false,
            txt_classification_2: true,
            status: FilingStatus::Draft,
            created_at: now.clone(),
            updated_at: now,
            submitted_at: None,
            confirmed_at: None,
            submission_filename: None,
            receipt_id: None,
            submission_attempts: 0,
            next_retry_at: None,
            last_error: None,
        }
    }

    /// Recompute all derived fields per BIR 0605 computation rules.
    ///
    /// Computation flow (per official BIR form instructions):
    /// - Item 20D = 20A + 20B + 20C  (Total Penalties = Surcharge + Interest + Compromise)
    /// - Item 21 = Item 19 + Item 20D (Total Amount Due = Basic Tax + Total Penalties)
    ///
    /// Item 19, 20A, 20B, 20C are user-entered. Items 20D, 21 are derived.
    pub fn recompute(&mut self) {
        // Item 20D: Total Penalties
        self.txt_tax20d = self.txt_tax20a + self.txt_tax20b + self.txt_tax20c;

        // Item 21: Total Amount Due
        self.txt_tax21 = self.txt_tax19 + self.txt_tax20d;

        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    // ── State Transition Methods ──

    pub fn is_editable(&self) -> bool {
        matches!(self.status, FilingStatus::Draft)
    }

    pub fn transition_to_queued(&mut self) -> Result<(), Vec<(String, String)>> {
        assert!(matches!(self.status, FilingStatus::Draft), "Must be Draft");
        let errors = <Self as FormValidator>::validate(self);
        if !errors.is_empty() {
            return Err(errors);
        }
        self.status = FilingStatus::Queued;
        self.submission_attempts = 0;
        self.next_retry_at = Some(chrono::Utc::now().to_rfc3339());
        self.last_error = None;
        self.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(())
    }

    pub fn transition_to_submitted(&mut self, filename: String) {
        assert!(
            matches!(self.status, FilingStatus::Queued),
            "Must be Queued"
        );
        let now = chrono::Utc::now();
        self.status = FilingStatus::Submitted;
        self.submitted_at = Some(now.to_rfc3339());
        self.submission_filename = Some(filename);
        self.submission_attempts = 0;
        self.next_retry_at = None;
        self.last_error = None;
        self.updated_at = now.to_rfc3339();
    }

    pub fn transition_to_confirmed(
        &mut self,
        confirmed_at: String,
        receipt_id: Option<i64>,
        filename: Option<String>,
    ) {
        assert!(
            matches!(self.status, FilingStatus::Submitted),
            "Must be Submitted"
        );
        self.status = FilingStatus::Confirmed;
        self.confirmed_at = Some(confirmed_at);
        self.receipt_id = receipt_id;
        if let Some(f) = filename {
            self.submission_filename = Some(f);
        }
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    pub fn transition_to_paid(&mut self) {
        assert!(
            matches!(self.status, FilingStatus::Confirmed),
            "Must be Confirmed"
        );
        self.status = FilingStatus::Paid;
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    pub fn revert_to_draft(&mut self) {
        assert!(
            !matches!(self.status, FilingStatus::Paid),
            "Cannot revert Paid"
        );
        self.status = FilingStatus::Draft;
        self.submitted_at = None;
        self.confirmed_at = None;
        self.receipt_id = None;
        self.submission_filename = None;
        self.submission_attempts = 0;
        self.next_retry_at = None;
        self.last_error = None;
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    pub fn record_submission_failure(&mut self, error_msg: String) {
        assert!(
            matches!(self.status, FilingStatus::Queued),
            "Must be Queued"
        );
        self.submission_attempts += 1;
        self.last_error = Some(error_msg);
        if self.submission_attempts >= 5 {
            self.status = FilingStatus::Draft;
            self.next_retry_at = None;
        } else {
            let delay = 2i64.pow(self.submission_attempts - 1);
            let next = chrono::Utc::now() + chrono::Duration::minutes(delay);
            self.next_retry_at = Some(next.to_rfc3339());
        }
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }
}
