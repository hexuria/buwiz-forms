// changedrpATCList('P') for 1600PT: the ATC popup rows AtcCode1..6 (checkboxes
// inside frmMain, serialized by saveXMLsubmit as true/false).
(function () {
  var tbl = d.getElementById('tbllistAtcCode');
  var rows = '';
  for (var i = 1; i <= 6; i++) {
    rows += "<tr class='atc'><td><input id='AtcCode" + i + "' name='AtcCode" + i + "' type='checkbox' value='' /></td></tr>";
  }
  tbl.innerHTML = rows;
})();
