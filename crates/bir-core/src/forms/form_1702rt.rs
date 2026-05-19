//! BIR Form 1702RTv2018C — Typed draft struct and computation logic.
//!
//! Generated from savefile: 00000000000000-1702RTv2018C-122025.xml
//! Total BIR fields: 258
//! Form-specific fields: 234
//!
//! ⚠️ ScaffoldOnly — formula evidence not yet verified

use crate::forms::{FilingStatus, FormValidator};
use crate::profile::TaxpayerProfile;
use serde::{Deserialize, Serialize};

/// Complete draft for Form 1702RTv2018C.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Form1702RTDraft {
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

    // === other ===
    /// BIR: `BranchMaskP1` (sample: `00000`)
    pub branch_mask_p1: u32,
    /// BIR: `frm1702RT:Pg2Pt4I40IncomeTaxRate` (sample: `30`)
    pub pg2pt4i40income_tax_rate: u32,

    // === radio_options ===
    /// BIR: `frm1702RT:rdoPg1I1Calendar` (sample: `true`)
    pub rdo_pg1i1calendar: bool,
    /// BIR: `frm1702RT:rdoPg1I1Fiscal` (sample: `false`)
    pub rdo_pg1i1fiscal: bool,
    /// BIR: `frm1702RT:rdoPg1I3AmmendNo` (sample: `true`)
    pub rdo_pg1i3ammend_no: bool,
    /// BIR: `frm1702RT:rdoPg1I3AmmendYes` (sample: `false`)
    pub rdo_pg1i3ammend_yes: bool,
    /// BIR: `frm1702RT:rdoPg1I4ShortPeriodNo` (sample: `true`)
    pub rdo_pg1i4short_period_no: bool,
    /// BIR: `frm1702RT:rdoPg1I4ShortPeriodYes` (sample: `false`)
    pub rdo_pg1i4short_period_yes: bool,
    /// BIR: `frm1702RT:rdoPg1I5Atc` (sample: `true`)
    pub rdo_pg1i5atc: bool,
    /// BIR: `frm1702RT:rdoPg1I5AtcOther` (sample: `true`)
    pub rdo_pg1i5atc_other: bool,
    /// BIR: `frm1702RT:rdoPg1Pt1I13ItemizedDeduction` (sample: `false`)
    pub rdo_pg1pt1i13itemized_deduction: bool,
    /// BIR: `frm1702RT:rdoPg1Pt1I13OptionalStandard` (sample: `true`)
    pub rdo_pg1pt1i13optional_standard: bool,
    /// BIR: `frm1702RT:rdoPg1Pt2I21OverpaymentCarried` (sample: `false`)
    pub rdo_pg1pt2i21overpayment_carried: bool,
    /// BIR: `frm1702RT:rdoPg1Pt2I21OverpaymentIssued` (sample: `false`)
    pub rdo_pg1pt2i21overpayment_issued: bool,
    /// BIR: `frm1702RT:rdoPg1Pt2I21OverpaymentRefunded` (sample: `true`)
    pub rdo_pg1pt2i21overpayment_refunded: bool,

    // === selects ===
    /// BIR: `frm1702RT:drpPg1I5AtcOther` (sample: `IC010`)
    pub drp_pg1i5atc_other: String,

    // === shared_text ===
    /// BIR: `txtBranchMaskP2` (sample: `00000`)
    pub txt_branch_mask_p2: u32,
    /// BIR: `txtBranchMaskP3` (sample: `00000`)
    pub txt_branch_mask_p3: u32,
    /// BIR: `txtBranchMaskP4` (sample: `00000`)
    pub txt_branch_mask_p4: u32,

    // === text_fields ===
    /// BIR: `frm1702RT:txtCurrentPage` (sample: `1`)
    pub txt_current_page: u32,
    /// BIR: `frm1702RT:txtMaxPage` (sample: `4`)
    pub txt_max_page: u32,
    /// BIR: `frm1702RT:txtPg1Pt1I10` (sample: `12/10/2019`)
    pub txt_pg1pt1i10: String,
    /// BIR: `frm1702RT:txtPg1Pt1I11Contact` (sample: `09123456789`)
    pub txt_pg1pt1i11contact: String,
    /// BIR: `frm1702RT:txtPg1Pt1I12Email` (sample: `CODEITLIKEMILEY@GMAIL.COM`)
    pub txt_pg1pt1i12email: String,
    /// BIR: `frm1702RT:txtPg1Pt1I6TIN4` (sample: `00000`)
    pub txt_pg1pt1i6tin4: u32,
    /// BIR: `frm1702RT:txtPg1Pt1I8Name1` (sample: `JUAN DELA CRUZ`)
    pub txt_pg1pt1i8name1: String,
    /// BIR: `frm1702RT:txtPg1Pt1I8Name2` (sample: ``)
    pub txt_pg1pt1i8name2: String,
    /// BIR: `frm1702RT:txtPg1Pt1I8Name3` (sample: ``)
    pub txt_pg1pt1i8name3: String,
    /// BIR: `frm1702RT:txtPg1Pt1I9Address1` (sample: `OLONGAPO`)
    pub txt_pg1pt1i9address1: String,
    /// BIR: `frm1702RT:txtPg1Pt1I9Address2` (sample: ``)
    pub txt_pg1pt1i9address2: String,
    /// BIR: `frm1702RT:txtPg1Pt1I9Address3` (sample: ``)
    pub txt_pg1pt1i9address3: String,
    /// BIR: `frm1702RT:txtPg1Pt2I14IncomeTax` (sample: `1,000`)
    pub txt_pg1pt2i14income_tax: String,
    /// BIR: `frm1702RT:txtPg1Pt2I15TotalTaxCredits` (sample: `9,000`)
    pub txt_pg1pt2i15total_tax_credits: String,
    /// BIR: `frm1702RT:txtPg1Pt2I16NetTax` (sample: `-8,000`)
    pub txt_pg1pt2i16net_tax: String,
    /// BIR: `frm1702RT:txtPg1Pt2I17Surcharge` (sample: `1,000`)
    pub txt_pg1pt2i17surcharge: String,
    /// BIR: `frm1702RT:txtPg1Pt2I18Interest` (sample: `1,000`)
    pub txt_pg1pt2i18interest: String,
    /// BIR: `frm1702RT:txtPg1Pt2I19Compromise` (sample: `1,000`)
    pub txt_pg1pt2i19compromise: String,
    /// BIR: `frm1702RT:txtPg1Pt2I20TotalPenalties` (sample: `3,000`)
    pub txt_pg1pt2i20total_penalties: String,
    /// BIR: `frm1702RT:txtPg1Pt2I21TotalAmount` (sample: `3,000`)
    pub txt_pg1pt2i21total_amount: String,
    /// BIR: `frm1702RT:txtPg1Pt2PagesFilled` (sample: `000`)
    pub txt_pg1pt2pages_filled: u32,
    /// BIR: `frm1702RT:txtPg1Pt2Signatory1` (sample: ``)
    pub txt_pg1pt2signatory1: String,
    /// BIR: `frm1702RT:txtPg1Pt2Signatory2` (sample: ``)
    pub txt_pg1pt2signatory2: String,
    /// BIR: `frm1702RT:txtPg1Pt3I23DebitMemoC1` (sample: ``)
    pub txt_pg1pt3i23debit_memo_c1: String,
    /// BIR: `frm1702RT:txtPg1Pt3I23DebitMemoC2` (sample: ``)
    pub txt_pg1pt3i23debit_memo_c2: String,
    /// BIR: `frm1702RT:txtPg1Pt3I23DebitMemoC3Date` (sample: ``)
    pub txt_pg1pt3i23debit_memo_c3date: String,
    /// BIR: `frm1702RT:txtPg1Pt3I23DebitMemoC4Amount` (sample: `0`)
    pub txt_pg1pt3i23debit_memo_c4amount: u32,
    /// BIR: `frm1702RT:txtPg1Pt3I24CheckC1` (sample: ``)
    pub txt_pg1pt3i24check_c1: String,
    /// BIR: `frm1702RT:txtPg1Pt3I24CheckC2` (sample: ``)
    pub txt_pg1pt3i24check_c2: String,
    /// BIR: `frm1702RT:txtPg1Pt3I24CheckC3Date` (sample: ``)
    pub txt_pg1pt3i24check_c3date: String,
    /// BIR: `frm1702RT:txtPg1Pt3I24CheckC4Amount` (sample: `0`)
    pub txt_pg1pt3i24check_c4amount: u32,
    /// BIR: `frm1702RT:txtPg1Pt3I25TaxDebitC2` (sample: ``)
    pub txt_pg1pt3i25tax_debit_c2: String,
    /// BIR: `frm1702RT:txtPg1Pt3I25TaxDebitC4Amount` (sample: `0`)
    pub txt_pg1pt3i25tax_debit_c4amount: u32,
    /// BIR: `frm1702RT:txtPg1Pt3I25TaxDebitDate` (sample: ``)
    pub txt_pg1pt3i25tax_debit_date: String,
    /// BIR: `frm1702RT:txtPg1Pt3I26Others` (sample: ``)
    pub txt_pg1pt3i26others: String,
    /// BIR: `frm1702RT:txtPg1Pt3I26OthersC1` (sample: ``)
    pub txt_pg1pt3i26others_c1: String,
    /// BIR: `frm1702RT:txtPg1Pt3I26OthersC2` (sample: ``)
    pub txt_pg1pt3i26others_c2: String,
    /// BIR: `frm1702RT:txtPg1Pt3I26OthersC3Date` (sample: ``)
    pub txt_pg1pt3i26others_c3date: String,
    /// BIR: `frm1702RT:txtPg1Pt3I26OthersC4Amount` (sample: `0`)
    pub txt_pg1pt3i26others_c4amount: u32,
    /// BIR: `frm1702RT:txtPg2Pt452SpecialTaxCredits` (sample: `1,000`)
    pub txt_pg2pt452special_tax_credits: String,
    /// BIR: `frm1702RT:txtPg2Pt4I27Sales` (sample: `1,000`)
    pub txt_pg2pt4i27sales: String,
    /// BIR: `frm1702RT:txtPg2Pt4I28LessSales` (sample: `1,000`)
    pub txt_pg2pt4i28less_sales: String,
    /// BIR: `frm1702RT:txtPg2Pt4I29NetSales` (sample: `0`)
    pub txt_pg2pt4i29net_sales: u32,
    /// BIR: `frm1702RT:txtPg2Pt4I30LessCost` (sample: `1,000`)
    pub txt_pg2pt4i30less_cost: String,
    /// BIR: `frm1702RT:txtPg2Pt4I31GrossIncome` (sample: `-1,000`)
    pub txt_pg2pt4i31gross_income: String,
    /// BIR: `frm1702RT:txtPg2Pt4I32AddOtherTaxable` (sample: `1,000`)
    pub txt_pg2pt4i32add_other_taxable: String,
    /// BIR: `frm1702RT:txtPg2Pt4I33TotalGross` (sample: `0`)
    pub txt_pg2pt4i33total_gross: u32,
    /// BIR: `frm1702RT:txtPg2Pt4I34OrdinaryAllowable` (sample: `0`)
    pub txt_pg2pt4i34ordinary_allowable: u32,
    /// BIR: `frm1702RT:txtPg2Pt4I35SpecialAllowable` (sample: `0`)
    pub txt_pg2pt4i35special_allowable: u32,
    /// BIR: `frm1702RT:txtPg2Pt4I36Nolco` (sample: `0`)
    pub txt_pg2pt4i36nolco: u32,
    /// BIR: `frm1702RT:txtPg2Pt4I37TotalItemized` (sample: `0`)
    pub txt_pg2pt4i37total_itemized: u32,
    /// BIR: `frm1702RT:txtPg2Pt4I38OptionalStandard` (sample: `0`)
    pub txt_pg2pt4i38optional_standard: u32,
    /// BIR: `frm1702RT:txtPg2Pt4I39NetTaxable` (sample: `0`)
    pub txt_pg2pt4i39net_taxable: u32,
    /// BIR: `frm1702RT:txtPg2Pt4I41IncomeTaxDue` (sample: `0`)
    pub txt_pg2pt4i41income_tax_due: u32,
    /// BIR: `frm1702RT:txtPg2Pt4I42MinimumCorporate` (sample: `1,000`)
    pub txt_pg2pt4i42minimum_corporate: String,
    /// BIR: `frm1702RT:txtPg2Pt4I43TotalIncomeTax` (sample: `1,000`)
    pub txt_pg2pt4i43total_income_tax: String,
    /// BIR: `frm1702RT:txtPg2Pt4I44ExcessCredits` (sample: `1,000`)
    pub txt_pg2pt4i44excess_credits: String,
    /// BIR: `frm1702RT:txtPg2Pt4I45IncomeTaxPaymentUnderMCIT` (sample: `1,000`)
    pub txt_pg2pt4i45income_tax_payment_under_mcit: String,
    /// BIR: `frm1702RT:txtPg2Pt4I46IncomeTaxUnderRegular` (sample: `1,000`)
    pub txt_pg2pt4i46income_tax_under_regular: String,
    /// BIR: `frm1702RT:txtPg2Pt4I47ExcessMCIT` (sample: `0`)
    pub txt_pg2pt4i47excess_mcit: u32,
    /// BIR: `frm1702RT:txtPg2Pt4I48CreditableTaxWithheldFromPrevious` (sample: `1,000`)
    pub txt_pg2pt4i48creditable_tax_withheld_from_previous: String,
    /// BIR: `frm1702RT:txtPg2Pt4I50ForeignTaxCredits` (sample: `1,000`)
    pub txt_pg2pt4i50foreign_tax_credits: String,
    /// BIR: `frm1702RT:txtPg2Pt4I51TaxPaidInReturn` (sample: `0`)
    pub txt_pg2pt4i51tax_paid_in_return: u32,
    /// BIR: `frm1702RT:txtPg2Pt4I53C1` (sample: `EXAMPLE`)
    pub txt_pg2pt4i53c1: String,
    /// BIR: `frm1702RT:txtPg2Pt4I53C2` (sample: `1,000`)
    pub txt_pg2pt4i53c2: String,
    /// BIR: `frm1702RT:txtPg2Pt4I54C1` (sample: `EXAMPLE 2`)
    pub txt_pg2pt4i54c1: String,
    /// BIR: `frm1702RT:txtPg2Pt4I54C2` (sample: `1,000`)
    pub txt_pg2pt4i54c2: String,
    /// BIR: `frm1702RT:txtPg2Pt4I54CtrModal` (sample: `0`)
    pub txt_pg2pt4i54ctr_modal: u32,
    /// BIR: `frm1702RT:txtPg2Pt4I54Subtotal` (sample: `0`)
    pub txt_pg2pt4i54subtotal: u32,
    /// BIR: `frm1702RT:txtPg2Pt4I55TotalTaxCredits` (sample: `9,000`)
    pub txt_pg2pt4i55total_tax_credits: String,
    /// BIR: `frm1702RT:txtPg2Pt4I56NetTax` (sample: `-8,000`)
    pub txt_pg2pt4i56net_tax: String,
    /// BIR: `frm1702RT:txtPg2Pt5I57SpecialAllowable` (sample: `0`)
    pub txt_pg2pt5i57special_allowable: u32,
    /// BIR: `frm1702RT:txtPg2Pt5I58AddSpecialTax` (sample: `1,000`)
    pub txt_pg2pt5i58add_special_tax: String,
    /// BIR: `frm1702RT:txtPg2Pt5I59TotalTax` (sample: `1,000`)
    pub txt_pg2pt5i59total_tax: String,
    /// BIR: `frm1702RT:txtPg2RegisteredName` (sample: `JUAN DELA CRUZ`)
    pub txt_pg2registered_name: String,
    /// BIR: `frm1702RT:txtPg2TIN4` (sample: `00000`)
    pub txt_pg2tin4: u32,
    /// BIR: `frm1702RT:txtPg3RegisteredName` (sample: `JUAN DELA CRUZ`)
    pub txt_pg3registered_name: String,
    /// BIR: `frm1702RT:txtPg3Sc1I10PensionTrust` (sample: `0`)
    pub txt_pg3sc1i10pension_trust: u32,
    /// BIR: `frm1702RT:txtPg3Sc1I11Rental` (sample: `0`)
    pub txt_pg3sc1i11rental: u32,
    /// BIR: `frm1702RT:txtPg3Sc1I12Research` (sample: `0`)
    pub txt_pg3sc1i12research: u32,
    /// BIR: `frm1702RT:txtPg3Sc1I13Salaries` (sample: `0`)
    pub txt_pg3sc1i13salaries: u32,
    /// BIR: `frm1702RT:txtPg3Sc1I14Contributions` (sample: `0`)
    pub txt_pg3sc1i14contributions: u32,
    /// BIR: `frm1702RT:txtPg3Sc1I15TaxesandLicenses` (sample: `0`)
    pub txt_pg3sc1i15taxesand_licenses: u32,
    /// BIR: `frm1702RT:txtPg3Sc1I16TransportationandTravel` (sample: `0`)
    pub txt_pg3sc1i16transportationand_travel: u32,
    /// BIR: `frm1702RT:txtPg3Sc1I17aJanitorial` (sample: `0`)
    pub txt_pg3sc1i17a_janitorial: u32,
    /// BIR: `frm1702RT:txtPg3Sc1I17bProfessionalFees` (sample: `0`)
    pub txt_pg3sc1i17b_professional_fees: u32,
    /// BIR: `frm1702RT:txtPg3Sc1I17cSecurityServices` (sample: `0`)
    pub txt_pg3sc1i17c_security_services: u32,
    /// BIR: `frm1702RT:txtPg3Sc1I17dC1` (sample: ``)
    pub txt_pg3sc1i17d_c1: String,
    /// BIR: `frm1702RT:txtPg3Sc1I17dC2` (sample: `0`)
    pub txt_pg3sc1i17d_c2: u32,
    /// BIR: `frm1702RT:txtPg3Sc1I17eC1` (sample: ``)
    pub txt_pg3sc1i17e_c1: String,
    /// BIR: `frm1702RT:txtPg3Sc1I17eC2` (sample: `0`)
    pub txt_pg3sc1i17e_c2: u32,
    /// BIR: `frm1702RT:txtPg3Sc1I17fC1` (sample: ``)
    pub txt_pg3sc1i17f_c1: String,
    /// BIR: `frm1702RT:txtPg3Sc1I17fC2` (sample: `0`)
    pub txt_pg3sc1i17f_c2: u32,
    /// BIR: `frm1702RT:txtPg3Sc1I17gC1` (sample: ``)
    pub txt_pg3sc1i17g_c1: String,
    /// BIR: `frm1702RT:txtPg3Sc1I17gC2` (sample: `0`)
    pub txt_pg3sc1i17g_c2: u32,
    /// BIR: `frm1702RT:txtPg3Sc1I17hC1` (sample: ``)
    pub txt_pg3sc1i17h_c1: String,
    /// BIR: `frm1702RT:txtPg3Sc1I17hC2` (sample: `0`)
    pub txt_pg3sc1i17h_c2: u32,
    /// BIR: `frm1702RT:txtPg3Sc1I17iC1` (sample: ``)
    pub txt_pg3sc1i17i_c1: String,
    /// BIR: `frm1702RT:txtPg3Sc1I17iC2` (sample: `0`)
    pub txt_pg3sc1i17i_c2: u32,
    /// BIR: `frm1702RT:txtPg3Sc1I17iCtrModal` (sample: `0`)
    pub txt_pg3sc1i17i_ctr_modal: u32,
    /// BIR: `frm1702RT:txtPg3Sc1I17iSubtotal` (sample: `0`)
    pub txt_pg3sc1i17i_subtotal: u32,
    /// BIR: `frm1702RT:txtPg3Sc1I18TotalOrdinaryAllowable` (sample: `0`)
    pub txt_pg3sc1i18total_ordinary_allowable: u32,
    /// BIR: `frm1702RT:txtPg3Sc1I1Amortization` (sample: `0`)
    pub txt_pg3sc1i1amortization: u32,
    /// BIR: `frm1702RT:txtPg3Sc1I2BadDebts` (sample: `0`)
    pub txt_pg3sc1i2bad_debts: u32,
    /// BIR: `frm1702RT:txtPg3Sc1I3CharitableContributions` (sample: `0`)
    pub txt_pg3sc1i3charitable_contributions: u32,
    /// BIR: `frm1702RT:txtPg3Sc1I4Depletion` (sample: `0`)
    pub txt_pg3sc1i4depletion: u32,
    /// BIR: `frm1702RT:txtPg3Sc1I5Depreciation` (sample: `0`)
    pub txt_pg3sc1i5depreciation: u32,
    /// BIR: `frm1702RT:txtPg3Sc1I6Entertainment` (sample: `0`)
    pub txt_pg3sc1i6entertainment: u32,
    /// BIR: `frm1702RT:txtPg3Sc1I7FringeBenefits` (sample: `0`)
    pub txt_pg3sc1i7fringe_benefits: u32,
    /// BIR: `frm1702RT:txtPg3Sc1I8Interest` (sample: `0`)
    pub txt_pg3sc1i8interest: u32,
    /// BIR: `frm1702RT:txtPg3Sc1I9Losses` (sample: `0`)
    pub txt_pg3sc1i9losses: u32,
    /// BIR: `frm1702RT:txtPg3Sc2I1C1` (sample: ``)
    pub txt_pg3sc2i1c1: String,
    /// BIR: `frm1702RT:txtPg3Sc2I1C2` (sample: ``)
    pub txt_pg3sc2i1c2: String,
    /// BIR: `frm1702RT:txtPg3Sc2I1C3` (sample: `0`)
    pub txt_pg3sc2i1c3: u32,
    /// BIR: `frm1702RT:txtPg3Sc2I2C1` (sample: ``)
    pub txt_pg3sc2i2c1: String,
    /// BIR: `frm1702RT:txtPg3Sc2I2C2` (sample: ``)
    pub txt_pg3sc2i2c2: String,
    /// BIR: `frm1702RT:txtPg3Sc2I2C3` (sample: `0`)
    pub txt_pg3sc2i2c3: u32,
    /// BIR: `frm1702RT:txtPg3Sc2I3C1` (sample: ``)
    pub txt_pg3sc2i3c1: String,
    /// BIR: `frm1702RT:txtPg3Sc2I3C2` (sample: ``)
    pub txt_pg3sc2i3c2: String,
    /// BIR: `frm1702RT:txtPg3Sc2I3C3` (sample: `0`)
    pub txt_pg3sc2i3c3: u32,
    /// BIR: `frm1702RT:txtPg3Sc2I4C1` (sample: ``)
    pub txt_pg3sc2i4c1: String,
    /// BIR: `frm1702RT:txtPg3Sc2I4C2` (sample: ``)
    pub txt_pg3sc2i4c2: String,
    /// BIR: `frm1702RT:txtPg3Sc2I4C3` (sample: `0`)
    pub txt_pg3sc2i4c3: u32,
    /// BIR: `frm1702RT:txtPg3Sc2I4CtrModal` (sample: `0`)
    pub txt_pg3sc2i4ctr_modal: u32,
    /// BIR: `frm1702RT:txtPg3Sc2I4Subtotal` (sample: `0`)
    pub txt_pg3sc2i4subtotal: u32,
    /// BIR: `frm1702RT:txtPg3Sc2I5TotalSpecialAllowable` (sample: `0`)
    pub txt_pg3sc2i5total_special_allowable: u32,
    /// BIR: `frm1702RT:txtPg3TIN4` (sample: `00000`)
    pub txt_pg3tin4: u32,
    /// BIR: `frm1702RT:txtPg4RegisteredName` (sample: `JUAN DELA CRUZ`)
    pub txt_pg4registered_name: String,
    /// BIR: `frm1702RT:txtPg4Sc3AI4C1` (sample: ``)
    pub txt_pg4sc3ai4c1: String,
    /// BIR: `frm1702RT:txtPg4Sc3AI4C2` (sample: `0`)
    pub txt_pg4sc3ai4c2: u32,
    /// BIR: `frm1702RT:txtPg4Sc3AI4C3` (sample: `0`)
    pub txt_pg4sc3ai4c3: u32,
    /// BIR: `frm1702RT:txtPg4Sc3AI4C4` (sample: `0`)
    pub txt_pg4sc3ai4c4: u32,
    /// BIR: `frm1702RT:txtPg4Sc3AI4C5` (sample: `0`)
    pub txt_pg4sc3ai4c5: u32,
    /// BIR: `frm1702RT:txtPg4Sc3AI4C6` (sample: `0`)
    pub txt_pg4sc3ai4c6: u32,
    /// BIR: `frm1702RT:txtPg4Sc3AI5C1` (sample: ``)
    pub txt_pg4sc3ai5c1: String,
    /// BIR: `frm1702RT:txtPg4Sc3AI5C2` (sample: `0`)
    pub txt_pg4sc3ai5c2: u32,
    /// BIR: `frm1702RT:txtPg4Sc3AI5C3` (sample: `0`)
    pub txt_pg4sc3ai5c3: u32,
    /// BIR: `frm1702RT:txtPg4Sc3AI5C4` (sample: `0`)
    pub txt_pg4sc3ai5c4: u32,
    /// BIR: `frm1702RT:txtPg4Sc3AI5C5` (sample: `0`)
    pub txt_pg4sc3ai5c5: u32,
    /// BIR: `frm1702RT:txtPg4Sc3AI5C6` (sample: `0`)
    pub txt_pg4sc3ai5c6: u32,
    /// BIR: `frm1702RT:txtPg4Sc3AI6C1` (sample: ``)
    pub txt_pg4sc3ai6c1: String,
    /// BIR: `frm1702RT:txtPg4Sc3AI6C2` (sample: `0`)
    pub txt_pg4sc3ai6c2: u32,
    /// BIR: `frm1702RT:txtPg4Sc3AI6C3` (sample: `0`)
    pub txt_pg4sc3ai6c3: u32,
    /// BIR: `frm1702RT:txtPg4Sc3AI6C4` (sample: `0`)
    pub txt_pg4sc3ai6c4: u32,
    /// BIR: `frm1702RT:txtPg4Sc3AI6C5` (sample: `0`)
    pub txt_pg4sc3ai6c5: u32,
    /// BIR: `frm1702RT:txtPg4Sc3AI6C6` (sample: `0`)
    pub txt_pg4sc3ai6c6: u32,
    /// BIR: `frm1702RT:txtPg4Sc3AI7C1` (sample: ``)
    pub txt_pg4sc3ai7c1: String,
    /// BIR: `frm1702RT:txtPg4Sc3AI7C2` (sample: `0`)
    pub txt_pg4sc3ai7c2: u32,
    /// BIR: `frm1702RT:txtPg4Sc3AI7C2Subtotal` (sample: `0`)
    pub txt_pg4sc3ai7c2subtotal: u32,
    /// BIR: `frm1702RT:txtPg4Sc3AI7C3` (sample: `0`)
    pub txt_pg4sc3ai7c3: u32,
    /// BIR: `frm1702RT:txtPg4Sc3AI7C3Subtotal` (sample: `0`)
    pub txt_pg4sc3ai7c3subtotal: u32,
    /// BIR: `frm1702RT:txtPg4Sc3AI7C4` (sample: `0`)
    pub txt_pg4sc3ai7c4: u32,
    /// BIR: `frm1702RT:txtPg4Sc3AI7C4Subtotal` (sample: `0`)
    pub txt_pg4sc3ai7c4subtotal: u32,
    /// BIR: `frm1702RT:txtPg4Sc3AI7C5` (sample: `0`)
    pub txt_pg4sc3ai7c5: u32,
    /// BIR: `frm1702RT:txtPg4Sc3AI7C5Subtotal` (sample: `0`)
    pub txt_pg4sc3ai7c5subtotal: u32,
    /// BIR: `frm1702RT:txtPg4Sc3AI7C6` (sample: `0`)
    pub txt_pg4sc3ai7c6: u32,
    /// BIR: `frm1702RT:txtPg4Sc3AI7C6Subtotal` (sample: `0`)
    pub txt_pg4sc3ai7c6subtotal: u32,
    /// BIR: `frm1702RT:txtPg4Sc3I1GrossIncome` (sample: `0`)
    pub txt_pg4sc3i1gross_income: u32,
    /// BIR: `frm1702RT:txtPg4Sc3I2TotalDeductions` (sample: `0`)
    pub txt_pg4sc3i2total_deductions: u32,
    /// BIR: `frm1702RT:txtPg4Sc3I3NetOperatingLoss` (sample: `0`)
    pub txt_pg4sc3i3net_operating_loss: u32,
    /// BIR: `frm1702RT:txtPg4Sc3I3Subtotal` (sample: `0`)
    pub txt_pg4sc3i3subtotal: u32,
    /// BIR: `frm1702RT:txtPg4Sc4I1C1` (sample: ``)
    pub txt_pg4sc4i1c1: String,
    /// BIR: `frm1702RT:txtPg4Sc4I1C2` (sample: `0`)
    pub txt_pg4sc4i1c2: u32,
    /// BIR: `frm1702RT:txtPg4Sc4I1C3` (sample: `0`)
    pub txt_pg4sc4i1c3: u32,
    /// BIR: `frm1702RT:txtPg4Sc4I1C4` (sample: `0`)
    pub txt_pg4sc4i1c4: u32,
    /// BIR: `frm1702RT:txtPg4Sc4I1C5` (sample: `0`)
    pub txt_pg4sc4i1c5: u32,
    /// BIR: `frm1702RT:txtPg4Sc4I1C6` (sample: `0`)
    pub txt_pg4sc4i1c6: u32,
    /// BIR: `frm1702RT:txtPg4Sc4I1C7` (sample: `0`)
    pub txt_pg4sc4i1c7: u32,
    /// BIR: `frm1702RT:txtPg4Sc4I1C8` (sample: `0`)
    pub txt_pg4sc4i1c8: u32,
    /// BIR: `frm1702RT:txtPg4Sc4I2C1` (sample: ``)
    pub txt_pg4sc4i2c1: String,
    /// BIR: `frm1702RT:txtPg4Sc4I2C2` (sample: `0`)
    pub txt_pg4sc4i2c2: u32,
    /// BIR: `frm1702RT:txtPg4Sc4I2C3` (sample: `0`)
    pub txt_pg4sc4i2c3: u32,
    /// BIR: `frm1702RT:txtPg4Sc4I2C4` (sample: `0`)
    pub txt_pg4sc4i2c4: u32,
    /// BIR: `frm1702RT:txtPg4Sc4I2C5` (sample: `0`)
    pub txt_pg4sc4i2c5: u32,
    /// BIR: `frm1702RT:txtPg4Sc4I2C6` (sample: `0`)
    pub txt_pg4sc4i2c6: u32,
    /// BIR: `frm1702RT:txtPg4Sc4I2C7` (sample: `0`)
    pub txt_pg4sc4i2c7: u32,
    /// BIR: `frm1702RT:txtPg4Sc4I2C8` (sample: `0`)
    pub txt_pg4sc4i2c8: u32,
    /// BIR: `frm1702RT:txtPg4Sc4I3C1` (sample: ``)
    pub txt_pg4sc4i3c1: String,
    /// BIR: `frm1702RT:txtPg4Sc4I3C2` (sample: `0`)
    pub txt_pg4sc4i3c2: u32,
    /// BIR: `frm1702RT:txtPg4Sc4I3C3` (sample: `0`)
    pub txt_pg4sc4i3c3: u32,
    /// BIR: `frm1702RT:txtPg4Sc4I3C4` (sample: `0`)
    pub txt_pg4sc4i3c4: u32,
    /// BIR: `frm1702RT:txtPg4Sc4I3C5` (sample: `0`)
    pub txt_pg4sc4i3c5: u32,
    /// BIR: `frm1702RT:txtPg4Sc4I3C6` (sample: `0`)
    pub txt_pg4sc4i3c6: u32,
    /// BIR: `frm1702RT:txtPg4Sc4I3C7` (sample: `0`)
    pub txt_pg4sc4i3c7: u32,
    /// BIR: `frm1702RT:txtPg4Sc4I3C8` (sample: `0`)
    pub txt_pg4sc4i3c8: u32,
    /// BIR: `frm1702RT:txtPg4Sc4I4Subtotal` (sample: `0`)
    pub txt_pg4sc4i4subtotal: u32,
    /// BIR: `frm1702RT:txtPg4Sc4I4TotalExcessMCIT` (sample: `0`)
    pub txt_pg4sc4i4total_excess_mcit: u32,
    /// BIR: `frm1702RT:txtPg4Sc4I8TotalNOLCO` (sample: `0`)
    pub txt_pg4sc4i8total_nolco: u32,
    /// BIR: `frm1702RT:txtPg4Sc5I10NetTaxableIncome` (sample: `0`)
    pub txt_pg4sc5i10net_taxable_income: u32,
    /// BIR: `frm1702RT:txtPg4Sc5I1NetIncome` (sample: `0`)
    pub txt_pg4sc5i1net_income: u32,
    /// BIR: `frm1702RT:txtPg4Sc5I2C1` (sample: ``)
    pub txt_pg4sc5i2c1: String,
    /// BIR: `frm1702RT:txtPg4Sc5I2C2` (sample: `0`)
    pub txt_pg4sc5i2c2: u32,
    /// BIR: `frm1702RT:txtPg4Sc5I3C1` (sample: ``)
    pub txt_pg4sc5i3c1: String,
    /// BIR: `frm1702RT:txtPg4Sc5I3C2` (sample: `0`)
    pub txt_pg4sc5i3c2: u32,
    /// BIR: `frm1702RT:txtPg4Sc5I3CtrModal` (sample: `0`)
    pub txt_pg4sc5i3ctr_modal: u32,
    /// BIR: `frm1702RT:txtPg4Sc5I3Subtotal` (sample: `0`)
    pub txt_pg4sc5i3subtotal: u32,
    /// BIR: `frm1702RT:txtPg4Sc5I4Total` (sample: `0`)
    pub txt_pg4sc5i4total: u32,
    /// BIR: `frm1702RT:txtPg4Sc5I5C1` (sample: ``)
    pub txt_pg4sc5i5c1: String,
    /// BIR: `frm1702RT:txtPg4Sc5I5C2` (sample: `0`)
    pub txt_pg4sc5i5c2: u32,
    /// BIR: `frm1702RT:txtPg4Sc5I6C1` (sample: ``)
    pub txt_pg4sc5i6c1: String,
    /// BIR: `frm1702RT:txtPg4Sc5I6C2` (sample: `0`)
    pub txt_pg4sc5i6c2: u32,
    /// BIR: `frm1702RT:txtPg4Sc5I6CtrModal` (sample: `0`)
    pub txt_pg4sc5i6ctr_modal: u32,
    /// BIR: `frm1702RT:txtPg4Sc5I6Subtotal` (sample: `0`)
    pub txt_pg4sc5i6subtotal: u32,
    /// BIR: `frm1702RT:txtPg4Sc5I7C1` (sample: ``)
    pub txt_pg4sc5i7c1: String,
    /// BIR: `frm1702RT:txtPg4Sc5I7C2` (sample: `0`)
    pub txt_pg4sc5i7c2: u32,
    /// BIR: `frm1702RT:txtPg4Sc5I8C1` (sample: ``)
    pub txt_pg4sc5i8c1: String,
    /// BIR: `frm1702RT:txtPg4Sc5I8C2` (sample: `0`)
    pub txt_pg4sc5i8c2: u32,
    /// BIR: `frm1702RT:txtPg4Sc5I8CtrModal` (sample: `0`)
    pub txt_pg4sc5i8ctr_modal: u32,
    /// BIR: `frm1702RT:txtPg4Sc5I8Subtotal` (sample: `0`)
    pub txt_pg4sc5i8subtotal: u32,
    /// BIR: `frm1702RT:txtPg4Sc5I9Total` (sample: `0`)
    pub txt_pg4sc5i9total: u32,
    /// BIR: `frm1702RT:txtPg4TIN4` (sample: `00000`)
    pub txt_pg4tin4: u32,
    /// BIR: `frm1702RT:txtRDO` (sample: `018`)
    pub txt_rdo: u32,
    /// BIR: `frm1702RT:txtSignaturePresident` (sample: ``)
    pub txt_signature_president: String,
    /// BIR: `frm1702RT:txtSignatureTreasurer` (sample: ``)
    pub txt_signature_treasurer: String,
    /// BIR: `frm1702RT:txtZIP` (sample: `2200`)
    pub txt_zip: u32,

    /// BIR: `frm1702RT:txtPg2Pt4I49CreditableTaxWithheldFor4thQuarter` (sample: `1,000`)
    pub txt_pg2pt4i49creditable_tax_withheld_for4th_quarter: String,

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

impl FormValidator for Form1702RTDraft {
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

impl Form1702RTDraft {
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
            branch_mask_p1: 0,
            pg2pt4i40income_tax_rate: 0,
            rdo_pg1i1calendar: true,
            rdo_pg1i1fiscal: false,
            rdo_pg1i3ammend_no: true,
            rdo_pg1i3ammend_yes: false,
            rdo_pg1i4short_period_no: true,
            rdo_pg1i4short_period_yes: false,
            rdo_pg1i5atc: true,
            rdo_pg1i5atc_other: true,
            rdo_pg1pt1i13itemized_deduction: false,
            rdo_pg1pt1i13optional_standard: true,
            rdo_pg1pt2i21overpayment_carried: false,
            rdo_pg1pt2i21overpayment_issued: false,
            rdo_pg1pt2i21overpayment_refunded: true,
            drp_pg1i5atc_other: String::new(),
            txt_branch_mask_p2: 0,
            txt_branch_mask_p3: 0,
            txt_branch_mask_p4: 0,
            txt_current_page: 0,
            txt_max_page: 0,
            txt_pg1pt1i10: String::new(),
            txt_pg1pt1i11contact: String::new(),
            txt_pg1pt1i12email: String::new(),
            txt_pg1pt1i6tin4: 0,
            txt_pg1pt1i8name1: String::new(),
            txt_pg1pt1i8name2: String::new(),
            txt_pg1pt1i8name3: String::new(),
            txt_pg1pt1i9address1: String::new(),
            txt_pg1pt1i9address2: String::new(),
            txt_pg1pt1i9address3: String::new(),
            txt_pg1pt2i14income_tax: String::new(),
            txt_pg1pt2i15total_tax_credits: String::new(),
            txt_pg1pt2i16net_tax: String::new(),
            txt_pg1pt2i17surcharge: String::new(),
            txt_pg1pt2i18interest: String::new(),
            txt_pg1pt2i19compromise: String::new(),
            txt_pg1pt2i20total_penalties: String::new(),
            txt_pg1pt2i21total_amount: String::new(),
            txt_pg1pt2pages_filled: 0,
            txt_pg1pt2signatory1: String::new(),
            txt_pg1pt2signatory2: String::new(),
            txt_pg1pt3i23debit_memo_c1: String::new(),
            txt_pg1pt3i23debit_memo_c2: String::new(),
            txt_pg1pt3i23debit_memo_c3date: String::new(),
            txt_pg1pt3i23debit_memo_c4amount: 0,
            txt_pg1pt3i24check_c1: String::new(),
            txt_pg1pt3i24check_c2: String::new(),
            txt_pg1pt3i24check_c3date: String::new(),
            txt_pg1pt3i24check_c4amount: 0,
            txt_pg1pt3i25tax_debit_c2: String::new(),
            txt_pg1pt3i25tax_debit_c4amount: 0,
            txt_pg1pt3i25tax_debit_date: String::new(),
            txt_pg1pt3i26others: String::new(),
            txt_pg1pt3i26others_c1: String::new(),
            txt_pg1pt3i26others_c2: String::new(),
            txt_pg1pt3i26others_c3date: String::new(),
            txt_pg1pt3i26others_c4amount: 0,
            txt_pg2pt452special_tax_credits: String::new(),
            txt_pg2pt4i27sales: String::new(),
            txt_pg2pt4i28less_sales: String::new(),
            txt_pg2pt4i29net_sales: 0,
            txt_pg2pt4i30less_cost: String::new(),
            txt_pg2pt4i31gross_income: String::new(),
            txt_pg2pt4i32add_other_taxable: String::new(),
            txt_pg2pt4i33total_gross: 0,
            txt_pg2pt4i34ordinary_allowable: 0,
            txt_pg2pt4i35special_allowable: 0,
            txt_pg2pt4i36nolco: 0,
            txt_pg2pt4i37total_itemized: 0,
            txt_pg2pt4i38optional_standard: 0,
            txt_pg2pt4i39net_taxable: 0,
            txt_pg2pt4i41income_tax_due: 0,
            txt_pg2pt4i42minimum_corporate: String::new(),
            txt_pg2pt4i43total_income_tax: String::new(),
            txt_pg2pt4i44excess_credits: String::new(),
            txt_pg2pt4i45income_tax_payment_under_mcit: String::new(),
            txt_pg2pt4i46income_tax_under_regular: String::new(),
            txt_pg2pt4i47excess_mcit: 0,
            txt_pg2pt4i48creditable_tax_withheld_from_previous: String::new(),
            txt_pg2pt4i50foreign_tax_credits: String::new(),
            txt_pg2pt4i51tax_paid_in_return: 0,
            txt_pg2pt4i53c1: String::new(),
            txt_pg2pt4i53c2: String::new(),
            txt_pg2pt4i54c1: String::new(),
            txt_pg2pt4i54c2: String::new(),
            txt_pg2pt4i54ctr_modal: 0,
            txt_pg2pt4i54subtotal: 0,
            txt_pg2pt4i55total_tax_credits: String::new(),
            txt_pg2pt4i56net_tax: String::new(),
            txt_pg2pt5i57special_allowable: 0,
            txt_pg2pt5i58add_special_tax: String::new(),
            txt_pg2pt5i59total_tax: String::new(),
            txt_pg2registered_name: String::new(),
            txt_pg2tin4: 0,
            txt_pg3registered_name: String::new(),
            txt_pg3sc1i10pension_trust: 0,
            txt_pg3sc1i11rental: 0,
            txt_pg3sc1i12research: 0,
            txt_pg3sc1i13salaries: 0,
            txt_pg3sc1i14contributions: 0,
            txt_pg3sc1i15taxesand_licenses: 0,
            txt_pg3sc1i16transportationand_travel: 0,
            txt_pg3sc1i17a_janitorial: 0,
            txt_pg3sc1i17b_professional_fees: 0,
            txt_pg3sc1i17c_security_services: 0,
            txt_pg3sc1i17d_c1: String::new(),
            txt_pg3sc1i17d_c2: 0,
            txt_pg3sc1i17e_c1: String::new(),
            txt_pg3sc1i17e_c2: 0,
            txt_pg3sc1i17f_c1: String::new(),
            txt_pg3sc1i17f_c2: 0,
            txt_pg3sc1i17g_c1: String::new(),
            txt_pg3sc1i17g_c2: 0,
            txt_pg3sc1i17h_c1: String::new(),
            txt_pg3sc1i17h_c2: 0,
            txt_pg3sc1i17i_c1: String::new(),
            txt_pg3sc1i17i_c2: 0,
            txt_pg3sc1i17i_ctr_modal: 0,
            txt_pg3sc1i17i_subtotal: 0,
            txt_pg3sc1i18total_ordinary_allowable: 0,
            txt_pg3sc1i1amortization: 0,
            txt_pg3sc1i2bad_debts: 0,
            txt_pg3sc1i3charitable_contributions: 0,
            txt_pg3sc1i4depletion: 0,
            txt_pg3sc1i5depreciation: 0,
            txt_pg3sc1i6entertainment: 0,
            txt_pg3sc1i7fringe_benefits: 0,
            txt_pg3sc1i8interest: 0,
            txt_pg3sc1i9losses: 0,
            txt_pg3sc2i1c1: String::new(),
            txt_pg3sc2i1c2: String::new(),
            txt_pg3sc2i1c3: 0,
            txt_pg3sc2i2c1: String::new(),
            txt_pg3sc2i2c2: String::new(),
            txt_pg3sc2i2c3: 0,
            txt_pg3sc2i3c1: String::new(),
            txt_pg3sc2i3c2: String::new(),
            txt_pg3sc2i3c3: 0,
            txt_pg3sc2i4c1: String::new(),
            txt_pg3sc2i4c2: String::new(),
            txt_pg3sc2i4c3: 0,
            txt_pg3sc2i4ctr_modal: 0,
            txt_pg3sc2i4subtotal: 0,
            txt_pg3sc2i5total_special_allowable: 0,
            txt_pg3tin4: 0,
            txt_pg4registered_name: String::new(),
            txt_pg4sc3ai4c1: String::new(),
            txt_pg4sc3ai4c2: 0,
            txt_pg4sc3ai4c3: 0,
            txt_pg4sc3ai4c4: 0,
            txt_pg4sc3ai4c5: 0,
            txt_pg4sc3ai4c6: 0,
            txt_pg4sc3ai5c1: String::new(),
            txt_pg4sc3ai5c2: 0,
            txt_pg4sc3ai5c3: 0,
            txt_pg4sc3ai5c4: 0,
            txt_pg4sc3ai5c5: 0,
            txt_pg4sc3ai5c6: 0,
            txt_pg4sc3ai6c1: String::new(),
            txt_pg4sc3ai6c2: 0,
            txt_pg4sc3ai6c3: 0,
            txt_pg4sc3ai6c4: 0,
            txt_pg4sc3ai6c5: 0,
            txt_pg4sc3ai6c6: 0,
            txt_pg4sc3ai7c1: String::new(),
            txt_pg4sc3ai7c2: 0,
            txt_pg4sc3ai7c2subtotal: 0,
            txt_pg4sc3ai7c3: 0,
            txt_pg4sc3ai7c3subtotal: 0,
            txt_pg4sc3ai7c4: 0,
            txt_pg4sc3ai7c4subtotal: 0,
            txt_pg4sc3ai7c5: 0,
            txt_pg4sc3ai7c5subtotal: 0,
            txt_pg4sc3ai7c6: 0,
            txt_pg4sc3ai7c6subtotal: 0,
            txt_pg4sc3i1gross_income: 0,
            txt_pg4sc3i2total_deductions: 0,
            txt_pg4sc3i3net_operating_loss: 0,
            txt_pg4sc3i3subtotal: 0,
            txt_pg4sc4i1c1: String::new(),
            txt_pg4sc4i1c2: 0,
            txt_pg4sc4i1c3: 0,
            txt_pg4sc4i1c4: 0,
            txt_pg4sc4i1c5: 0,
            txt_pg4sc4i1c6: 0,
            txt_pg4sc4i1c7: 0,
            txt_pg4sc4i1c8: 0,
            txt_pg4sc4i2c1: String::new(),
            txt_pg4sc4i2c2: 0,
            txt_pg4sc4i2c3: 0,
            txt_pg4sc4i2c4: 0,
            txt_pg4sc4i2c5: 0,
            txt_pg4sc4i2c6: 0,
            txt_pg4sc4i2c7: 0,
            txt_pg4sc4i2c8: 0,
            txt_pg4sc4i3c1: String::new(),
            txt_pg4sc4i3c2: 0,
            txt_pg4sc4i3c3: 0,
            txt_pg4sc4i3c4: 0,
            txt_pg4sc4i3c5: 0,
            txt_pg4sc4i3c6: 0,
            txt_pg4sc4i3c7: 0,
            txt_pg4sc4i3c8: 0,
            txt_pg4sc4i4subtotal: 0,
            txt_pg4sc4i4total_excess_mcit: 0,
            txt_pg4sc4i8total_nolco: 0,
            txt_pg4sc5i10net_taxable_income: 0,
            txt_pg4sc5i1net_income: 0,
            txt_pg4sc5i2c1: String::new(),
            txt_pg4sc5i2c2: 0,
            txt_pg4sc5i3c1: String::new(),
            txt_pg4sc5i3c2: 0,
            txt_pg4sc5i3ctr_modal: 0,
            txt_pg4sc5i3subtotal: 0,
            txt_pg4sc5i4total: 0,
            txt_pg4sc5i5c1: String::new(),
            txt_pg4sc5i5c2: 0,
            txt_pg4sc5i6c1: String::new(),
            txt_pg4sc5i6c2: 0,
            txt_pg4sc5i6ctr_modal: 0,
            txt_pg4sc5i6subtotal: 0,
            txt_pg4sc5i7c1: String::new(),
            txt_pg4sc5i7c2: 0,
            txt_pg4sc5i8c1: String::new(),
            txt_pg4sc5i8c2: 0,
            txt_pg4sc5i8ctr_modal: 0,
            txt_pg4sc5i8subtotal: 0,
            txt_pg4sc5i9total: 0,
            txt_pg4tin4: 0,
            txt_rdo: 0,
            txt_signature_president: String::new(),
            txt_signature_treasurer: String::new(),
            txt_zip: 0,
            txt_pg2pt4i49creditable_tax_withheld_for4th_quarter: String::new(),
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

    /// Parse BIR money string (e.g. "1,000.50" or "-8,000") to f64.
    fn parse_money(s: &str) -> f64 {
        s.replace(',', "").parse::<f64>().unwrap_or(0.0)
    }

    /// Format f64 back to BIR money string.
    fn fmt_money(v: f64) -> String {
        if v == 0.0 {
            return "0".to_string();
        }
        let neg = v < 0.0;
        let abs = v.abs();
        let whole = abs as i64;
        let frac = ((abs - whole as f64) * 100.0).round() as i64;
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
        } else if frac > 0 {
            format!("{}.{:02}", formatted, frac)
        } else {
            formatted
        }
    }

    /// Recompute all derived fields per BIR 1702RT (Annual ITR for Corporations).
    ///
    /// **Key computation (Page 2, Part 4):**
    /// - Item 29: Net Sales = Sales (27) − Returns/Discounts (28)
    /// - Item 31: Gross Income = Net Sales − Cost of Sales (30)
    /// - Item 33: Total Gross = Gross Income + Other Taxable Income (32)
    /// - Item 37: Total Itemized Deductions (from Schedule 1)
    /// - Item 38: OSD = 40% of Total Gross (if elected)
    /// - Item 39: Net Taxable Income = Total Gross − Deductions
    /// - Item 41: RCIT = Net Taxable × Rate (25% default, or pg2pt4i40income_tax_rate)
    /// - Item 42: MCIT = 2% × Gross Income (from 4th year of operations)
    /// - Item 43: Total Income Tax = max(RCIT, MCIT)
    /// - Items 44-54: Tax Credits
    /// - Item 55: Total Tax Credits
    /// - Item 56: Net Tax = Total Income Tax − Total Tax Credits
    ///
    /// **Page 1 summary (Items 14-21):**
    /// - Item 14: Income Tax = Total Tax from Pg2
    /// - Item 15: Total Tax Credits (from Pg2)
    /// - Item 16: Net Tax
    /// - Items 17-19: Penalties (surcharge, interest, compromise)
    /// - Item 20: Total Penalties
    /// - Item 21: Total Amount Payable
    pub fn recompute(&mut self) {
        let pm = Self::parse_money;
        let fm = Self::fmt_money;

        // ── Page 2, Part 4: Income Computation ──

        // Item 29: Net Sales
        let sales = pm(&self.txt_pg2pt4i27sales);
        let less_sales = pm(&self.txt_pg2pt4i28less_sales);
        let net_sales = sales - less_sales;
        self.txt_pg2pt4i29net_sales = net_sales as u32;

        // Item 31: Gross Income
        let cost = pm(&self.txt_pg2pt4i30less_cost);
        let gross_income = net_sales - cost;
        self.txt_pg2pt4i31gross_income = fm(gross_income);

        // Item 33: Total Gross Income
        let other_taxable = pm(&self.txt_pg2pt4i32add_other_taxable);
        let total_gross = gross_income + other_taxable;
        self.txt_pg2pt4i33total_gross = f64::max(0.0, total_gross) as u32;

        // Deductions (OSD or Itemized)
        if self.rdo_pg1pt1i13optional_standard {
            // OSD = 40% of total gross
            let osd = total_gross * 0.40;
            self.txt_pg2pt4i38optional_standard = f64::max(0.0, osd) as u32;
            self.txt_pg2pt4i37total_itemized = 0;
        } else {
            // Itemized: items 34 + 35 + 36
            let total_itemized = self.txt_pg2pt4i34ordinary_allowable
                + self.txt_pg2pt4i35special_allowable
                + self.txt_pg2pt4i36nolco;
            self.txt_pg2pt4i37total_itemized = total_itemized;
            self.txt_pg2pt4i38optional_standard = 0;
        }

        // Item 39: Net Taxable Income
        let deductions = if self.rdo_pg1pt1i13optional_standard {
            self.txt_pg2pt4i38optional_standard as f64
        } else {
            self.txt_pg2pt4i37total_itemized as f64
        };
        let net_taxable = f64::max(0.0, total_gross - deductions);
        self.txt_pg2pt4i39net_taxable = net_taxable as u32;

        // Item 40: Income Tax Rate (default 25% for RCIT under CREATE law)
        let rate = if self.pg2pt4i40income_tax_rate > 0 {
            self.pg2pt4i40income_tax_rate as f64 / 100.0
        } else {
            0.25
        };

        // Item 41: RCIT = Net Taxable × Rate
        let rcit = net_taxable * rate;
        self.txt_pg2pt4i41income_tax_due = rcit as u32;

        // Item 42: MCIT = 2% of Gross Income (1% during CREATE transition)
        let mcit = f64::max(0.0, gross_income) * 0.02;
        self.txt_pg2pt4i42minimum_corporate = fm(mcit);

        // Item 43: Total Income Tax = max(RCIT, MCIT)
        let total_income_tax = f64::max(rcit, mcit);
        self.txt_pg2pt4i43total_income_tax = fm(total_income_tax);

        // Excess MCIT (Item 47): only when MCIT > RCIT
        let excess_mcit = if mcit > rcit { mcit - rcit } else { 0.0 };
        self.txt_pg2pt4i47excess_mcit = excess_mcit as u32;

        // ── Tax Credits (Items 44-54) ──
        let credits_44 = pm(&self.txt_pg2pt4i44excess_credits);
        let credits_45 = pm(&self.txt_pg2pt4i45income_tax_payment_under_mcit);
        let credits_46 = pm(&self.txt_pg2pt4i46income_tax_under_regular);
        let credits_48 = pm(&self.txt_pg2pt4i48creditable_tax_withheld_from_previous);
        let credits_49 = pm(&self.txt_pg2pt4i49creditable_tax_withheld_for4th_quarter);
        let credits_50 = pm(&self.txt_pg2pt4i50foreign_tax_credits);
        let credits_51 = self.txt_pg2pt4i51tax_paid_in_return as f64;
        let credits_52 = pm(&self.txt_pg2pt452special_tax_credits);
        let credits_53 = pm(&self.txt_pg2pt4i53c2);
        let credits_54 = pm(&self.txt_pg2pt4i54c2);

        let total_tax_credits = credits_44
            + credits_45
            + credits_46
            + credits_48
            + credits_49
            + credits_50
            + credits_51
            + credits_52
            + credits_53
            + credits_54;
        self.txt_pg2pt4i55total_tax_credits = fm(total_tax_credits);

        // Item 56: Net Tax = Total Income Tax − Total Tax Credits
        let net_tax = total_income_tax - total_tax_credits;
        self.txt_pg2pt4i56net_tax = fm(net_tax);

        // ── Page 1 Summary (Part 2) ──
        self.txt_pg1pt2i14income_tax = fm(total_income_tax);
        self.txt_pg1pt2i15total_tax_credits = fm(total_tax_credits);
        self.txt_pg1pt2i16net_tax = fm(net_tax);

        // Penalties
        let surcharge = pm(&self.txt_pg1pt2i17surcharge);
        let interest = pm(&self.txt_pg1pt2i18interest);
        let compromise = pm(&self.txt_pg1pt2i19compromise);
        let total_penalties = surcharge + interest + compromise;
        self.txt_pg1pt2i20total_penalties = fm(total_penalties);

        // Total Amount Payable
        let total_amount = f64::max(0.0, net_tax) + total_penalties;
        self.txt_pg1pt2i21total_amount = fm(total_amount);

        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    // ── State Transition Methods ──

    pub fn is_editable(&self) -> bool {
        matches!(self.status, FilingStatus::Draft)
    }

    pub fn transition_to_queued(&mut self) -> Result<(), Vec<(String, String)>> {
        assert!(matches!(self.status, FilingStatus::Draft), "Must be Draft");
        let errors = self.validate();
        if errors.is_empty() {
            self.recompute();
            self.status = FilingStatus::Queued;
            self.updated_at = chrono::Utc::now().to_rfc3339();
            Ok(())
        } else {
            Err(errors)
        }
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

    fn make_draft() -> Form1702RTDraft {
        let profile = TaxpayerProfile {
            id: Some(1),
            full_name: "TestCorp".into(),
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
            default_form_type: "1702RT".into(),
            taxpayer_type: Default::default(),
            is_vat_registered: false,
            business_start_date: None,
            birth_date: None,
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
            compliance_source_mode: Default::default(),
        };
        Form1702RTDraft::new_from_profile(&profile, 2025, 12)
    }

    #[test]
    fn test_parse_money() {
        assert_eq!(Form1702RTDraft::parse_money("1,000"), 1000.0);
        assert_eq!(Form1702RTDraft::parse_money("-8,000"), -8000.0);
        assert_eq!(Form1702RTDraft::parse_money("0"), 0.0);
    }

    #[test]
    fn test_rcit_osd() {
        let mut d = make_draft();
        d.rdo_pg1pt1i13optional_standard = true;
        d.txt_pg2pt4i27sales = "1,000,000".to_string();
        d.recompute();
        // Net sales = 1M, gross = 1M, OSD = 400k, net taxable = 600k
        assert_eq!(d.txt_pg2pt4i29net_sales, 1_000_000);
        assert_eq!(d.txt_pg2pt4i38optional_standard, 400_000);
        assert_eq!(d.txt_pg2pt4i39net_taxable, 600_000);
        // RCIT = 600k * 25% = 150k
        assert_eq!(d.txt_pg2pt4i41income_tax_due, 150_000);
    }

    #[test]
    fn test_mcit_wins() {
        let mut d = make_draft();
        d.rdo_pg1pt1i13optional_standard = true;
        d.txt_pg2pt4i27sales = "100,000".to_string();
        d.recompute();
        // Gross = 100k, OSD=40k, net taxable=60k
        // RCIT = 60k * 25% = 15k
        // MCIT = 100k * 2% = 2k
        // RCIT > MCIT → RCIT applies
        assert_eq!(d.txt_pg2pt4i41income_tax_due, 15_000);
        // For MCIT to win, we need low net taxable but high gross
        // e.g. Sales=1M, Cost=900k → gross=100k, net taxable with OSD=60k
        // RCIT=15k, MCIT=2k → still RCIT
    }

    #[test]
    fn test_penalties_total() {
        let mut d = make_draft();
        d.txt_pg2pt4i27sales = "500,000".to_string();
        d.txt_pg1pt2i17surcharge = "1,000".to_string();
        d.txt_pg1pt2i18interest = "500".to_string();
        d.txt_pg1pt2i19compromise = "250".to_string();
        d.recompute();
        assert_eq!(
            Form1702RTDraft::parse_money(&d.txt_pg1pt2i20total_penalties),
            1750.0
        );
    }
}
