// form.typ — BIR Form 2551Q (Typst-native, pixel-perfect)
//
// This is a self-contained Typst document that renders the complete
// BIR Form 2551Q by using the official SVG as a pixel-perfect background
// and overlaying dynamic field values from data.json.
//
// Usage:
//   typst compile --root . form.typ generated.pdf
//
// The data.json file must be present in the same directory and contain
// a flat object mapping BIR field keys to their string values.

#set page(width: 612pt, height: 936pt, margin: 0pt)

// ── Import shared macros ──
#import "template.typ": put, label, cells, mark, amount

// ── Load field data ──
#let data = json("data.json")

// ── Helper: safe field access with default ──
#let field(key, default: "") = data.at(key, default: default)

// ── Helper: check if a field is truthy (checkbox) ──
#let checked(key) = {
  let v = field(key)
  v in ("1", "true", "yes", "y", "x", "True", "Yes", "Y", "X")
}

// ══════════════════════════════════════════════════════════════════════
// PAGE 1
// ══════════════════════════════════════════════════════════════════════
#page(
  background: image("svgbase/page1.svg", width: 612pt, height: 936pt),
  foreground: {

    // ── Part I: Background Information ──

    // For the: Calendar (checkbox) / Fiscal (checkbox)
    if checked("frm2551Qv2018:forThe_1") { mark(87, 108) }
    if checked("frm2551Qv2018:forThe_2") { mark(159, 108) }

    // Year Ended (MMYYYY)
    cells(135, 129, 14.0, field("__year_ended"))

    // Quarter checkboxes
    if checked("frm2551Qv2018:qtr_1") { mark(241, 124) }
    if checked("frm2551Qv2018:qtr_2") { mark(284, 124) }
    if checked("frm2551Qv2018:qtr_3") { mark(327, 124) }
    if checked("frm2551Qv2018:qtr_4") { mark(370, 124) }

    // Amended Return: Yes / No
    if checked("frm2551Qv2018:amendedRtn_1") { mark(425, 124) }
    if checked("frm2551Qv2018:amendedRtn_2") { mark(463, 124) }

    // No. of Sheets Attached
    cells(565, 124, 14.0, field("frm2551Qv2018:txtSheets"))

    // 1 TIN (three groups + branch code)
    cells(220, 164, 14.1, field("frm2551Qv2018:txtTIN1"))
    cells(284, 164, 14.1, field("frm2551Qv2018:txtTIN2"))
    cells(348, 164, 14.1, field("frm2551Qv2018:txtTIN3"))

    // 2 Branch Code
    cells(410, 164, 14.1, field("frm2551Qv2018:txtBranchCode"))

    // 3 RDO Code
    cells(548, 164, 14.1, field("frm2551Qv2018:txtRDOCode"))

    // 4 Taxpayer's Name / Registered Name
    label(33, 186, 8.5, field("frm2551Qv2018:registeredName"))

    // 5 Registered Address
    label(33, 219, 7.5, field("frm2551Qv2018:registeredAddress"))

    // 6 Zip Code
    cells(542, 244, 14.1, field("frm2551Qv2018:zipCode"))

    // 10 Contact Number
    label(33, 278, 8.5, field("frm2551Qv2018:telNo"))

    // 11 Email Address
    label(200, 278, 8.5, field("txtEmail"))

    // 12 Tax Treaty: Yes / No
    if checked("frm2551Qv2018:taxTreaty_1") { mark(195, 292) }
    if checked("frm2551Qv2018:taxTreaty_2") { mark(246, 292) }

    // 13 Tax Rate selection
    if checked("frm2551Qv2018:taxRate1") { mark(178, 334) }
    if checked("frm2551Qv2018:taxRate2") { mark(348, 334) }

    // ── Part II: Total Tax Payable ──

    // 14 Total Tax Due
    amount(384, 376, 14, 11, 553, field("frm2551Qv2018:txt14", default: "0.00"))

    // 15 Creditable Percentage Tax Withheld
    amount(384, 408, 14, 11, 553, field("frm2551Qv2018:txt15", default: "0.00"))

    // 16 Tax Paid in Return Previously Filed
    amount(384, 427, 14, 11, 553, field("frm2551Qv2018:txt16", default: "0.00"))

    // 17 Other Tax Credit/Payment
    amount(384, 446, 14, 11, 553, field("frm2551Qv2018:txt17", default: "0.00"))

    // 18 Total Tax Credits/Payments
    amount(384, 464, 14, 11, 553, field("frm2551Qv2018:txt18", default: "0.00"))

    // 19 Tax Still Payable/(Overpayment)
    amount(384, 482, 14, 11, 553, field("frm2551Qv2018:txt19", default: "0.00"))

    // 20 Surcharge
    amount(384, 519, 14, 11, 553, field("frm2551Qv2018:txt20", default: "0.00"))

    // 21 Interest
    amount(384, 537, 14, 11, 553, field("frm2551Qv2018:txt21", default: "0.00"))

    // 22 Compromise
    amount(384, 556, 14, 11, 553, field("frm2551Qv2018:txt22", default: "0.00"))

    // 23 Total Penalties
    amount(384, 574, 14, 11, 553, field("frm2551Qv2018:txt23", default: "0.00"))

    // 24 TOTAL AMOUNT PAYABLE
    amount(384, 593, 14, 11, 553, field("frm2551Qv2018:txt24", default: "0.00"))
  }
)[]

// ══════════════════════════════════════════════════════════════════════
// PAGE 2
// ══════════════════════════════════════════════════════════════════════
#page(
  background: image("svgbase/page2.svg", width: 612pt, height: 936pt),
  foreground: {

    // ── Header: TIN repeat + Taxpayer Name ──
    cells(25, 113, 14.1, field("frm2551Qv2018:txtPg2TIN1"))
    cells(67, 113, 14.1, field("frm2551Qv2018:txtPg2TIN2"))
    cells(109, 113, 14.1, field("frm2551Qv2018:txtPg2TIN3"))
    cells(151, 113, 14.1, field("frm2551Qv2018:txtPg2BranchCode"))
    label(225, 113, 8.5, field("frm2551Qv2018:txtPg2TaxpayerName"))

    // ── Schedule 1: Computation of Tax ──

    // Row 1
    label(52, 168, 8.5, field("drpATC1"))
    amount(272, 168, 14, 11, 437, field("txtATCAmt1", default: "0.00"))
    label(337, 168, 8.5, field("txtATCRate1"))
    amount(512, 168, 10.5, 7, 586, field("txtATCDue1", default: "0.00"))

    // Row 2
    label(52, 186, 8.5, field("drpATC2", default: ""))
    amount(272, 186, 14, 11, 437, field("txtATCAmt2", default: "0.00"))
    label(337, 186, 8.5, field("txtATCRate2", default: ""))
    amount(512, 186, 10.5, 7, 586, field("txtATCDue2", default: "0.00"))

    // Row 3
    label(52, 204, 8.5, field("drpATC3", default: ""))
    amount(272, 204, 14, 11, 437, field("txtATCAmt3", default: "0.00"))
    label(337, 204, 8.5, field("txtATCRate3", default: ""))
    amount(512, 204, 10.5, 7, 586, field("txtATCDue3", default: "0.00"))

    // Row 4
    label(52, 222, 8.5, field("drpATC4", default: ""))
    amount(272, 222, 14, 11, 437, field("txtATCAmt4", default: "0.00"))
    label(337, 222, 8.5, field("txtATCRate4", default: ""))
    amount(512, 222, 10.5, 7, 586, field("txtATCDue4", default: "0.00"))

    // Row 5
    label(52, 240, 8.5, field("drpATC5", default: ""))
    amount(272, 240, 14, 11, 437, field("txtATCAmt5", default: "0.00"))
    label(337, 240, 8.5, field("txtATCRate5", default: ""))
    amount(512, 240, 10.5, 7, 586, field("txtATCDue5", default: "0.00"))

    // Row 6
    label(52, 258, 8.5, field("drpATC6", default: ""))
    amount(272, 258, 14, 11, 437, field("txtATCAmt6", default: "0.00"))
    label(337, 258, 8.5, field("txtATCRate6", default: ""))
    amount(512, 258, 10.5, 7, 586, field("txtATCDue6", default: "0.00"))

    // 7 Total (Schedule 1)
    amount(512, 284, 10.5, 7, 586, field("txtTotalSched1", default: "0.00"))
  }
)[]
