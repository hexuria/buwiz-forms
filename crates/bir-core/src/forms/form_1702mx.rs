//! BIR Form 1702MXv2018C — Typed draft struct and computation logic.
//!
//! Generated from savefile: 00000000000000-1702MXv2018C-1225.xml
//! Total BIR fields: 210
//! Form-specific fields: 173
//!
//! ⚠️ ScaffoldOnly — formula evidence not yet verified

use crate::forms::{FilingStatus, FormValidator};
use crate::profile::TaxpayerProfile;
use serde::{Deserialize, Serialize};

/// Complete draft for Form 1702MXv2018C.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Form1702MXDraft {
    /// Database row ID (None before first save)
    pub id: Option<i64>,

    // === Filing Period ===
    pub tin: String,
    pub taxable_year: u16,

    // === Header / Options ===
    pub is_amended: bool,

    // === Profile Fields (pre-filled) ===
    pub rdo_code: String,
    pub taxpayer_name: String,
    pub registered_address: String,
    pub zip_code: String,
    pub contact_number: String,
    pub email: String,

    // === checkboxes ===
    /// BIR: `frm1702MX:chkPg1I5ATCR1` (sample: `true`)
    pub chk_pg1i5atcr1: bool,
    /// BIR: `frm1702MX:chkPg1I5ATCR2` (sample: `true`)
    pub chk_pg1i5atcr2: bool,

    // === other ===
    /// BIR: `frm1702MX:InstAPg2Part4` (sample: `true`)
    pub inst_apg2part4: bool,
    /// BIR: `frm1702MX:InstBPg2Part4` (sample: `false`)
    pub inst_bpg2part4: bool,
    /// BIR: `frm1702MX:ddlPg1I2Date` (sample: `12`)
    pub ddl_pg1i2date: u32,
    /// BIR: `frm1702MX:totalEXAttach` (sample: `0`)
    pub total_exattach: u32,
    /// BIR: `frm1702MX:totalSPAttach` (sample: `0`)
    pub total_spattach: u32,

    // === radio_options ===
    /// BIR: `frm1702MX:rdoPg1AttMExempt` (sample: `false`)
    pub rdo_pg1att_mexempt: bool,
    /// BIR: `frm1702MX:rdoPg1AttMSpecialRate` (sample: `false`)
    pub rdo_pg1att_mspecial_rate: bool,
    /// BIR: `frm1702MX:rdoPg1I1Calendar` (sample: `true`)
    pub rdo_pg1i1calendar: bool,
    /// BIR: `frm1702MX:rdoPg1I1Fiscal` (sample: `false`)
    pub rdo_pg1i1fiscal: bool,
    /// BIR: `frm1702MX:rdoPg1I4ShortPeriodNo` (sample: `true`)
    pub rdo_pg1i4short_period_no: bool,
    /// BIR: `frm1702MX:rdoPg1I4ShortPeriodYes` (sample: `false`)
    pub rdo_pg1i4short_period_yes: bool,
    /// BIR: `frm1702MX:rdoPg1Pt1I13MethodOfDeducItemized` (sample: `false`)
    pub rdo_pg1pt1i13method_of_deduc_itemized: bool,
    /// BIR: `frm1702MX:rdoPg1Pt1I13MethodOfDeducOptional` (sample: `true`)
    pub rdo_pg1pt1i13method_of_deduc_optional: bool,
    /// BIR: `frm1702MX:rdoPg1Pt2I21CarriedOver` (sample: `false`)
    pub rdo_pg1pt2i21carried_over: bool,
    /// BIR: `frm1702MX:rdoPg1Pt2I21IssueTCC` (sample: `false`)
    pub rdo_pg1pt2i21issue_tcc: bool,
    /// BIR: `frm1702MX:rdoPg1Pt2I21Refund` (sample: `false`)
    pub rdo_pg1pt2i21refund: bool,

    // === selects ===
    /// BIR: `frm1702MX:drpPg1I5ATCR2` (sample: `IC010`)
    pub drp_pg1i5atcr2: String,
    /// BIR: `frm1702MX:drpPg1Pt1I7RDO` (sample: `018`)
    pub drp_pg1pt1i7rdo: u32,
    /// BIR: `frm1702MX:drpPg3Sc1I11CB` (sample: `0`)
    pub drp_pg3sc1i11cb: u32,

    // === text_fields ===
    /// BIR: `frm1702MX:txtCtrmodalPg3Sc5I17i` (sample: `0`)
    pub txt_ctrmodal_pg3sc5i17i: u32,
    /// BIR: `frm1702MX:txtCtrmodalPg6Sc6` (sample: `0`)
    pub txt_ctrmodal_pg6sc6: u32,
    /// BIR: `frm1702MX:txtCtrmodalPg7Sc10I3` (sample: `0`)
    pub txt_ctrmodal_pg7sc10i3: u32,
    /// BIR: `frm1702MX:txtCtrmodalPg7Sc10I6` (sample: `0`)
    pub txt_ctrmodal_pg7sc10i6: u32,
    /// BIR: `frm1702MX:txtCtrmodalPg7Sc10I8` (sample: `0`)
    pub txt_ctrmodal_pg7sc10i8: u32,
    /// BIR: `frm1702MX:txtCtrmodalPg7Sc8` (sample: `0`)
    pub txt_ctrmodal_pg7sc8: u32,
    /// BIR: `frm1702MX:txtCurrentPage` (sample: `1`)
    pub txt_current_page: u32,
    /// BIR: `frm1702MX:txtMaxPage` (sample: `4`)
    pub txt_max_page: u32,
    /// BIR: `frm1702MX:txtPg1AttMPt5ScAIt5` (sample: ``)
    pub txt_pg1att_mpt5sc_ait5: String,
    /// BIR: `frm1702MX:txtPg1AttMPt5ScAIt6` (sample: ``)
    pub txt_pg1att_mpt5sc_ait6: String,
    /// BIR: `frm1702MX:txtPg1AttMTIN4` (sample: `00000`)
    pub txt_pg1att_mtin4: u32,
    /// BIR: `frm1702MX:txtPg1AttMTINMASK` (sample: `0000`)
    pub txt_pg1att_mtinmask: u32,
    /// BIR: `frm1702MX:txtPg1P2I23NumOfAttachments` (sample: `00`)
    pub txt_pg1p2i23num_of_attachments: u32,
    /// BIR: `frm1702MX:txtPg1Pt1I11ContactNumber` (sample: `09123456789`)
    pub txt_pg1pt1i11contact_number: String,
    /// BIR: `frm1702MX:txtPg1Pt1I12Email` (sample: `CODEITLIKEMILEY@GMAIL.COM`)
    pub txt_pg1pt1i12email: String,
    /// BIR: `frm1702MX:txtPg1Pt1I26CashC1` (sample: ``)
    pub txt_pg1pt1i26cash_c1: String,
    /// BIR: `frm1702MX:txtPg1Pt1I26CashC2` (sample: ``)
    pub txt_pg1pt1i26cash_c2: String,
    /// BIR: `frm1702MX:txtPg1Pt1I26CashC3` (sample: ``)
    pub txt_pg1pt1i26cash_c3: String,
    /// BIR: `frm1702MX:txtPg1Pt1I27CheckC1` (sample: ``)
    pub txt_pg1pt1i27check_c1: String,
    /// BIR: `frm1702MX:txtPg1Pt1I27CheckC2` (sample: ``)
    pub txt_pg1pt1i27check_c2: String,
    /// BIR: `frm1702MX:txtPg1Pt1I27CheckC3` (sample: ``)
    pub txt_pg1pt1i27check_c3: String,
    /// BIR: `frm1702MX:txtPg1Pt1I28TaxDebitC2` (sample: ``)
    pub txt_pg1pt1i28tax_debit_c2: String,
    /// BIR: `frm1702MX:txtPg1Pt1I28TaxDebitC3` (sample: ``)
    pub txt_pg1pt1i28tax_debit_c3: String,
    /// BIR: `frm1702MX:txtPg1Pt1I29Others` (sample: ``)
    pub txt_pg1pt1i29others: String,
    /// BIR: `frm1702MX:txtPg1Pt1I29OthersC1` (sample: ``)
    pub txt_pg1pt1i29others_c1: String,
    /// BIR: `frm1702MX:txtPg1Pt1I29OthersC2` (sample: ``)
    pub txt_pg1pt1i29others_c2: String,
    /// BIR: `frm1702MX:txtPg1Pt1I29OthersC3` (sample: ``)
    pub txt_pg1pt1i29others_c3: String,
    /// BIR: `frm1702MX:txtPg1Pt1I6TINC1` (sample: `000`)
    pub txt_pg1pt1i6tinc1: u32,
    /// BIR: `frm1702MX:txtPg1Pt1I6TINC2` (sample: `000`)
    pub txt_pg1pt1i6tinc2: u32,
    /// BIR: `frm1702MX:txtPg1Pt1I6TINC3` (sample: `000`)
    pub txt_pg1pt1i6tinc3: u32,
    /// BIR: `frm1702MX:txtPg1Pt1I6TINC4` (sample: `00000`)
    pub txt_pg1pt1i6tinc4: u32,
    /// BIR: `frm1702MX:txtPg1Pt1I7RDO` (sample: `018`)
    pub txt_pg1pt1i7rdo: u32,
    /// BIR: `frm1702MX:txtPg1Pt1I8` (sample: `12/10/2019`)
    pub txt_pg1pt1i8: String,
    /// BIR: `frm1702MX:txtPg1Pt1I9RegisteredName` (sample: `JUAN DELA CRUZ`)
    pub txt_pg1pt1i9registered_name: String,
    /// BIR: `frm1702MX:txtPg1Pt1I9RegisteredName2` (sample: ``)
    pub txt_pg1pt1i9registered_name2: String,
    /// BIR: `frm1702MX:txtPg1Pt1I9RegisteredName3` (sample: ``)
    pub txt_pg1pt1i9registered_name3: String,
    /// BIR: `frm1702MX:txtPg1Pt2AuthorizedRepresentative` (sample: ``)
    pub txt_pg1pt2authorized_representative: String,
    /// BIR: `frm1702MX:txtPg1Pt2I17` (sample: `1000`)
    pub txt_pg1pt2i17: u32,
    /// BIR: `frm1702MX:txtPg1Pt2I18` (sample: `1000`)
    pub txt_pg1pt2i18: u32,
    /// BIR: `frm1702MX:txtPg1Pt2I19` (sample: `1000`)
    pub txt_pg1pt2i19: u32,
    /// BIR: `frm1702MX:txtPg1Pt2I20TotalPenalties` (sample: `3000`)
    pub txt_pg1pt2i20total_penalties: u32,
    /// BIR: `frm1702MX:txtPg1Pt2I21TotalAmount` (sample: `3000`)
    pub txt_pg1pt2i21total_amount: u32,
    /// BIR: `frm1702MX:txtPg1Pt2TINofSignatory` (sample: ``)
    pub txt_pg1pt2tinof_signatory: String,
    /// BIR: `frm1702MX:txtPg1Pt2TINofSignatory2` (sample: ``)
    pub txt_pg1pt2tinof_signatory2: String,
    /// BIR: `frm1702MX:txtPg1Pt2TitleofSignatory` (sample: ``)
    pub txt_pg1pt2titleof_signatory: String,
    /// BIR: `frm1702MX:txtPg1Pt2TitleofSignatory2` (sample: ``)
    pub txt_pg1pt2titleof_signatory2: String,
    /// BIR: `frm1702MX:txtPg1Pt2Treasurer` (sample: ``)
    pub txt_pg1pt2treasurer: String,
    /// BIR: `frm1702MX:txtPg1TINMASK` (sample: `00000`)
    pub txt_pg1tinmask: u32,
    /// BIR: `frm1702MX:txtPg2AttMDesc20` (sample: ``)
    pub txt_pg2att_mdesc20: String,
    /// BIR: `frm1702MX:txtPg2AttMDesc21` (sample: ``)
    pub txt_pg2att_mdesc21: String,
    /// BIR: `frm1702MX:txtPg2AttMDesc22` (sample: ``)
    pub txt_pg2att_mdesc22: String,
    /// BIR: `frm1702MX:txtPg2AttMDesc23` (sample: ``)
    pub txt_pg2att_mdesc23: String,
    /// BIR: `frm1702MX:txtPg2AttMDesc24` (sample: ``)
    pub txt_pg2att_mdesc24: String,
    /// BIR: `frm1702MX:txtPg2AttMScDI17iother` (sample: ``)
    pub txt_pg2att_msc_di17iother: String,
    /// BIR: `frm1702MX:txtPg2AttMScF1I4year` (sample: ``)
    pub txt_pg2att_msc_f1i4year: String,
    /// BIR: `frm1702MX:txtPg2AttMTIN4` (sample: `00000`)
    pub txt_pg2att_mtin4: u32,
    /// BIR: `frm1702MX:txtPg2AttMTINMASK` (sample: `0000`)
    pub txt_pg2att_mtinmask: u32,
    /// BIR: `frm1702MX:txtPg2Pt4I31CA` (sample: ``)
    pub txt_pg2pt4i31ca: String,
    /// BIR: `frm1702MX:txtPg2Pt4I31CB` (sample: ``)
    pub txt_pg2pt4i31cb: String,
    /// BIR: `frm1702MX:txtPg2Pt4I31CC` (sample: ``)
    pub txt_pg2pt4i31cc: String,
    /// BIR: `frm1702MX:txtPg2Pt4I32CA` (sample: ``)
    pub txt_pg2pt4i32ca: String,
    /// BIR: `frm1702MX:txtPg2Pt4I32CB` (sample: ``)
    pub txt_pg2pt4i32cb: String,
    /// BIR: `frm1702MX:txtPg2Pt4I32CC` (sample: ``)
    pub txt_pg2pt4i32cc: String,
    /// BIR: `frm1702MX:txtPg2Pt4I33CA` (sample: ``)
    pub txt_pg2pt4i33ca: String,
    /// BIR: `frm1702MX:txtPg2Pt4I33CB` (sample: ``)
    pub txt_pg2pt4i33cb: String,
    /// BIR: `frm1702MX:txtPg2Pt4I33CC` (sample: ``)
    pub txt_pg2pt4i33cc: String,
    /// BIR: `frm1702MX:txtPg2Pt4I34SpecialTaxRate` (sample: `0.0`)
    pub txt_pg2pt4i34special_tax_rate: f64,
    /// BIR: `frm1702MX:txtPg2Pt4I35CA` (sample: ``)
    pub txt_pg2pt4i35ca: String,
    /// BIR: `frm1702MX:txtPg2Pt4I35CB` (sample: ``)
    pub txt_pg2pt4i35cb: String,
    /// BIR: `frm1702MX:txtPg2Pt4I35CC` (sample: ``)
    pub txt_pg2pt4i35cc: String,
    /// BIR: `frm1702MX:txtPg2Pt4I36CA` (sample: ``)
    pub txt_pg2pt4i36ca: String,
    /// BIR: `frm1702MX:txtPg2Pt4I36CB` (sample: ``)
    pub txt_pg2pt4i36cb: String,
    /// BIR: `frm1702MX:txtPg2Pt4I36CC` (sample: ``)
    pub txt_pg2pt4i36cc: String,
    /// BIR: `frm1702MX:txtPg2RegisteredName` (sample: `JUAN DELA CRUZ`)
    pub txt_pg2registered_name: String,
    /// BIR: `frm1702MX:txtPg2Sc2It14B` (sample: `0.00`)
    pub txt_pg2sc2it14b: f64,
    /// BIR: `frm1702MX:txtPg2Sc2It14C` (sample: `0.00`)
    pub txt_pg2sc2it14c: f64,
    /// BIR: `frm1702MX:txtPg2Sc3It30` (sample: ``)
    pub txt_pg2sc3it30: String,
    /// BIR: `frm1702MX:txtPg2Sc3It31` (sample: ``)
    pub txt_pg2sc3it31: String,
    /// BIR: `frm1702MX:txtPg2TIN4` (sample: `00000`)
    pub txt_pg2tin4: u32,
    /// BIR: `frm1702MX:txtPg2TINMASK` (sample: `00000`)
    pub txt_pg2tinmask: u32,
    /// BIR: `frm1702MX:txtPg3IShed78D` (sample: `0`)
    pub txt_pg3ished78d: u32,
    /// BIR: `frm1702MX:txtPg3IShed7_4A` (sample: `0`)
    pub txt_pg3ished7_4a: u32,
    /// BIR: `frm1702MX:txtPg3IShed7_4B` (sample: `0`)
    pub txt_pg3ished7_4b: u32,
    /// BIR: `frm1702MX:txtPg3IShed7_4C` (sample: `0`)
    pub txt_pg3ished7_4c: u32,
    /// BIR: `frm1702MX:txtPg3IShed7_4D` (sample: `0`)
    pub txt_pg3ished7_4d: u32,
    /// BIR: `frm1702MX:txtPg3IShed7_4E` (sample: `0`)
    pub txt_pg3ished7_4e: u32,
    /// BIR: `frm1702MX:txtPg3IShed7_5A` (sample: `0`)
    pub txt_pg3ished7_5a: u32,
    /// BIR: `frm1702MX:txtPg3IShed7_5B` (sample: `0`)
    pub txt_pg3ished7_5b: u32,
    /// BIR: `frm1702MX:txtPg3IShed7_5C` (sample: `0`)
    pub txt_pg3ished7_5c: u32,
    /// BIR: `frm1702MX:txtPg3IShed7_5D` (sample: `0`)
    pub txt_pg3ished7_5d: u32,
    /// BIR: `frm1702MX:txtPg3IShed7_5E` (sample: `0`)
    pub txt_pg3ished7_5e: u32,
    /// BIR: `frm1702MX:txtPg3IShed7_6A` (sample: `0`)
    pub txt_pg3ished7_6a: u32,
    /// BIR: `frm1702MX:txtPg3IShed7_6B` (sample: `0`)
    pub txt_pg3ished7_6b: u32,
    /// BIR: `frm1702MX:txtPg3IShed7_6C` (sample: `0`)
    pub txt_pg3ished7_6c: u32,
    /// BIR: `frm1702MX:txtPg3IShed7_6D` (sample: `0`)
    pub txt_pg3ished7_6d: u32,
    /// BIR: `frm1702MX:txtPg3IShed7_6E` (sample: `0`)
    pub txt_pg3ished7_6e: u32,
    /// BIR: `frm1702MX:txtPg3IShed7_7A` (sample: `0`)
    pub txt_pg3ished7_7a: u32,
    /// BIR: `frm1702MX:txtPg3IShed7_7B` (sample: `0`)
    pub txt_pg3ished7_7b: u32,
    /// BIR: `frm1702MX:txtPg3IShed7_7C` (sample: `0`)
    pub txt_pg3ished7_7c: u32,
    /// BIR: `frm1702MX:txtPg3IShed7_7D` (sample: `0`)
    pub txt_pg3ished7_7d: u32,
    /// BIR: `frm1702MX:txtPg3IShed7_7E` (sample: `0`)
    pub txt_pg3ished7_7e: u32,
    /// BIR: `frm1702MX:txtPg3IShed88D` (sample: `0`)
    pub txt_pg3ished88d: u32,
    /// BIR: `frm1702MX:txtPg3IShed8_6C` (sample: `0`)
    pub txt_pg3ished8_6c: u32,
    /// BIR: `frm1702MX:txtPg3RegisteredName` (sample: `JUAN DELA CRUZ`)
    pub txt_pg3registered_name: String,
    /// BIR: `frm1702MX:txtPg3Sc5It17d` (sample: ``)
    pub txt_pg3sc5it17d: String,
    /// BIR: `frm1702MX:txtPg3Sc5It17e` (sample: ``)
    pub txt_pg3sc5it17e: String,
    /// BIR: `frm1702MX:txtPg3Sc5It17f` (sample: ``)
    pub txt_pg3sc5it17f: String,
    /// BIR: `frm1702MX:txtPg3Sc5It17g` (sample: ``)
    pub txt_pg3sc5it17g: String,
    /// BIR: `frm1702MX:txtPg3Sc5It17h` (sample: ``)
    pub txt_pg3sc5it17h: String,
    /// BIR: `frm1702MX:txtPg3Sc5It17i` (sample: ``)
    pub txt_pg3sc5it17i: String,
    /// BIR: `frm1702MX:txtPg3Sc6I4description` (sample: ``)
    pub txt_pg3sc6i4description: String,
    /// BIR: `frm1702MX:txtPg3Sc6I4legal` (sample: ``)
    pub txt_pg3sc6i4legal: String,
    /// BIR: `frm1702MX:txtPg3TIN4` (sample: `00000`)
    pub txt_pg3tin4: u32,
    /// BIR: `frm1702MX:txtPg3TINMASK` (sample: `00000`)
    pub txt_pg3tinmask: u32,
    /// BIR: `frm1702MX:txtPg4IShed8_4A` (sample: `0`)
    pub txt_pg4ished8_4a: u32,
    /// BIR: `frm1702MX:txtPg4IShed8_4B` (sample: `0`)
    pub txt_pg4ished8_4b: u32,
    /// BIR: `frm1702MX:txtPg4IShed8_4C` (sample: `0`)
    pub txt_pg4ished8_4c: u32,
    /// BIR: `frm1702MX:txtPg4IShed8_4D` (sample: `0`)
    pub txt_pg4ished8_4d: u32,
    /// BIR: `frm1702MX:txtPg4IShed8_4E` (sample: `0`)
    pub txt_pg4ished8_4e: u32,
    /// BIR: `frm1702MX:txtPg4IShed8_5A` (sample: `0`)
    pub txt_pg4ished8_5a: u32,
    /// BIR: `frm1702MX:txtPg4IShed8_5B` (sample: `0`)
    pub txt_pg4ished8_5b: u32,
    /// BIR: `frm1702MX:txtPg4IShed8_5C` (sample: `0`)
    pub txt_pg4ished8_5c: u32,
    /// BIR: `frm1702MX:txtPg4IShed8_5D` (sample: `0`)
    pub txt_pg4ished8_5d: u32,
    /// BIR: `frm1702MX:txtPg4IShed8_5E` (sample: `0`)
    pub txt_pg4ished8_5e: u32,
    /// BIR: `frm1702MX:txtPg4IShed8_6A` (sample: `0`)
    pub txt_pg4ished8_6a: u32,
    /// BIR: `frm1702MX:txtPg4IShed8_6B` (sample: `0`)
    pub txt_pg4ished8_6b: u32,
    /// BIR: `frm1702MX:txtPg4IShed8_6D` (sample: `0`)
    pub txt_pg4ished8_6d: u32,
    /// BIR: `frm1702MX:txtPg4IShed8_6E` (sample: `0`)
    pub txt_pg4ished8_6e: u32,
    /// BIR: `frm1702MX:txtPg4IShed8_7A` (sample: `0`)
    pub txt_pg4ished8_7a: u32,
    /// BIR: `frm1702MX:txtPg4IShed8_7B` (sample: `0`)
    pub txt_pg4ished8_7b: u32,
    /// BIR: `frm1702MX:txtPg4IShed8_7C` (sample: `0`)
    pub txt_pg4ished8_7c: u32,
    /// BIR: `frm1702MX:txtPg4IShed8_7D` (sample: `0`)
    pub txt_pg4ished8_7d: u32,
    /// BIR: `frm1702MX:txtPg4IShed8_7E` (sample: `0`)
    pub txt_pg4ished8_7e: u32,
    /// BIR: `frm1702MX:txtPg4RegisteredName` (sample: `JUAN DELA CRUZ`)
    pub txt_pg4registered_name: String,
    /// BIR: `frm1702MX:txtPg4Sc10Itm2` (sample: ``)
    pub txt_pg4sc10itm2: String,
    /// BIR: `frm1702MX:txtPg4Sc10Itm3` (sample: ``)
    pub txt_pg4sc10itm3: String,
    /// BIR: `frm1702MX:txtPg4Sc10Itm5` (sample: ``)
    pub txt_pg4sc10itm5: String,
    /// BIR: `frm1702MX:txtPg4Sc10Itm6` (sample: ``)
    pub txt_pg4sc10itm6: String,
    /// BIR: `frm1702MX:txtPg4Sc10Itm7` (sample: ``)
    pub txt_pg4sc10itm7: String,
    /// BIR: `frm1702MX:txtPg4Sc10Itm8` (sample: ``)
    pub txt_pg4sc10itm8: String,
    /// BIR: `frm1702MX:txtPg4TIN4` (sample: `00000`)
    pub txt_pg4tin4: u32,
    /// BIR: `frm1702MX:txtPg4TINMASK` (sample: `00000`)
    pub txt_pg4tinmask: u32,
    /// BIR: `frm1702MX:txtPg6Sc6I1description` (sample: ``)
    pub txt_pg6sc6i1description: String,
    /// BIR: `frm1702MX:txtPg6Sc6I1legal` (sample: ``)
    pub txt_pg6sc6i1legal: String,
    /// BIR: `frm1702MX:txtPg6Sc6I2description` (sample: ``)
    pub txt_pg6sc6i2description: String,
    /// BIR: `frm1702MX:txtPg6Sc6I2legal` (sample: ``)
    pub txt_pg6sc6i2legal: String,
    /// BIR: `frm1702MX:txtPg6Sc6I3description` (sample: ``)
    pub txt_pg6sc6i3description: String,
    /// BIR: `frm1702MX:txtPg6Sc6I3legal` (sample: ``)
    pub txt_pg6sc6i3legal: String,
    /// BIR: `frm1702MX:txtPg7Sc9I1` (sample: ``)
    pub txt_pg7sc9i1: String,
    /// BIR: `frm1702MX:txtPg7Sc9I2` (sample: ``)
    pub txt_pg7sc9i2: String,
    /// BIR: `frm1702MX:txtPg7Sc9I3` (sample: ``)
    pub txt_pg7sc9i3: String,
    /// BIR: `frm1702MX:txtZIP` (sample: `2200`)
    pub txt_zip: u32,

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

impl FormValidator for Form1702MXDraft {
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

impl Form1702MXDraft {
    /// Create a new draft from a taxpayer profile.
    pub fn new_from_profile(profile: &TaxpayerProfile, year: u16) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: None,
            tin: profile.tin.full(),
            taxable_year: year,
            is_amended: false,
            rdo_code: profile.rdo_code.clone(),
            taxpayer_name: profile.full_name.clone(),
            registered_address: profile.registered_address.clone(),
            zip_code: profile.zip_code.clone(),
            contact_number: profile.phone.clone(),
            email: profile.email.clone(),
            chk_pg1i5atcr1: true,
            chk_pg1i5atcr2: true,
            inst_apg2part4: true,
            inst_bpg2part4: false,
            ddl_pg1i2date: 0,
            total_exattach: 0,
            total_spattach: 0,
            rdo_pg1att_mexempt: false,
            rdo_pg1att_mspecial_rate: false,
            rdo_pg1i1calendar: true,
            rdo_pg1i1fiscal: false,
            rdo_pg1i4short_period_no: true,
            rdo_pg1i4short_period_yes: false,
            rdo_pg1pt1i13method_of_deduc_itemized: false,
            rdo_pg1pt1i13method_of_deduc_optional: true,
            rdo_pg1pt2i21carried_over: false,
            rdo_pg1pt2i21issue_tcc: false,
            rdo_pg1pt2i21refund: false,
            drp_pg1i5atcr2: String::new(),
            drp_pg1pt1i7rdo: 0,
            drp_pg3sc1i11cb: 0,
            txt_ctrmodal_pg3sc5i17i: 0,
            txt_ctrmodal_pg6sc6: 0,
            txt_ctrmodal_pg7sc10i3: 0,
            txt_ctrmodal_pg7sc10i6: 0,
            txt_ctrmodal_pg7sc10i8: 0,
            txt_ctrmodal_pg7sc8: 0,
            txt_current_page: 0,
            txt_max_page: 0,
            txt_pg1att_mpt5sc_ait5: String::new(),
            txt_pg1att_mpt5sc_ait6: String::new(),
            txt_pg1att_mtin4: 0,
            txt_pg1att_mtinmask: 0,
            txt_pg1p2i23num_of_attachments: 0,
            txt_pg1pt1i11contact_number: String::new(),
            txt_pg1pt1i12email: String::new(),
            txt_pg1pt1i26cash_c1: String::new(),
            txt_pg1pt1i26cash_c2: String::new(),
            txt_pg1pt1i26cash_c3: String::new(),
            txt_pg1pt1i27check_c1: String::new(),
            txt_pg1pt1i27check_c2: String::new(),
            txt_pg1pt1i27check_c3: String::new(),
            txt_pg1pt1i28tax_debit_c2: String::new(),
            txt_pg1pt1i28tax_debit_c3: String::new(),
            txt_pg1pt1i29others: String::new(),
            txt_pg1pt1i29others_c1: String::new(),
            txt_pg1pt1i29others_c2: String::new(),
            txt_pg1pt1i29others_c3: String::new(),
            txt_pg1pt1i6tinc1: 0,
            txt_pg1pt1i6tinc2: 0,
            txt_pg1pt1i6tinc3: 0,
            txt_pg1pt1i6tinc4: 0,
            txt_pg1pt1i7rdo: 0,
            txt_pg1pt1i8: String::new(),
            txt_pg1pt1i9registered_name: String::new(),
            txt_pg1pt1i9registered_name2: String::new(),
            txt_pg1pt1i9registered_name3: String::new(),
            txt_pg1pt2authorized_representative: String::new(),
            txt_pg1pt2i17: 0,
            txt_pg1pt2i18: 0,
            txt_pg1pt2i19: 0,
            txt_pg1pt2i20total_penalties: 0,
            txt_pg1pt2i21total_amount: 0,
            txt_pg1pt2tinof_signatory: String::new(),
            txt_pg1pt2tinof_signatory2: String::new(),
            txt_pg1pt2titleof_signatory: String::new(),
            txt_pg1pt2titleof_signatory2: String::new(),
            txt_pg1pt2treasurer: String::new(),
            txt_pg1tinmask: 0,
            txt_pg2att_mdesc20: String::new(),
            txt_pg2att_mdesc21: String::new(),
            txt_pg2att_mdesc22: String::new(),
            txt_pg2att_mdesc23: String::new(),
            txt_pg2att_mdesc24: String::new(),
            txt_pg2att_msc_di17iother: String::new(),
            txt_pg2att_msc_f1i4year: String::new(),
            txt_pg2att_mtin4: 0,
            txt_pg2att_mtinmask: 0,
            txt_pg2pt4i31ca: String::new(),
            txt_pg2pt4i31cb: String::new(),
            txt_pg2pt4i31cc: String::new(),
            txt_pg2pt4i32ca: String::new(),
            txt_pg2pt4i32cb: String::new(),
            txt_pg2pt4i32cc: String::new(),
            txt_pg2pt4i33ca: String::new(),
            txt_pg2pt4i33cb: String::new(),
            txt_pg2pt4i33cc: String::new(),
            txt_pg2pt4i34special_tax_rate: 0.0,
            txt_pg2pt4i35ca: String::new(),
            txt_pg2pt4i35cb: String::new(),
            txt_pg2pt4i35cc: String::new(),
            txt_pg2pt4i36ca: String::new(),
            txt_pg2pt4i36cb: String::new(),
            txt_pg2pt4i36cc: String::new(),
            txt_pg2registered_name: String::new(),
            txt_pg2sc2it14b: 0.0,
            txt_pg2sc2it14c: 0.0,
            txt_pg2sc3it30: String::new(),
            txt_pg2sc3it31: String::new(),
            txt_pg2tin4: 0,
            txt_pg2tinmask: 0,
            txt_pg3ished78d: 0,
            txt_pg3ished7_4a: 0,
            txt_pg3ished7_4b: 0,
            txt_pg3ished7_4c: 0,
            txt_pg3ished7_4d: 0,
            txt_pg3ished7_4e: 0,
            txt_pg3ished7_5a: 0,
            txt_pg3ished7_5b: 0,
            txt_pg3ished7_5c: 0,
            txt_pg3ished7_5d: 0,
            txt_pg3ished7_5e: 0,
            txt_pg3ished7_6a: 0,
            txt_pg3ished7_6b: 0,
            txt_pg3ished7_6c: 0,
            txt_pg3ished7_6d: 0,
            txt_pg3ished7_6e: 0,
            txt_pg3ished7_7a: 0,
            txt_pg3ished7_7b: 0,
            txt_pg3ished7_7c: 0,
            txt_pg3ished7_7d: 0,
            txt_pg3ished7_7e: 0,
            txt_pg3ished88d: 0,
            txt_pg3ished8_6c: 0,
            txt_pg3registered_name: String::new(),
            txt_pg3sc5it17d: String::new(),
            txt_pg3sc5it17e: String::new(),
            txt_pg3sc5it17f: String::new(),
            txt_pg3sc5it17g: String::new(),
            txt_pg3sc5it17h: String::new(),
            txt_pg3sc5it17i: String::new(),
            txt_pg3sc6i4description: String::new(),
            txt_pg3sc6i4legal: String::new(),
            txt_pg3tin4: 0,
            txt_pg3tinmask: 0,
            txt_pg4ished8_4a: 0,
            txt_pg4ished8_4b: 0,
            txt_pg4ished8_4c: 0,
            txt_pg4ished8_4d: 0,
            txt_pg4ished8_4e: 0,
            txt_pg4ished8_5a: 0,
            txt_pg4ished8_5b: 0,
            txt_pg4ished8_5c: 0,
            txt_pg4ished8_5d: 0,
            txt_pg4ished8_5e: 0,
            txt_pg4ished8_6a: 0,
            txt_pg4ished8_6b: 0,
            txt_pg4ished8_6d: 0,
            txt_pg4ished8_6e: 0,
            txt_pg4ished8_7a: 0,
            txt_pg4ished8_7b: 0,
            txt_pg4ished8_7c: 0,
            txt_pg4ished8_7d: 0,
            txt_pg4ished8_7e: 0,
            txt_pg4registered_name: String::new(),
            txt_pg4sc10itm2: String::new(),
            txt_pg4sc10itm3: String::new(),
            txt_pg4sc10itm5: String::new(),
            txt_pg4sc10itm6: String::new(),
            txt_pg4sc10itm7: String::new(),
            txt_pg4sc10itm8: String::new(),
            txt_pg4tin4: 0,
            txt_pg4tinmask: 0,
            txt_pg6sc6i1description: String::new(),
            txt_pg6sc6i1legal: String::new(),
            txt_pg6sc6i2description: String::new(),
            txt_pg6sc6i2legal: String::new(),
            txt_pg6sc6i3description: String::new(),
            txt_pg6sc6i3legal: String::new(),
            txt_pg7sc9i1: String::new(),
            txt_pg7sc9i2: String::new(),
            txt_pg7sc9i3: String::new(),
            txt_zip: 0,
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

    /// Parse BIR money string (e.g. "1,000" or "-8,000") to f64.
    fn parse_money(s: &str) -> f64 {
        if s.is_empty() {
            return 0.0;
        }
        s.replace(',', "").parse::<f64>().unwrap_or(0.0)
    }

    /// Format f64 to BIR money string.
    fn fmt_money(v: f64) -> String {
        if v == 0.0 {
            return String::new();
        }
        let neg = v < 0.0;
        let abs = v.abs();
        let whole = abs as i64;
        let s = whole.to_string();
        let mut result = String::new();
        for (i, c) in s.chars().rev().enumerate() {
            if i > 0 && i % 3 == 0 {
                result.push(',');
            }
            result.push(c);
        }
        let formatted: String = result.chars().rev().collect();
        if neg {
            format!("-{}", formatted)
        } else {
            formatted
        }
    }

    /// Recompute all derived fields per BIR 1702MX (Mixed Income Corporations).
    ///
    /// The 1702MX uses a 3-column structure (CA=Regular, CB=Special, CC=Total):
    /// - Item 31: Net Sales/Revenue
    /// - Item 32: Less: Cost of Sales
    /// - Item 33: Gross Income = 31 − 32
    /// - Item 35: Deductions (OSD or Itemized)
    /// - Item 36: Net Taxable Income = 33 − 35
    /// - RCIT (25%) vs MCIT (2%) on Regular column
    /// - Special tax rate on Special column
    /// - Penalties + Total Payable on Page 1
    pub fn recompute(&mut self) {
        let pm = Self::parse_money;
        let fm = Self::fmt_money;

        // ── Part 4: 3-Column Income Computation ──

        // Item 33: Gross Income = Net Sales (31) − Cost of Sales (32)
        let gross_a = pm(&self.txt_pg2pt4i31ca) - pm(&self.txt_pg2pt4i32ca);
        let gross_b = pm(&self.txt_pg2pt4i31cb) - pm(&self.txt_pg2pt4i32cb);
        let gross_c = gross_a + gross_b;
        self.txt_pg2pt4i33ca = fm(gross_a);
        self.txt_pg2pt4i33cb = fm(gross_b);
        self.txt_pg2pt4i33cc = fm(gross_c);

        // Item 35: Deductions
        if self.rdo_pg1pt1i13method_of_deduc_optional {
            // OSD = 40% of gross (Regular column only)
            let osd_a = f64::max(0.0, gross_a) * 0.40;
            self.txt_pg2pt4i35ca = fm(osd_a);
            self.txt_pg2pt4i35cb = String::new(); // No OSD on special
            self.txt_pg2pt4i35cc = fm(osd_a);
        }
        // If itemized, user fills 35CA/35CB directly

        // Item 36: Net Taxable Income = Gross − Deductions
        let ded_a = pm(&self.txt_pg2pt4i35ca);
        let ded_b = pm(&self.txt_pg2pt4i35cb);
        let net_a = f64::max(0.0, gross_a - ded_a);
        let net_b = f64::max(0.0, gross_b - ded_b);
        self.txt_pg2pt4i36ca = fm(net_a);
        self.txt_pg2pt4i36cb = fm(net_b);
        self.txt_pg2pt4i36cc = fm(net_a + net_b);

        // ── Tax Computation ──

        // Regular column: RCIT (25%) vs MCIT (2%)
        let rcit = net_a * 0.25;
        let mcit = f64::max(0.0, gross_a) * 0.02;
        let regular_tax = f64::max(rcit, mcit);

        // Special column: use special tax rate
        let special_rate = if self.txt_pg2pt4i34special_tax_rate > 0.0 {
            self.txt_pg2pt4i34special_tax_rate / 100.0
        } else {
            0.0
        };
        let special_tax = net_b * special_rate;

        // Store in Schedule 2 (Item 14)
        self.txt_pg2sc2it14b = regular_tax;
        self.txt_pg2sc2it14c = special_tax;

        // Total tax
        let total_tax = regular_tax + special_tax;

        // ── Page 1 Summary ──

        // Items 17-19: Penalties (u32, user-entered)
        // Item 20: Total Penalties
        self.txt_pg1pt2i20total_penalties =
            self.txt_pg1pt2i17 + self.txt_pg1pt2i18 + self.txt_pg1pt2i19;

        // Item 21: Total Amount Payable
        self.txt_pg1pt2i21total_amount =
            (f64::max(0.0, total_tax) as u32) + self.txt_pg1pt2i20total_penalties;

        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    // ── State Transition Methods ──

    pub fn is_editable(&self) -> bool {
        matches!(self.status, FilingStatus::Draft)
    }

    pub fn transition_to_queued(&mut self) -> Result<(), Vec<(String, String)>> {
        assert!(matches!(self.status, FilingStatus::Draft), "Must be Draft");
        Err(vec![(
            "support_level".to_string(),
            "Form 1702MX is scaffold-only and cannot be queued for submission yet.".to_string(),
        )])
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::Tin;

    fn make_draft() -> Form1702MXDraft {
        let profile = TaxpayerProfile {
            id: Some(1),
            full_name: "MixedCorp".into(),
            tin: Tin {
                segment1: "000".into(),
                segment2: "000".into(),
                segment3: "000".into(),
                branch: "000".into(),
            },
            rdo_code: "039".into(),
            line_of_business: String::new(),
            registered_address: "Test".into(),
            zip_code: "1100".into(),
            phone: "09170000000".into(),
            email: "t@t.com".into(),
            default_form_type: "1702MX".into(),
            taxpayer_type: Default::default(),
            is_vat_registered: false,
            business_start_date: None,
            tax_classification: None,
            eopt_tier: None,
            is_bmbe: false,
            is_gpp_partner: false,
            is_create_msme: false,
            is_expanded_withholding_agent: false,
            atc_codes: vec![],
            excise_tax_categories: vec![],
            tax_elections: vec![],
            has_employees: false,
            is_dormant: false,
            has_single_employer: false,
            withholds_compensation: false,
            withholds_expanded: false,
            withholds_final: false,
            is_top_withholding_agent: false,
            is_government_withholding_entity: false,
            registration_activity_status: Default::default(),
            is_archived: false,
            profile_pin_hash: None,
            totp_secret: None,
            email_tracking_enabled: false,
            email_auth_method: Default::default(),
            imap_email: None,
            imap_host: None,
            test_notification_enabled: false,
            imap_app_password: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
            profile_versions: vec![],
        };
        Form1702MXDraft::new_from_profile(&profile, 2025)
    }

    #[test]
    fn test_mixed_income_osd() {
        let mut d = make_draft();
        d.rdo_pg1pt1i13method_of_deduc_optional = true;
        d.txt_pg2pt4i31ca = "1,000,000".to_string(); // Regular sales
        d.txt_pg2pt4i31cb = "500,000".to_string(); // Special sales
        d.txt_pg2pt4i32ca = "200,000".to_string(); // Regular cost
        d.txt_pg2pt4i32cb = "100,000".to_string(); // Special cost
        d.txt_pg2pt4i34special_tax_rate = 10.0; // 10% special rate
        d.recompute();

        // Regular: Gross = 800k, OSD = 320k, Net = 480k
        assert_eq!(Form1702MXDraft::parse_money(&d.txt_pg2pt4i33ca), 800_000.0);
        assert_eq!(Form1702MXDraft::parse_money(&d.txt_pg2pt4i35ca), 320_000.0);
        assert_eq!(Form1702MXDraft::parse_money(&d.txt_pg2pt4i36ca), 480_000.0);

        // Special: Gross = 400k, no OSD, Net = 400k
        assert_eq!(Form1702MXDraft::parse_money(&d.txt_pg2pt4i33cb), 400_000.0);
        assert_eq!(Form1702MXDraft::parse_money(&d.txt_pg2pt4i36cb), 400_000.0);

        // RCIT on Regular = 480k * 25% = 120k
        // MCIT on Regular = 800k * 2% = 16k → RCIT wins
        assert_eq!(d.txt_pg2sc2it14b, 120_000.0);

        // Special = 400k * 10% = 40k
        assert_eq!(d.txt_pg2sc2it14c, 40_000.0);
    }

    #[test]
    fn test_penalties_aggregate() {
        let mut d = make_draft();
        d.txt_pg2pt4i31ca = "100,000".to_string();
        d.txt_pg1pt2i17 = 1000; // surcharge
        d.txt_pg1pt2i18 = 500; // interest
        d.txt_pg1pt2i19 = 250; // compromise
        d.recompute();
        assert_eq!(d.txt_pg1pt2i20total_penalties, 1750);
    }

    #[test]
    fn test_total_column_cc() {
        let mut d = make_draft();
        d.rdo_pg1pt1i13method_of_deduc_optional = true;
        d.txt_pg2pt4i31ca = "500,000".to_string();
        d.txt_pg2pt4i31cb = "300,000".to_string();
        d.recompute();
        // Gross CC = 500k + 300k = 800k
        assert_eq!(Form1702MXDraft::parse_money(&d.txt_pg2pt4i33cc), 800_000.0);
    }
}
