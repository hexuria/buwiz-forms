# Disabled-not-cleared official upload evidence

Main: `0265dc6103482b201d017ea512f3700918fb927e`. Official package: `hexuria/buwiz-validation` `ea8334a9bf8b0afa54cd40fe9af294771c3cf8e4` (`package/forms/*.hta`, recovered v7.9.6 generation). No product change.

The filed upload is the plaintext built by `saveEncryptedProfile(true)` before `EncryptFile(...)`: `tools/official-xml/README.md` states that submit uses `emailFilePath = saveEncryptedProfile(true)`, and `official.js` extracts that loop. In every relevant form the loop iterates `document.getElementById('frmMain').elements` and writes every `text`/`select-one` control's current `.value`. Its only type exclusions are `button`, `hidden`, and `undefined`; it does not inspect `.disabled` anywhere.

## Result per key

| form/key | upload answer | evidence |
|---|---|---|
| 2000 Item 15A, `frm2000:txtTax15A` | **KEPT** | input is a disabled text control at `BIR-Form2000v2018.hta:752`; `processAmended()` lines 3253-3258 only toggles `.disabled` and never writes `.value`; `saveEncryptedProfile()` lines 2606-2612 writes every text value |
| 1702MX Schedule 3 Items 27A-27C, `frm1702MX:txtPg2Sc3It27A/B/C` | **KEPT** (numeric normalization only) | inputs are `class="numbertext"` at lines 2282, 2287 and 2292; the amended-return gate lines 18496-18501 only enables/disables them; `saveEncryptedProfile()` lines 20176-20196 writes every numeric text value after removing commas or converting parenthesized negatives |
| 1701MS Part V Item 6 on short period, `frm1701MS:txtTaxPaidPreviouslyFiledNo6a/b` | **KEPT** | inputs are disabled text controls at lines 3755-3767 and 3771-3780; `shortPeriodReturn()` lines 9805-9825 toggles `.disabled` and does not write either value; `saveEncryptedProfile()` lines 6732-6737 writes every text value |
| 1701A Items 61B/62B with joint filing off, `frm1701A:txt61B`, `frm1701A:txt62B` | **KEPT** | inputs are disabled text controls at lines 1415 and 1428; `processFilingStatus()` lines 3486-3491 calls `disableSpouse()` when joint filing is off; `disableSpouse()` lines 4621-4684 does not mention `txt61B` or `txt62B` at all; `saveEncryptedProfile()` lines 6559-6564 writes every text value |

## Exact serializer quotes

### 2000: `saveEncryptedProfile`, lines 2606-2612

```js
var elem = document.getElementById('frmMain').elements;
for(var i = 0; i < elem.length; i++)
{
    if (elem[i].type != 'button' && elem[i].type != 'hidden' && elem[i].type != 'undefined') {
        if (elem[i].type == 'text' || elem[i].type == 'select-one') {
            allXML += "<div>"+elem[i].id+"=" +elem[i].value+elem[i].id+"=</div>"+tab;
```

`processAmended`, lines 3253-3258:

```js
function processAmended() {
    if (d.getElementById("frm2000:AmendedRtn_1").checked) {
        d.getElementById("frm2000:txtTax15A").disabled = false;
    } else {
        d.getElementById("frm2000:txtTax15A").disabled = true;
    }
}
```

### 1702MX: `saveEncryptedProfile`, lines 20176-20196

```js
var elem = document.getElementById('frmMain').elements;
for (var i = 0; i < elem.length; i++) {
    if (elem[i].type != 'button' && elem[i].type != 'hidden' && elem[i].type != 'undefined') {
        if (elem[i].type == 'text' || elem[i].type == 'select-one') {
            ...
            else if (elem[i].className.indexOf("numbertext") > -1) {
                var elemValue = elem[i].value;
                if (elem[i].value.toString().indexOf(',') > -1) {
                    elemValue = elemValue.replace(/,/g, '');
                }
                if (elem[i].value.toString().indexOf('(') > -1 && elem[i].value.toString().indexOf(')') > -1) {
                    elemValue = NumWithParenthesis(elemValue);
                }
                allXML += "<div>" + elem[i].id + "=" + elemValue + elem[i].id + "=</div>" + tab;
            }
```

The gate, lines 18496-18501:

```js
if(isAmendedReturn){
    enableField('txtPg2Sc3It27A,txtPg2Sc3It27B,txtPg2Sc3It27C');
}
else{
    disableField('txtPg2Sc3It27A,txtPg2Sc3It27B,txtPg2Sc3It27C');
}
```

### 1701MS: `saveEncryptedProfile`, lines 6732-6737

```js
var elem = document.getElementById('frmMain').elements;
for (var i = 0; i < elem.length; i++) {
    if (elem[i].type != 'button' && elem[i].type != 'hidden' && elem[i].type != 'undefined') {
        if (elem[i].type == 'text' || elem[i].type == 'select-one') {
            allXML += "<div>" + elem[i].id + "=" + elem[i].value + elem[i].id + "=</div>" + tab;
```

`shortPeriodReturn`, lines 9814-9824:

```js
d.getElementById('frm1701MS:txtMonthNo1').disabled = false;
document.getElementById('frm1701MS:txtTaxPaidPreviouslyFiledNo6a').disabled = false
document.getElementById('frm1701MS:txtTaxPaidPreviouslyFiledNo6b').disabled = false
...
document.getElementById('frm1701MS:txtTaxPaidPreviouslyFiledNo6a').disabled = true
document.getElementById('frm1701MS:txtTaxPaidPreviouslyFiledNo6b').disabled = true
```

### 1701A: `saveEncryptedProfile`, lines 6559-6564

```js
var elem = document.getElementById('frmMain').elements;
for (var i = 0; i < elem.length; i++) {
    if (elem[i].type != 'button' && elem[i].type != 'hidden' && elem[i].type != 'undefined') {
        if (elem[i].type == 'text' || elem[i].type == 'select-one') {
            allXML += "<div>" + elem[i].id + "=" + elem[i].value + elem[i].id + "=</div>" + tab;
```

`processFilingStatus`, lines 3486-3491:

```js
function processFilingStatus() {
    if (d.getElementById("frm1701A:optFilingStatus_1").checked) {
        enableSpouse();
    } else {
        disableSpouse();
    }
}
```

`disableSpouse`, lines 4621-4684, disables and zeroes other spouse fields but contains no `txt61B` or `txt62B` reference.

## Executed official-loop probe

`node tools/official-xml/oracle.js` runs the extracted official `saveEncryptedProfile` loop in jsdom. Values were injected without enabling the controls. Relevant output:

```text
<div>frm2000:txtTax15A=123.45frm2000:txtTax15A=
<div>frm1702MX:txtPg2Sc3It27A=111frm1702MX:txtPg2Sc3It27A=
<div>frm1702MX:txtPg2Sc3It27B=222frm1702MX:txtPg2Sc3It27B=
<div>frm1702MX:txtPg2Sc3It27C=333frm1702MX:txtPg2Sc3It27C=
<div>frm1701MS:txtTaxPaidPreviouslyFiledNo6a=444frm1701MS:txtTaxPaidPreviouslyFiledNo6a=
<div>frm1701MS:txtTaxPaidPreviouslyFiledNo6b=555frm1701MS:txtTaxPaidPreviouslyFiledNo6b=
<div>frm1701A:txt61B=666frm1701A:txt61B=
<div>frm1701A:txt62B=777frm1701A:txt62B=
```

## Consequence

Every requested key is **KEPT** when disabled but still holding a value. Therefore, if main clears any of them solely because the control is disabled, our clearing changes what is filed for all of these keys: 2000 Item 15A; 1702MX Schedule 3 Items 27A-27C; 1701MS Part V Item 6 on the short-period path; and 1701A Items 61B/62B with joint filing off.
