// The text controls of an official form, with whether each is numeric
// (keypress/blur handlers that filter or round numbers) and disabled.
// Usage: node list_inputs.js <BIR-FormXXXX.hta>   (JSON on stdout)
'use strict';
const { load, prepare } = require('./official');

const { html } = load(process.argv[2]);
const { form } = prepare(html);
const out = [];
for (const el of form.elements) {
  if ((el.type || '').toLowerCase() !== 'text' || !el.id) continue;
  const handlers = (el.getAttribute('onkeypress') || '') + ' ' + (el.getAttribute('onblur') || '');
  out.push({
    id: el.id,
    numeric: /numbersonly|round\(|compute/i.test(handlers),
    disabled: el.hasAttribute('disabled'),
  });
}
process.stdout.write(JSON.stringify(out));
