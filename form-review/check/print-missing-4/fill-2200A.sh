#!/bin/bash
# Fills the audit 2200A return through the agent write path. Usage: fill-2200A.sh <addr> <token>
# Identical at every commit -> the same saved draft before and after.
A="--addr $1"; export GPUI_AGENT_TOKEN=$2
set -e
gpui-agent invoke profile.save $A --arg tin=00012345300000 --arg tin_root=000123453 --arg name="Juana Santos" --arg taxpayer_type=individual --arg rdo_code=008 --arg zip_code=1004 --arg address_line1="1234 Socorro St" >/dev/null 2>&1 || true
gpui-agent invoke profile.forms_set $A --arg tin=00012345300000 --arg year=2026 --arg confirm=true --arg 'codes=["2200A","2200S","2550Q","1702Q","0605","1701","1600WP","1702MX"]' >/dev/null
gpui-agent invoke form.open $A --arg code=2200A --arg year=2026 --arg period=11 --arg tin=00012345300000 >/dev/null
gpui-agent invoke form.fill $A --arg 'fields={"frm2200Av2020:txtSheets":"3","frm2200Av2020:phoneNumber":"09171234567","frm2200Av2020:txtEmail":"juana@example.com","frm2200Av2020:prodRegion":"CAR","frm2200Av2020:prodProvince":"Benguet","frm2200Av2020:prodCity":"Baguio City","frm2200Av2020:remRegion":"CAR","frm2200Av2020:remProvince":"Benguet","frm2200Av2020:remCity":"Baguio City","frm2200Av2020:cashBank":"Rizal Commercial Banking Corporation International Remittance","frm2200Av2020:cashNum":"111222333444555","frm2200Av2020:cashDate":"11/20/2026","frm2200Av2020:cashAmt":"0.05","frm2200Av2020:checkBank":"Bank of the Philippine Islands Commonwealth Avenue Branch","frm2200Av2020:checkNum":"987654321012345678","frm2200Av2020:checkDate":"11/24/2026","frm2200Av2020:checkAmt":"123456789012.34","frm2200Av2020:taxAmt":"7777777.77","frm2200Av2020:othersDetails":"Miscellaneous excise payment","frm2200Av2020:othersBank":"Metrobank Only Bank Case","frm2200Av2020:txtExciseDue":"99999999.99","frm2200Av2020:txtPrevReturn":"22222222.22"}' | head -c 200
echo
gpui-agent invoke form.save_draft $A >/dev/null && echo SAVED-2200A
