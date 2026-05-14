//! BIR field mapping for Form 1702MXv2018C.
//!
//! Auto-generated from savefile: 00000000000000-1702MXv2018C-1225.xml
//! Maps Rust struct fields to BIR pseudo-XML field IDs.

use super::form_1702mx::Form1702MXDraft;

use std::collections::BTreeMap;

impl Form1702MXDraft {
    pub fn to_bir_field_map(&self) -> BTreeMap<String, String> {
        let mut fields = BTreeMap::new();
        let (tin1, tin2, tin3, _branch) = split_tin(&self.tin);

        // === Common fields (all forms) ===
        insert(&mut fields, "attachmentCurrent", "0");
        insert(&mut fields, "attachmentTotal", "0");
        insert(&mut fields, "driveSelectTPExport", "0");
        insert(&mut fields, "ebirOnlineConfirmUsername", "");
        insert(&mut fields, "ebirOnlineSecret", "");
        insert(&mut fields, "ebirOnlineUsername", "");
        insert(&mut fields, "txtEnroll", "Y");
        insert(&mut fields, "txtFinalFlag", "1");

        // === Form-specific fields ===
        insert_bool(&mut fields, "frm1702MX:InstAPg2Part4", self.inst_apg2part4);
        insert_bool(&mut fields, "frm1702MX:InstBPg2Part4", self.inst_bpg2part4);
        insert_bool(&mut fields, "frm1702MX:chkPg1I5ATCR1", self.chk_pg1i5atcr1);
        insert_bool(&mut fields, "frm1702MX:chkPg1I5ATCR2", self.chk_pg1i5atcr2);
        insert(
            &mut fields,
            "frm1702MX:ddlPg1I2Date",
            self.ddl_pg1i2date.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:drpPg1I5ATCR2",
            self.drp_pg1i5atcr2.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:drpPg1Pt1I7RDO",
            self.drp_pg1pt1i7rdo.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:drpPg3Sc1I11CB",
            self.drp_pg3sc1i11cb.to_string(),
        );
        insert_bool(
            &mut fields,
            "frm1702MX:rdoPg1AttMExempt",
            self.rdo_pg1att_mexempt,
        );
        insert_bool(
            &mut fields,
            "frm1702MX:rdoPg1AttMSpecialRate",
            self.rdo_pg1att_mspecial_rate,
        );
        insert_bool(
            &mut fields,
            "frm1702MX:rdoPg1I1Calendar",
            self.rdo_pg1i1calendar,
        );
        insert_bool(
            &mut fields,
            "frm1702MX:rdoPg1I1Fiscal",
            self.rdo_pg1i1fiscal,
        );
        insert_bool(&mut fields, "frm1702MX:rdoPg1I3AmendedNo", self.is_amended);
        insert_bool(&mut fields, "frm1702MX:rdoPg1I3AmendedYes", self.is_amended);
        insert_bool(
            &mut fields,
            "frm1702MX:rdoPg1I4ShortPeriodNo",
            self.rdo_pg1i4short_period_no,
        );
        insert_bool(
            &mut fields,
            "frm1702MX:rdoPg1I4ShortPeriodYes",
            self.rdo_pg1i4short_period_yes,
        );
        insert_bool(
            &mut fields,
            "frm1702MX:rdoPg1Pt1I13MethodOfDeducItemized",
            self.rdo_pg1pt1i13method_of_deduc_itemized,
        );
        insert_bool(
            &mut fields,
            "frm1702MX:rdoPg1Pt1I13MethodOfDeducOptional",
            self.rdo_pg1pt1i13method_of_deduc_optional,
        );
        insert_bool(
            &mut fields,
            "frm1702MX:rdoPg1Pt2I21CarriedOver",
            self.rdo_pg1pt2i21carried_over,
        );
        insert_bool(
            &mut fields,
            "frm1702MX:rdoPg1Pt2I21IssueTCC",
            self.rdo_pg1pt2i21issue_tcc,
        );
        insert_bool(
            &mut fields,
            "frm1702MX:rdoPg1Pt2I21Refund",
            self.rdo_pg1pt2i21refund,
        );
        insert(
            &mut fields,
            "frm1702MX:totalEXAttach",
            self.total_exattach.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:totalSPAttach",
            self.total_spattach.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtCtrmodalPg3Sc5I17i",
            self.txt_ctrmodal_pg3sc5i17i.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtCtrmodalPg6Sc6",
            self.txt_ctrmodal_pg6sc6.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtCtrmodalPg7Sc10I3",
            self.txt_ctrmodal_pg7sc10i3.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtCtrmodalPg7Sc10I6",
            self.txt_ctrmodal_pg7sc10i6.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtCtrmodalPg7Sc10I8",
            self.txt_ctrmodal_pg7sc10i8.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtCtrmodalPg7Sc8",
            self.txt_ctrmodal_pg7sc8.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtCurrentPage",
            self.txt_current_page.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtMaxPage",
            self.txt_max_page.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1AttMPt5ScAIt5",
            self.txt_pg1att_mpt5sc_ait5.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1AttMPt5ScAIt6",
            self.txt_pg1att_mpt5sc_ait6.clone(),
        );
        insert(&mut fields, "frm1702MX:txtPg1AttMTIN1", tin1.clone());
        insert(&mut fields, "frm1702MX:txtPg1AttMTIN2", tin2.clone());
        insert(&mut fields, "frm1702MX:txtPg1AttMTIN3", tin3.clone());
        insert(
            &mut fields,
            "frm1702MX:txtPg1AttMTIN4",
            self.txt_pg1att_mtin4.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1AttMTINMASK",
            self.txt_pg1att_mtinmask.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1I2YearEnd",
            self.taxable_year.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1P2I23NumOfAttachments",
            self.txt_pg1p2i23num_of_attachments.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt1I10RegisteredAddress",
            self.registered_address.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt1I10RegisteredAddress2",
            self.registered_address.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt1I10RegisteredAddress3",
            self.registered_address.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt1I11ContactNumber",
            self.txt_pg1pt1i11contact_number.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt1I12Email",
            self.txt_pg1pt1i12email.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt1I26CashC1",
            self.txt_pg1pt1i26cash_c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt1I26CashC2",
            self.txt_pg1pt1i26cash_c2.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt1I26CashC3",
            self.txt_pg1pt1i26cash_c3.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt1I27CheckC1",
            self.txt_pg1pt1i27check_c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt1I27CheckC2",
            self.txt_pg1pt1i27check_c2.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt1I27CheckC3",
            self.txt_pg1pt1i27check_c3.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt1I28TaxDebitC2",
            self.txt_pg1pt1i28tax_debit_c2.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt1I28TaxDebitC3",
            self.txt_pg1pt1i28tax_debit_c3.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt1I29Others",
            self.txt_pg1pt1i29others.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt1I29OthersC1",
            self.txt_pg1pt1i29others_c1.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt1I29OthersC2",
            self.txt_pg1pt1i29others_c2.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt1I29OthersC3",
            self.txt_pg1pt1i29others_c3.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt1I6TINC1",
            self.txt_pg1pt1i6tinc1.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt1I6TINC2",
            self.txt_pg1pt1i6tinc2.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt1I6TINC3",
            self.txt_pg1pt1i6tinc3.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt1I6TINC4",
            self.txt_pg1pt1i6tinc4.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt1I7RDO",
            self.txt_pg1pt1i7rdo.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt1I8",
            self.txt_pg1pt1i8.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt1I9RegisteredName",
            self.txt_pg1pt1i9registered_name.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt1I9RegisteredName2",
            self.txt_pg1pt1i9registered_name2.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt1I9RegisteredName3",
            self.txt_pg1pt1i9registered_name3.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt2AuthorizedRepresentative",
            self.txt_pg1pt2authorized_representative.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt2I17",
            self.txt_pg1pt2i17.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt2I18",
            self.txt_pg1pt2i18.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt2I19",
            self.txt_pg1pt2i19.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt2I20TotalPenalties",
            self.txt_pg1pt2i20total_penalties.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt2I21TotalAmount",
            self.txt_pg1pt2i21total_amount.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt2TINofSignatory",
            self.txt_pg1pt2tinof_signatory.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt2TINofSignatory2",
            self.txt_pg1pt2tinof_signatory2.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt2TitleofSignatory",
            self.txt_pg1pt2titleof_signatory.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt2TitleofSignatory2",
            self.txt_pg1pt2titleof_signatory2.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1Pt2Treasurer",
            self.txt_pg1pt2treasurer.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg1TINMASK",
            self.txt_pg1tinmask.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg2AttMDesc20",
            self.txt_pg2att_mdesc20.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg2AttMDesc21",
            self.txt_pg2att_mdesc21.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg2AttMDesc22",
            self.txt_pg2att_mdesc22.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg2AttMDesc23",
            self.txt_pg2att_mdesc23.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg2AttMDesc24",
            self.txt_pg2att_mdesc24.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg2AttMScDI17iother",
            self.txt_pg2att_msc_di17iother.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg2AttMScF1I4year",
            self.txt_pg2att_msc_f1i4year.clone(),
        );
        insert(&mut fields, "frm1702MX:txtPg2AttMTIN1", tin1.clone());
        insert(&mut fields, "frm1702MX:txtPg2AttMTIN2", tin2.clone());
        insert(&mut fields, "frm1702MX:txtPg2AttMTIN3", tin3.clone());
        insert(
            &mut fields,
            "frm1702MX:txtPg2AttMTIN4",
            self.txt_pg2att_mtin4.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg2AttMTINMASK",
            self.txt_pg2att_mtinmask.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg2Pt4I31CA",
            self.txt_pg2pt4i31ca.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg2Pt4I31CB",
            self.txt_pg2pt4i31cb.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg2Pt4I31CC",
            self.txt_pg2pt4i31cc.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg2Pt4I32CA",
            self.txt_pg2pt4i32ca.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg2Pt4I32CB",
            self.txt_pg2pt4i32cb.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg2Pt4I32CC",
            self.txt_pg2pt4i32cc.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg2Pt4I33CA",
            self.txt_pg2pt4i33ca.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg2Pt4I33CB",
            self.txt_pg2pt4i33cb.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg2Pt4I33CC",
            self.txt_pg2pt4i33cc.clone(),
        );
        insert_money(
            &mut fields,
            "frm1702MX:txtPg2Pt4I34SpecialTaxRate",
            self.txt_pg2pt4i34special_tax_rate,
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg2Pt4I35CA",
            self.txt_pg2pt4i35ca.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg2Pt4I35CB",
            self.txt_pg2pt4i35cb.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg2Pt4I35CC",
            self.txt_pg2pt4i35cc.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg2Pt4I36CA",
            self.txt_pg2pt4i36ca.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg2Pt4I36CB",
            self.txt_pg2pt4i36cb.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg2Pt4I36CC",
            self.txt_pg2pt4i36cc.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg2RegisteredName",
            self.txt_pg2registered_name.clone(),
        );
        insert_money(
            &mut fields,
            "frm1702MX:txtPg2Sc2It14B",
            self.txt_pg2sc2it14b,
        );
        insert_money(
            &mut fields,
            "frm1702MX:txtPg2Sc2It14C",
            self.txt_pg2sc2it14c,
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg2Sc3It30",
            self.txt_pg2sc3it30.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg2Sc3It31",
            self.txt_pg2sc3it31.clone(),
        );
        insert(&mut fields, "frm1702MX:txtPg2TIN1", tin1.clone());
        insert(&mut fields, "frm1702MX:txtPg2TIN2", tin2.clone());
        insert(&mut fields, "frm1702MX:txtPg2TIN3", tin3.clone());
        insert(
            &mut fields,
            "frm1702MX:txtPg2TIN4",
            self.txt_pg2tin4.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg2TINMASK",
            self.txt_pg2tinmask.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed78D",
            self.txt_pg3ished78d.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed7_4A",
            self.txt_pg3ished7_4a.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed7_4B",
            self.txt_pg3ished7_4b.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed7_4C",
            self.txt_pg3ished7_4c.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed7_4D",
            self.txt_pg3ished7_4d.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed7_4E",
            self.txt_pg3ished7_4e.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed7_4Year",
            self.taxable_year.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed7_5A",
            self.txt_pg3ished7_5a.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed7_5B",
            self.txt_pg3ished7_5b.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed7_5C",
            self.txt_pg3ished7_5c.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed7_5D",
            self.txt_pg3ished7_5d.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed7_5E",
            self.txt_pg3ished7_5e.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed7_5Year",
            self.taxable_year.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed7_6A",
            self.txt_pg3ished7_6a.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed7_6B",
            self.txt_pg3ished7_6b.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed7_6C",
            self.txt_pg3ished7_6c.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed7_6D",
            self.txt_pg3ished7_6d.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed7_6E",
            self.txt_pg3ished7_6e.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed7_6Year",
            self.taxable_year.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed7_7A",
            self.txt_pg3ished7_7a.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed7_7B",
            self.txt_pg3ished7_7b.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed7_7C",
            self.txt_pg3ished7_7c.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed7_7D",
            self.txt_pg3ished7_7d.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed7_7E",
            self.txt_pg3ished7_7e.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed7_7Year",
            self.taxable_year.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed88D",
            self.txt_pg3ished88d.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed8_4Year",
            self.taxable_year.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed8_5Year",
            self.taxable_year.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed8_6C",
            self.txt_pg3ished8_6c.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3IShed8_6Year",
            self.taxable_year.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3RegisteredName",
            self.txt_pg3registered_name.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3Sc5It17d",
            self.txt_pg3sc5it17d.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3Sc5It17e",
            self.txt_pg3sc5it17e.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3Sc5It17f",
            self.txt_pg3sc5it17f.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3Sc5It17g",
            self.txt_pg3sc5it17g.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3Sc5It17h",
            self.txt_pg3sc5it17h.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3Sc5It17i",
            self.txt_pg3sc5it17i.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3Sc6I4description",
            self.txt_pg3sc6i4description.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3Sc6I4legal",
            self.txt_pg3sc6i4legal.clone(),
        );
        insert(&mut fields, "frm1702MX:txtPg3TIN1", tin1.clone());
        insert(&mut fields, "frm1702MX:txtPg3TIN2", tin2.clone());
        insert(&mut fields, "frm1702MX:txtPg3TIN3", tin3.clone());
        insert(
            &mut fields,
            "frm1702MX:txtPg3TIN4",
            self.txt_pg3tin4.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg3TINMASK",
            self.txt_pg3tinmask.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg4IShed8_4A",
            self.txt_pg4ished8_4a.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg4IShed8_4B",
            self.txt_pg4ished8_4b.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg4IShed8_4C",
            self.txt_pg4ished8_4c.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg4IShed8_4D",
            self.txt_pg4ished8_4d.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg4IShed8_4E",
            self.txt_pg4ished8_4e.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg4IShed8_5A",
            self.txt_pg4ished8_5a.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg4IShed8_5B",
            self.txt_pg4ished8_5b.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg4IShed8_5C",
            self.txt_pg4ished8_5c.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg4IShed8_5D",
            self.txt_pg4ished8_5d.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg4IShed8_5E",
            self.txt_pg4ished8_5e.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg4IShed8_6A",
            self.txt_pg4ished8_6a.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg4IShed8_6B",
            self.txt_pg4ished8_6b.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg4IShed8_6D",
            self.txt_pg4ished8_6d.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg4IShed8_6E",
            self.txt_pg4ished8_6e.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg4IShed8_7A",
            self.txt_pg4ished8_7a.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg4IShed8_7B",
            self.txt_pg4ished8_7b.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg4IShed8_7C",
            self.txt_pg4ished8_7c.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg4IShed8_7D",
            self.txt_pg4ished8_7d.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg4IShed8_7E",
            self.txt_pg4ished8_7e.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg4IShed8_7Year",
            self.taxable_year.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg4RegisteredName",
            self.txt_pg4registered_name.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg4Sc10Itm2",
            self.txt_pg4sc10itm2.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg4Sc10Itm3",
            self.txt_pg4sc10itm3.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg4Sc10Itm5",
            self.txt_pg4sc10itm5.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg4Sc10Itm6",
            self.txt_pg4sc10itm6.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg4Sc10Itm7",
            self.txt_pg4sc10itm7.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg4Sc10Itm8",
            self.txt_pg4sc10itm8.clone(),
        );
        insert(&mut fields, "frm1702MX:txtPg4TIN1", tin1.clone());
        insert(&mut fields, "frm1702MX:txtPg4TIN2", tin2.clone());
        insert(&mut fields, "frm1702MX:txtPg4TIN3", tin3.clone());
        insert(
            &mut fields,
            "frm1702MX:txtPg4TIN4",
            self.txt_pg4tin4.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg4TINMASK",
            self.txt_pg4tinmask.to_string(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg6Sc6I1description",
            self.txt_pg6sc6i1description.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg6Sc6I1legal",
            self.txt_pg6sc6i1legal.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg6Sc6I2description",
            self.txt_pg6sc6i2description.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg6Sc6I2legal",
            self.txt_pg6sc6i2legal.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg6Sc6I3description",
            self.txt_pg6sc6i3description.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg6Sc6I3legal",
            self.txt_pg6sc6i3legal.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg7Sc9I1",
            self.txt_pg7sc9i1.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg7Sc9I2",
            self.txt_pg7sc9i2.clone(),
        );
        insert(
            &mut fields,
            "frm1702MX:txtPg7Sc9I3",
            self.txt_pg7sc9i3.clone(),
        );
        insert(&mut fields, "frm1702MX:txtZIP", self.txt_zip.to_string());

        fields
    }

    pub fn to_bir_xml_payload(&self) -> String {
        crate::bir_xml::generate_bir_xml(&self.to_bir_field_map())
    }
}

fn split_tin(tin: &str) -> (String, String, String, String) {
    let digits: String = tin.chars().filter(|ch| ch.is_ascii_digit()).collect();
    let segment = |start: usize, end: usize| -> String {
        digits
            .get(start..end.min(digits.len()))
            .unwrap_or("")
            .to_string()
    };
    (
        segment(0, 3),
        segment(3, 6),
        segment(6, 9),
        digits
            .get(9..)
            .filter(|s| !s.is_empty())
            .unwrap_or("000")
            .to_string(),
    )
}

fn insert(map: &mut BTreeMap<String, String>, key: &str, value: impl Into<String>) {
    map.insert(key.to_string(), value.into());
}

fn insert_bool(map: &mut BTreeMap<String, String>, key: &str, value: bool) {
    insert(map, key, if value { "true" } else { "false" });
}

fn insert_money(map: &mut BTreeMap<String, String>, key: &str, value: f64) {
    insert(map, key, format!("{:.2}", value));
}
