// Derive the official submit plaintext layout of one form.
//
// Usage: node gen_layout.js <BIR-FormXXXX.hta> <form_id>   (JSON on stdout)
//
// Run 1 puts a unique marker with a space in every text/select control and
// checks every radio/checkbox; each <div> then shows which controls it holds,
// in which order, and whether escape() ran (space -> %20). Run 2 uses the
// page's own defaults, which gives each occurrence's default body.
'use strict';
const path = require('path');
const { load, prepare, controls, setValue, run } = require('./official');

const [htaPath, formId] = process.argv.slice(2);
const { html, libs, sha256, loop } = load(htaPath);

// Split the plaintext into: the text before the first <div>, each <div>
// with the exact text that follows it, and the final tail. Forms differ in
// the whitespace they put after the header and after each div.
function parseDivs(text) {
  const header = "<?xml version='1.0'?>";
  if (!text.startsWith(header)) throw new Error('unexpected header');
  let at = text.indexOf('<div>');
  const lead = text.slice(header.length, at);
  const divs = [];
  while (text.startsWith('<div>', at)) {
    const eq = text.indexOf('=', at + 5);
    const key = text.slice(at + 5, eq);
    const end = text.indexOf(key + '=</div>', eq + 1);
    if (end < 0) throw new Error('unterminated div for ' + key);
    const body = text.slice(eq + 1, end);
    at = end + key.length + '=</div>'.length;
    let next = text.indexOf('<div>', at);
    if (next < 0) next = text.length;
    divs.push({ key, body, after: text.slice(at, next) });
    at = next;
  }
  // The last div's "after" holds the separator, the close marker and "0".
  const last = divs[divs.length - 1];
  const tail = last.after;
  return { lead, divs, tail };
}

// Run 1: markers.
const a = prepare(html);
const ctl = controls(a.form);
const marker = new Map();
ctl.forEach((el, n) => {
  const t = (el.type || '').toLowerCase();
  if (t === 'radio' || t === 'checkbox') setValue(a.doc, el, 'true');
  // Lowercase with a space: shows escape() (%20) and toUpperCase() (M..Q Z).
  else if (el.id) { const m = `m${n}q z`; if (setValue(a.doc, el, m)) marker.set(String(n), el.id); }
});
const marked = parseDivs(run(a.dom, loop, libs));

// Some loops (1702MX) normalize "numbertext" amounts (commas stripped,
// "(5.00)" -> "-5.00") and skip the <div> when the amount is zero. Those
// entries get `number: "omit-zero"`.
const numberRule = /className\.indexOf\("numbertext"\)/.test(loop);
const numberKeys = new Set(
  numberRule ? ctl.filter((el) => /(^|\s)numbertext(\s|$)/.test(el.className || '')).map((el) => el.id) : []
);

// Run 2: page defaults. Omitted zero amounts are absent here, so align by key.
const b = prepare(html);
const defaults = parseDivs(run(b.dom, loop, libs));
const defaultBody = [];
{
  let j = 0;
  marked.divs.forEach((d, n) => {
    const dd = defaults.divs[j];
    if (dd && dd.key === d.key) {
      if (dd.after !== d.after) throw new Error('layout differs between runs at ' + n);
      defaultBody.push(dd.body);
      j++;
    } else if (numberKeys.has(d.key)) {
      defaultBody.push(null); // omitted: the page default is a zero amount
    } else {
      throw new Error('div count differs between runs at ' + d.key);
    }
  });
  if (j !== defaults.divs.length) throw new Error('div count differs between runs');
}
const controlDefault = new Map(controls(b.form).map((el) => [el.id, el.value]));

const kindOf = new Map(ctl.map((el) => [el.id, (el.type || '').toLowerCase()]));
const entries = marked.divs.map((d, n) => {
  const kind = kindOf.get(d.key);
  if (kind === 'radio' || kind === 'checkbox') {
    return { key: d.key, kind: 'bool', default: defaultBody[n], after: d.after };
  }
  const parts = [];
  const re = /([mM])(\d+)([qQ])( |%20)([zZ])/g;
  let m;
  let consumed = 0;
  while ((m = re.exec(d.body))) {
    // Text the official code writes regardless of the control's value.
    if (m.index !== consumed) parts.push({ literal: d.body.slice(consumed, m.index) });
    const upper = m[1] === 'M';
    if (upper !== (m[3] === 'Q') || upper !== (m[5] === 'Z')) throw new Error('mixed case in ' + d.key);
    const part = { source: marker.get(m[2]), codec: m[4] === ' ' ? 'raw' : 'js-escape' };
    if (upper) part.uppercase = true;
    parts.push(part);
    consumed = re.lastIndex;
  }
  if (consumed !== d.body.length) parts.push({ literal: d.body.slice(consumed) });
  if (numberKeys.has(d.key)) {
    const def = defaultBody[n] === null ? String(controlDefault.get(d.key) ?? '') : defaultBody[n];
    return { key: d.key, kind: 'value', parts, default: def, number: 'omit-zero', after: d.after };
  }
  return { key: d.key, kind: 'value', parts, default: defaultBody[n], after: d.after };
});

// Probes for value-dependent rules (number formatting, zero omission).
// Each must reproduce the recorded layout exactly, or the form is flagged.
// The official numbertext normalization (NumWithParenthesis, then commas).
function officialNumber(v) {
  let out = v;
  if (v.indexOf('(') > -1 && v.indexOf(')') > -1) {
    out = out.replace('(', '-').replace(')', '');
    if (v.indexOf(',') > -1) out = out.replace(/,/g, '');
  } else if (v.indexOf(',') > -1) out = out.replace(/,/g, '');
  return out;
}
function expected(entry, value, n) {
  if (entry.kind === 'bool') return marked.divs[n].body;
  if (entry.number) {
    const v = officialNumber(value);
    return v * 1 !== 0 ? v : null;
  }
  return entry.parts.map((p) => {
    if (p.literal !== undefined) return p.literal;
    let v = value;
    if (p.uppercase) v = v.toUpperCase();
    return p.codec === 'js-escape' ? escape(v) : v;
  }).join('');
}
const value_rules = [];
for (const probe of ['1,234.50', '0.00', '(5.00)']) {
  const c = prepare(html);
  controls(c.form).forEach((el, n) => {
    const t = (el.type || '').toLowerCase();
    if (t === 'radio' || t === 'checkbox') setValue(c.doc, el, 'true');
    else if (marker.has(String(n))) setValue(c.doc, el, probe);
  });
  const got = parseDivs(run(c.dom, loop, libs)).divs;
  const want = entries
    .map((e, n) => ({ key: e.key, body: expected(e, probe, n) }))
    .filter((w) => w.body !== null);
  if (got.length !== want.length) {
    value_rules.push({ probe, issue: `div count ${got.length} vs ${want.length}` });
    continue;
  }
  got.forEach((g, n) => {
    if (g.key !== want[n].key || g.body !== want[n].body) value_rules.push({ probe, key: g.key, expected: want[n].body, official: g.body });
  });
}

const out = {
  schema_version: '1.0.0',
  form_id: formId,
  generated_by: 'oracle/gen_layout.js (official saveXMLsubmit loop run in jsdom)',
  official_hta: path.basename(htaPath),
  official_hta_sha256: sha256,
  header: "<?xml version='1.0'?>",
  lead: marked.lead,
  value_rules,
  entries,
};
process.stdout.write(JSON.stringify(out, null, 2) + '\n');
