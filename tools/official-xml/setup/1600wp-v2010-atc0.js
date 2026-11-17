// changedrpATCList (either category: two ATCs each) draws the Part II ATC
// popup (AtcCd1..2 checkboxes) and the Schedule II popup (SchedIIAtcCde1..2
// radios), both inside frmMain; getATCCode then adds one Part II row per
// ticked ATC (here 0), ids starting at 1.
(function () {
  var part2 = '', sched = '';
  for (var i = 1; i <= 2; i++) {
    part2 += "<tr class='atc'><td class='atc'><input id='AtcCd" + i + "' name='AtcCd" + i + "' type='checkbox' value='' /></td></tr>";
    sched += "<tr class='atc'><td class='atc'><input id='SchedIIAtcCde" + i + "' name='SchedIIAtcCde' type='radio' value='' /></td></tr>";
  }
  d.getElementById('tbllistAtcCode').innerHTML = part2;
  d.getElementById('tbllistSchedIIAtcCode').innerHTML = sched;
  var rows = '';
  for (var n = 1; n <= 0; n++) {
    rows += "<tr><td><input type='text' id='frm1600WP:txtAtcCode" + n + "' value='' disabled /></td>" +
      "<td><input type='text' id='frm1600WP:txtTaxBase" + n + "' maxlength='17' value='0.00' /></td>" +
      "<td><input type='text' id='frm1600WP:txtTaxRate" + n + "' value='' disabled /></td>" +
      "<td><input type='text' id='frm1600WP:txtTaxbeWithHeld" + n + "' maxlength='25' value='0.00' disabled /></td></tr>";
  }
  d.getElementById('tbodyComputeTax').innerHTML = rows;
})();
