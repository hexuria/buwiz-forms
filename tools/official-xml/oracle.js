// Official eBIRForms submit plaintext for a set of control values.
//
// Usage: node oracle.js <BIR-FormXXXX.hta> <values.json>
//
// values.json is {serialized_key: value}; radio/checkbox values are "true" or
// "false". Prints the exact plaintext the official app builds in
// saveXMLsubmit() (submit = true). Reports, on stderr, keys that are not
// controls of the form and select values that are not official options.
'use strict';
const fs = require('fs');
const { load, prepare, setValue, run } = require('./official');

const [htaPath, valuesPath] = process.argv.slice(2);
const { html, libs, loop } = load(htaPath);
const values = JSON.parse(fs.readFileSync(valuesPath, 'utf8'));
const { dom, doc, form } = prepare(html);

const problems = [];
for (const [key, value] of Object.entries(values)) {
  const el = doc.getElementById(key);
  if (!el || !form.contains(el)) { problems.push(`not a control: ${key}`); continue; }
  if (el.tagName === 'SELECT' && !key.endsWith(':txtRDOCode') &&
      ![...el.options].some((o) => o.value === String(value))) {
    problems.push(`not an official option: ${key}=${value}`);
  }
  if (!setValue(doc, el, value)) problems.push(`control refused value: ${key}=${value}`);
}
if (problems.length) process.stderr.write(problems.join('\n') + '\n');
process.stdout.write(run(dom, loop, libs));
process.exitCode = problems.length ? 2 : 0;
