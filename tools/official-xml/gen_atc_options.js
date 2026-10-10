// The official 2551Q ATC dropdown options, as loadATC()/populateATC() build
// them in BIR-Form2551Qv2018.hta: every atcCodes.xml entry whose text holds
// "2551_", in file order, numbered from 1. Option 0 is the blank choice.
//
// Usage: node gen_atc_options.js <buwiz-validation dir>   (JSON on stdout)
'use strict';
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const dir = process.argv[2];
const file = path.join(dir, 'package', 'xml', 'atcCodes.xml');
const raw = fs.readFileSync(file);
const text = raw.toString('utf8');
const count = Number(text.split('atcCount=')[1].split('atcCount=')[0]);
const options = [];
for (let i = 1; i <= count; i++) {
  // Same split as the official loadATC().
  const atcStr = text.split('atc' + i + ':')[1];
  if (atcStr.indexOf('2551_') < 0) continue;
  const v = atcStr.split('~');
  options.push({ value: String(options.length + 1), code: v[0], rate: v[2], description: v[1] });
}
process.stdout.write(JSON.stringify({
  form_id: '2551q-v2018',
  source: 'package/xml/atcCodes.xml',
  source_sha256: crypto.createHash('sha256').update(raw).digest('hex'),
  blank: { value: '0', rate: '0.00' },
  options,
}, null, 2) + '\n');
