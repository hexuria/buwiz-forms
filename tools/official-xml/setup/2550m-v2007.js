// init() runs ATCList() at load for 2550M: one checkbox AtcCode1..36 per
// xml/atcCodes.xml entry tagged 2550M, inside frmMain (written as true/false).
(function () {
  var tbl = d.getElementById('tbllistAtcCode');
  var rows = '';
  for (var i = 1; i <= 36; i++) {
    rows += "<tr class='atc'><td><input id='AtcCode" + i + "' name='AtcCode' type='checkbox' value='' /></td></tr>";
  }
  tbl.innerHTML = rows;
})();
