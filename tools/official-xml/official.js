// Shared helpers: run an official HTA's upload loop in jsdom.
//
// What eBIRForms uploads over SFTP is the file `saveEncryptedProfile(true)`
// writes (every form's submit does `emailFilePath = saveEncryptedProfile(true)`
// then RenameAndSendFile). Set OFFICIAL_LOOP=submit for the older
// saveXMLsubmit() loop (eBIRForms Online path) instead.
'use strict';
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { JSDOM, VirtualConsole } = require('jsdom');

const BOM = String.fromCharCode(0xfeff);

// Skip whitespace and comments from i; return the index of the next code char.
function skipTrivia(src, i) {
  for (;;) {
    while (/\s/.test(src[i])) i++;
    if (src.startsWith('//', i)) { i = src.indexOf('\n', i); continue; }
    if (src.startsWith('/*', i)) { i = src.indexOf('*/', i) + 2; continue; }
    return i;
  }
}

// From an opening brace at i, return the index of its matching close brace.
function matchBrace(src, i) {
  let depth = 0;
  for (; i < src.length; i++) {
    const c = src[i];
    if (c === '/' && src[i + 1] === '/') { i = src.indexOf('\n', i); continue; }
    if (c === '/' && src[i + 1] === '*') { i = src.indexOf('*/', i) + 1; continue; }
    if (c === '"' || c === "'") {
      const q = c;
      for (i++; i < src.length && src[i] !== q; i++) if (src[i] === '\\') i++;
      continue;
    }
    if (c === '{') depth++;
    else if (c === '}' && --depth === 0) return i;
  }
  throw new Error('unbalanced braces');
}

function extractLoop(src) {
  let fn, start;
  if (process.env.OFFICIAL_LOOP === 'submit') {
    // A few forms (1700) only have saveXML(isFinalCopy).
    fn = src.indexOf('function saveXMLsubmit(');
    if (fn < 0) fn = src.indexOf('function saveXML(');
    if (fn < 0) throw new Error('no saveXML function');
    start = src.indexOf('var allXML', fn);
  } else {
    fn = src.indexOf('function saveEncryptedProfile(');
    if (fn < 0) throw new Error('no saveEncryptedProfile function');
    // The last allXML build before the file is encrypted (some forms keep an
    // older, commented-out variant above it).
    const enc = src.indexOf('EncryptFile(', fn);
    start = src.lastIndexOf('var allXML', enc);
    if (start < fn) throw new Error('allXML not found');
  }
  if (start < 0) throw new Error('allXML not found');
  let forAt = skipTrivia(src, src.indexOf(';', src.indexOf('.elements', start)) + 1);
  while (!src.startsWith('for', forAt)) forAt = skipTrivia(src, src.indexOf(';', forAt) + 1);
  // Match the for(...) header, then the body's opening brace after any comments.
  let i = src.indexOf('(', forAt), paren = 0;
  for (; i < src.length; i++) {
    if (src[i] === '(') paren++;
    else if (src[i] === ')' && --paren === 0) break;
  }
  i = skipTrivia(src, i + 1);
  if (src[i] !== '{') throw new Error('loop body is not a block');
  return src.slice(start, matchBrace(src, i) + 1);
}

// Source of a named function from any of the given texts.
function functionSource(name, texts) {
  const re = new RegExp('function\\s+' + name + '\\s*\\(');
  for (const src of texts) {
    const at = src.search(re);
    if (at < 0) continue;
    return src.slice(at, matchBrace(src, src.indexOf('{', at)) + 1);
  }
  return null;
}

function load(htaPath) {
  const raw = fs.readFileSync(htaPath);
  let html = raw.toString('utf8');
  if (html.startsWith(BOM)) html = html.slice(1);
  // The HTA and the shared scripts it loads, for helper functions the loop calls.
  const jsDir = path.join(path.dirname(htaPath), '..', 'js');
  const libs = fs.existsSync(jsDir)
    ? fs.readdirSync(jsDir).filter((f) => f.endsWith('.js')).sort()
      .map((f) => fs.readFileSync(path.join(jsDir, f), 'utf8'))
    : [];
  return {
    html,
    libs: [html, ...libs],
    sha256: crypto.createHash('sha256').update(raw).digest('hex'),
    loop: extractLoop(html),
  };
}

// A load-time helper's source, or null when absent or not parseable here.
function optionalFunctionSource(name, html) {
  try { return functionSource(name, [html]); } catch (e) { return null; }
}

// IE drops a newline directly after an <xmp> start tag (as for <pre>);
// jsdom keeps it. The xmlFormat separator is read from that element, and the
// user's real 2551Q upload proves IE's reading ("\t\t", not "\n\t\t").
function ieXmpText(doc) {
  for (const el of doc.querySelectorAll('xmp')) {
    const t = el.textContent;
    if (t.startsWith('\r\n')) el.textContent = t.slice(2);
    else if (t.startsWith('\n')) el.textContent = t.slice(1);
  }
}

// Build the DOM the way the page looks after its load handlers ran.
function prepare(html) {
  const virtualConsole = new VirtualConsole(); // ignore jsdom CSS parse warnings
  const dom = new JSDOM(html, { runScripts: 'outside-only', virtualConsole });
  const doc = dom.window.document;
  ieXmpText(doc);
  const form = doc.getElementById('frmMain');
  const first = form.querySelector('[id*=":"]');
  const prefix = first ? first.id.split(':')[0] : '';
  // getRdo() injects the RDO select into td#rdoSelect at load, with the id
  // its own markup names (frm1604c:rdoCode on 1604C; txtRDOCode on most).
  const rdoCell = doc.getElementById('rdoSelect');
  const mainRdo = /function\s+getRdo\s*\([\s\S]*?<select[^>]*\bid=['"]([^'"]+)['"]/.exec(html);
  const rdoId = mainRdo ? mainRdo[1] : prefix + ':txtRDOCode';
  if (rdoCell && prefix && !doc.getElementById(rdoId)) {
    rdoCell.innerHTML = `<select id='${rdoId}' name='${rdoId}' size='1'><option value='000'> </option></select>`;
  }
  // Other getRdo() bodies fill more cells with `$('#cell').html(data)`: 1606
  // (seller, Item 16A in td#rdoSelect2/3), 1600VT/PT (td#rdoContainer).
  // Inject those selects with the ids the page gives them.
  const getRdo = optionalFunctionSource('getRdo', html);
  if (getRdo) {
    const selectIds = {};
    for (const m of getRdo.matchAll(/var\s+(\w+)\s*=\s*"<select[^"]*?id='([^']+)'/g)) selectIds[m[1]] = m[2];
    for (const m of getRdo.matchAll(/\$\('#(\w+)'\)\.html\((\w+)\)/g)) {
      const cell = doc.getElementById(m[1]);
      const id = selectIds[m[2]];
      if (cell && id && !doc.getElementById(id)) {
        cell.innerHTML = `<select id='${id}' name='${id}' size='1'><option value='000'> </option></select>`;
      }
    }
  }
  // populateAtcPart2() (1600VT/PT) draws the five empty Part II ATC rows at
  // load; run the page's own function with a minimal `$(...).html()`.
  const populate = optionalFunctionSource('populateAtcPart2', html);
  if (populate) {
    const w = dom.window;
    w.d = doc;
    w.$ = (sel) => {
      const el = doc.getElementById(String(sel).replace(/^#/, ''));
      return { html: (h) => (h === undefined ? el.innerHTML : (el.innerHTML = h)) };
    };
    w.eval(populate + '\npopulateAtcPart2();');
    delete w.$;
  }
  // getDrives() (js/string-util.js) fills every drive select with a "0"
  // placeholder, selected, ahead of the machine's drive letters.
  for (const sel of doc.querySelectorAll('select.driveSelect')) {
    if (!sel.options.length) sel.innerHTML = "<option value='0'> - </option>";
  }
  // populateATC() fills drpATC1..n with option values 1..n at load.
  for (const sel of doc.querySelectorAll('select[id^="drpATC"]')) {
    for (let v = 1; v <= 40; v++) {
      const o = doc.createElement('option'); o.value = String(v); o.text = String(v); sel.appendChild(o);
    }
  }
  return { dom, doc, form };
}

function controls(form) {
  return [...form.elements].filter((e) => {
    const t = (e.type || '').toLowerCase();
    return t !== 'button' && t !== 'hidden' && t !== 'undefined';
  });
}

// Set a control's value; false when this control type cannot hold it.
function setValue(doc, el, value) {
  const type = (el.type || '').toLowerCase();
  if (type === 'radio' || type === 'checkbox') { el.checked = String(value) === 'true'; return true; }
  if (type === 'file') return false;
  if (el.tagName === 'SELECT' && ![...el.options].some((o) => o.value === String(value))) {
    const o = doc.createElement('option'); o.value = String(value); o.text = String(value); el.appendChild(o);
  }
  try { el.value = String(value); } catch (e) { return false; }
  return el.value === String(value);
}

// Run the loop. A helper the loop calls is loaded from the official scripts;
// any other missing global is page state set elsewhere, so it starts empty.
function run(dom, loop, libs = []) {
  const w = dom.window;
  for (let attempt = 0; attempt < 60; attempt++) {
    try {
      return w.eval(
        'var d = document;\n' + loop +
        "\nallXML += tab + d.getElementById('xmlClose').innerHTML + '0';\nallXML;"
      );
    } catch (e) {
      const msg = (e && e.message) || '';
      const m = /^(\w+) is not defined$/.exec(msg) || /^(\w+) is not a function$/.exec(msg);
      if (!m) throw e;
      const fnSrc = functionSource(m[1], libs);
      if (fnSrc) w.eval(fnSrc);
      else w[m[1]] = '';
    }
  }
  throw new Error('too many undefined globals');
}

module.exports = { load, prepare, controls, setValue, run, ieXmpText };
