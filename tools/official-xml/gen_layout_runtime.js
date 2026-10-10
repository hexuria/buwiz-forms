// Derive a form's official submit layout from the page as it stands after its
// own load handlers ran (the environment of runtime.js), instead of the static
// DOM gen_layout.js prepares.
//
// Usage: node gen_layout_runtime.js <BIR-FormXXXX.hta> <form_id>   (JSON on stdout)
//
// Use it for forms whose load handlers add controls that gen_layout.js does
// not know about: getRdo() filling several RDO cells (1706: txtRDOCodeB and
// txtRDOCode14A; 1707: rdoPg1Pt1I5RDO; 1707A: rdoI6RDO; 1801: txtRDOCode_1)
// or schedule rows created at load (1801). The marker / default / probe runs
// are the same as gen_layout.js; each run loads a fresh page. The page clock
// is frozen at 2000-01-01 so defaults that init() derives from today's date
// stay reproducible.
'use strict';
const fs = require('fs');
const path = require('path');
const { JSDOM, VirtualConsole } = require('jsdom');
const { load, controls, setValue, run } = require('./official');

const [htaPath, formId] = process.argv.slice(2);
const { html, libs, sha256, loop } = load(htaPath);
const FROZEN = Date.UTC(2000, 0, 1, 0, 0, 0);

function inert() {
  const target = function () {};
  return new Proxy(target, {
    get: (_, prop) => (prop === Symbol.toPrimitive ? () => '' : prop === 'length' ? 0 : inert()),
    apply: () => inert(),
    construct: () => inert(),
  });
}
const packageDir = path.resolve(path.dirname(htaPath), '..');
function resolvePackagePath(p) {
  const rel = String(p).replace(/\\/g, '/').replace(/^[A-Za-z]:\/eBIRForms\//i, '');
  return path.isAbsolute(rel) ? rel : path.join(packageDir, rel);
}
function textStream(content) {
  const lines = content.split(/\r?\n/);
  let at = 0;
  return {
    ReadAll: () => content,
    ReadLine: () => lines[at++] || '',
    get AtEndOfStream() { return at >= lines.length; },
    Write: () => {}, WriteLine: () => {}, Close: () => {},
  };
}
function fileSystemObject() {
  return {
    FileExists: (p) => fs.existsSync(resolvePackagePath(p)),
    FolderExists: (p) => fs.existsSync(resolvePackagePath(p)),
    OpenTextFile: (p) => {
      const file = resolvePackagePath(p);
      return textStream(fs.existsSync(file) ? fs.readFileSync(file, 'utf8') : '');
    },
    CreateTextFile: () => textStream(''),
    GetAbsolutePathName: (p) => resolvePackagePath(p),
    GetFolder: () => inert(),
    DeleteFile: () => {}, CreateFolder: () => {}, MoveFile: () => {}, CopyFile: () => {},
    Drives: inert(),
  };
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

// A fresh official page after its load handlers and startup timers ran.
function loadPage() {
  return new Promise((resolve) => {
    const virtualConsole = new VirtualConsole();
    const dom = new JSDOM(html, {
      url: 'file://' + path.resolve(htaPath),
      runScripts: 'dangerously',
      resources: 'usable',
      virtualConsole,
      beforeParse(window) {
        const RealDate = window.Date;
        class FrozenDate extends RealDate {
          constructor(...args) { if (args.length) super(...args); else super(FROZEN); }
          static now() { return FROZEN; }
        }
        window.Date = FrozenDate;
        window.ActiveXObject = function (progId) {
          return /FileSystemObject/i.test(String(progId)) ? fileSystemObject() : inert();
        };
        window.Enumerator = function () {
          return { atEnd: () => true, moveNext: () => {}, item: () => inert(), moveFirst: () => {} };
        };
        window.alert = () => {};
        window.confirm = () => true;
        window.prompt = () => '';
        window.print = () => {};
        window.showModalDialog = () => undefined;
        window.execScript = () => {};
        window.ValidateTinWChkDgt = () => 0;
        window.EncryptFile = () => 0;
        window.RenameAndSendFile = () => 0;
      },
    });
    dom.window.addEventListener('load', async () => {
      await sleep(Number(process.env.RUNTIME_SETTLE_MS || 1500));
      const doc = dom.window.document;
      // HTAs run in IE7 mode, where an unknown input type such as "number"
      // is a text box (1707v2021 Schedule 2 shares, row A).
      for (const el of doc.querySelectorAll('input[type="number"]')) el.type = 'text';
      resolve({ dom, doc, form: doc.getElementById('frmMain') });
    });
  });
}

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
  return { lead, divs };
}

(async () => {
  // Run 1: markers.
  const a = await loadPage();
  const ctl = controls(a.form);
  const marker = new Map();
  // Two controls can share an id (1707A txtI11Email: Item 11 and the e-mail
  // dialog). getElementById only ever reaches the first, so a later twin is
  // recorded under `<id>#<k>`: it keeps its page default unless a field map
  // names that pseudo-source explicitly.
  const seenIds = new Map();
  ctl.forEach((el, n) => {
    const t = (el.type || '').toLowerCase();
    if (t === 'radio' || t === 'checkbox') setValue(a.doc, el, 'true');
    else if (el.id) {
      const k = (seenIds.get(el.id) || 0) + 1;
      seenIds.set(el.id, k);
      const m = `m${n}q z`;
      if (setValue(a.doc, el, m)) marker.set(String(n), k === 1 ? el.id : `${el.id}#${k}`);
    }
  });
  const marked = parseDivs(run(a.dom, loop, libs));
  a.dom.window.close();

  // Run 2: page defaults.
  const b = await loadPage();
  const defaults = parseDivs(run(b.dom, loop, libs));
  b.dom.window.close();
  if (defaults.divs.length !== marked.divs.length) throw new Error('div count differs between runs');
  defaults.divs.forEach((d, n) => {
    if (d.key !== marked.divs[n].key || d.after !== marked.divs[n].after) throw new Error('layout differs between runs at ' + n);
  });

  const kindOf = new Map(ctl.map((el) => [el.id, (el.type || '').toLowerCase()]));
  const entries = marked.divs.map((d, n) => {
    const kind = kindOf.get(d.key);
    if (kind === 'radio' || kind === 'checkbox') {
      return { key: d.key, kind: 'bool', default: defaults.divs[n].body, after: d.after };
    }
    const parts = [];
    const re = /([mM])(\d+)([qQ])( |%20)([zZ])/g;
    let m;
    let consumed = 0;
    while ((m = re.exec(d.body))) {
      if (m.index !== consumed) parts.push({ literal: d.body.slice(consumed, m.index) });
      const upper = m[1] === 'M';
      if (upper !== (m[3] === 'Q') || upper !== (m[5] === 'Z')) throw new Error('mixed case in ' + d.key);
      const part = { source: marker.get(m[2]), codec: m[4] === ' ' ? 'raw' : 'js-escape' };
      if (upper) part.uppercase = true;
      parts.push(part);
      consumed = re.lastIndex;
    }
    if (consumed !== d.body.length) parts.push({ literal: d.body.slice(consumed) });
    return { key: d.key, kind: 'value', parts, default: defaults.divs[n].body, after: d.after };
  });

  function expected(entry, value, n) {
    if (entry.kind === 'bool') return marked.divs[n].body;
    return entry.parts.map((p) => {
      if (p.literal !== undefined) return p.literal;
      let v = value;
      if (p.uppercase) v = v.toUpperCase();
      return p.codec === 'js-escape' ? escape(v) : v;
    }).join('');
  }
  const value_rules = [];
  for (const probe of ['1,234.50', '0.00', '(5.00)']) {
    const c = await loadPage();
    controls(c.form).forEach((el, n) => {
      const t = (el.type || '').toLowerCase();
      if (t === 'radio' || t === 'checkbox') setValue(c.doc, el, 'true');
      else if (marker.has(String(n))) setValue(c.doc, el, probe);
    });
    const got = parseDivs(run(c.dom, loop, libs)).divs;
    c.dom.window.close();
    if (got.length !== entries.length) {
      value_rules.push({ probe, issue: `div count ${got.length} vs ${entries.length}` });
      continue;
    }
    got.forEach((g, n) => {
      const want = expected(entries[n], probe, n);
      if (g.key !== entries[n].key || g.body !== want) value_rules.push({ probe, key: g.key, expected: want, official: g.body });
    });
  }

  const out = {
    schema_version: '1.0.0',
    form_id: formId,
    generated_by: 'oracle/gen_layout_runtime.js (official saveXMLsubmit loop run on the loaded page in jsdom)',
    official_hta: path.basename(htaPath),
    official_hta_sha256: sha256,
    header: "<?xml version='1.0'?>",
    lead: marked.lead,
    value_rules,
    entries,
  };
  process.stdout.write(JSON.stringify(out, null, 2) + '\n');
  process.exit(0);
})().catch((e) => { process.stderr.write(String(e && e.stack || e) + '\n'); process.exit(1); });
