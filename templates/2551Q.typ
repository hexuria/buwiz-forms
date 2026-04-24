#let data = json("data.json")

#set page(
  paper: "us-legal", // 8.5 x 14 in (612 x 1008 pt)
  background: image("blank_2551Q_0.png", width: 100%, height: 100%),
  margin: 0pt
)

#set text(font: "Courier", size: 10pt, weight: "bold")

#let abs_pos(x, y, content) = place(dx: x * 1pt, dy: y * 1pt)[#content]

#let checkbox(checked) = {
  if checked == "true" {
    "X"
  }
}

// ---------------------------------------------------------
// PART I - Background Information
// ---------------------------------------------------------

// Item 1: For the (Calendar / Fiscal)
#abs_pos(110, 140)[#checkbox(data.at("frm2551Qv2018:forThe_1", default: ""))]
#abs_pos(165, 140)[#checkbox(data.at("frm2551Qv2018:forThe_2", default: ""))]

// Item 2: Year Ended
#abs_pos(50, 160)[#data.at("frm2551Qv2018:rtnMonth", default: "")]
#abs_pos(90, 160)[#data.at("frm2551Qv2018:txtYear", default: "")]

// Item 3: Quarter
#abs_pos(250, 158)[#checkbox(data.at("frm2551Qv2018:qtr_1", default: ""))]
#abs_pos(290, 158)[#checkbox(data.at("frm2551Qv2018:qtr_2", default: ""))]
#abs_pos(330, 158)[#checkbox(data.at("frm2551Qv2018:qtr_3", default: ""))]
#abs_pos(370, 158)[#checkbox(data.at("frm2551Qv2018:qtr_4", default: ""))]

// Item 4: Amended Return
#abs_pos(450, 158)[#checkbox(data.at("frm2551Qv2018:amendedRtn_1", default: ""))]
#abs_pos(490, 158)[#checkbox(data.at("frm2551Qv2018:amendedRtn_2", default: ""))]

// Item 5: No of Sheets
#abs_pos(545, 160)[#data.at("frm2551Qv2018:txtSheets", default: "")]

// Item 6: TIN
#abs_pos(150, 185)[#data.at("frm2551Qv2018:txtTIN1", default: "")]
#abs_pos(210, 185)[#data.at("frm2551Qv2018:txtTIN2", default: "")]
#abs_pos(270, 185)[#data.at("frm2551Qv2018:txtTIN3", default: "")]
#abs_pos(340, 185)[#data.at("frm2551Qv2018:txtBranchCode", default: "")]

// Item 7: RDO
#abs_pos(460, 185)[#data.at("frm2551Qv2018:txtRDOCode", default: "")]

// Item 8: Taxpayer Name
#abs_pos(40, 210)[#data.at("frm2551Qv2018:registeredName", default: "")]

// Item 9: Registered Address
#abs_pos(40, 235)[#data.at("frm2551Qv2018:registeredAddress", default: "")]

// Item 9A: ZIP Code
#abs_pos(460, 235)[#data.at("frm2551Qv2018:zipCode", default: "")]

// Item 10: Contact Number
#abs_pos(140, 260)[#data.at("frm2551Qv2018:telNo", default: "")]

// Item 11: Email Address
#abs_pos(300, 260)[#data.at("txtEmail", default: "")]

// ---------------------------------------------------------
// PART II - Computation of Tax
// ---------------------------------------------------------

// Column dx for amounts: 490
#abs_pos(490, 420)[#data.at("frm2551Qv2018:txt14", default: "")]
#abs_pos(490, 440)[#data.at("frm2551Qv2018:txt15", default: "")]
#abs_pos(490, 460)[#data.at("frm2551Qv2018:txt16", default: "")]
#abs_pos(490, 480)[#data.at("frm2551Qv2018:txt17", default: "")]
#abs_pos(490, 500)[#data.at("frm2551Qv2018:txt18", default: "")]

#abs_pos(490, 520)[#data.at("frm2551Qv2018:txt19", default: "")]

#abs_pos(490, 540)[#data.at("frm2551Qv2018:txt20", default: "")]
#abs_pos(490, 560)[#data.at("frm2551Qv2018:txt21", default: "")]
#abs_pos(490, 580)[#data.at("frm2551Qv2018:txt22", default: "")]

#abs_pos(490, 600)[#data.at("frm2551Qv2018:txt23", default: "")]
#abs_pos(490, 620)[#data.at("frm2551Qv2018:txt24", default: "")]

