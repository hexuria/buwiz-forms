# official-xml

Reproduces the official eBIRForms submit plaintext byte for byte.

Each official form HTA builds its upload in `saveXMLsubmit()`: a loop over the
page controls that writes `<div>key=valuekey=</div>` with per-form quirks. These
tools run that exact loop, taken from the HTA, in [jsdom](https://github.com/jsdom/jsdom)
against the HTA's own `<form id="frmMain">`. Nothing is reimplemented by hand.

Needs Node 20+ and a checkout of
[hexuria/buwiz-validation](https://github.com/hexuria/buwiz-validation)
(the official package, byte-exact).

| File | Does |
|---|---|
| `official.js` | Loads an HTA, extracts the submit loop, prepares the DOM the way the page's load handlers leave it (RDO select, ATC options, drive select), runs the loop. Helper functions the loop calls run from the official scripts. |
| `gen_layout.js` | Derives a form's layout: order, `escape()` or raw, duplicates, folded fields, literals, defaults and the exact text after each `<div>`. Probes with `1,234.50`, `0.00` and `(5.00)` record any value-dependent rule in `value_rules`. |
| `generate.sh` | Runs `gen_layout.js` for every form in `forms.txt` into `crates/bir-core/data/official-xml/`. |
| `oracle.js` | The official plaintext for a JSON map of values. Flags keys that are not controls and select values that are not official options. |
| `gen_atc_options.js` | The official 2551Q ATC dropdown options from `xml/atcCodes.xml`. |
| `runtime.js` | Runs the whole official page with its own scripts (ActiveX file access served from the package; VBScript, dialogs and drives stubbed), plays a list of filer steps through the official event handlers, and prints what `saveXMLsubmit()` submits. Official calculations and amount formatting (`250,000.00`) happen inside. `alert()` texts go to stderr. |
| `list_inputs.js`, `steps_from_plain.py` | Turn one of our plaintexts into runtime steps: typed controls get their value, true radios get clicked, disabled numeric controls are left for the official page to compute. |

## Regenerate

```sh
tools/official-xml/generate.sh /path/to/buwiz-validation
node tools/official-xml/gen_atc_options.js /path/to/buwiz-validation \
  > crates/bir-core/data/official-xml/2551q-v2018-atc-options.json
```

## Check a form against the official app

1. Write the form's field map as JSON (`{"frm…:txtYear": "2025", …}`).
2. `node oracle.js /path/to/buwiz-validation/package/forms/BIR-Form….hta values.json > official.xml`
3. Compare `official.xml` with our `to_bir_xml_payload()` output byte for byte.

## Check a form end to end (calculations and formatting too)

```sh
python3 tools/official-xml/steps_from_plain.py BIR-Form….hta ours.plain.xml steps.json
node tools/official-xml/runtime.js BIR-Form….hta steps.json > official.xml
```

`crates/bir-core/tests/sample-payloads/*.official.xml` and `*.steps.json` are
made this way; `sample_payloads_test` requires our plaintext to equal them.

## Known gaps

- `1702mx-v2018c` amounts (`numbertext`) are normalized (`1,234.50` ->
  `1234.50`, `(5.00)` -> `-5.00`) and written only when non-zero. Those
  entries carry `"number": "omit-zero"`; `official_xml::write` applies the
  rule and `read` accepts the missing `<div>`s.
- `value_rules` is non-empty for 1600PT, 1600VT, 1702EX, 1702RT and 1707A
  (commas stripped, parentheses become a minus sign). Their field maps must
  apply the same rule before `official_xml::write`.
