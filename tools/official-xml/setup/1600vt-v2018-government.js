// changedrpATCList('G') for 1600VT: the ATC popup rows AtcCode1..12 (checkboxes
// inside frmMain, serialized by saveXMLsubmit as true/false).
(function () {
  var tbl = d.getElementById('tbllistAtcCode');
  var rows = '';
  for (var i = 1; i <= 12; i++) {
    rows += "<tr class='atc'><td><input id='AtcCode" + i + "' name='AtcCode" + i + "' type='checkbox' value='' /></td></tr>";
  }
  tbl.innerHTML = rows;
})();
