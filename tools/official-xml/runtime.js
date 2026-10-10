// Run an official form page with its own scripts, enter values the way a
// filer does, and return what the official saveXMLsubmit() writes.
//
// Usage: node runtime.js <BIR-FormXXXX.hta> <inputs.json>
//
// inputs.json is an ordered list of steps:
//   [{"set": "frm2553:txt14C", "value": "1234.5"},   // type, then blur/change
//    {"click": "frm2553:optQtr:_1"},                  // radio / checkbox
//    {"call": "computeTaxDue('frm2553:txt14C','frm2553:txt14D','frm2553:txt14E')"},
//    {"wait": 300}]                                   // let page timers run
// The official event handlers compute and format every derived value. Windows-only
// pieces (ActiveX, VBScript, file dialogs) are stubbed; alert() texts are reported
// on stderr so validation messages can be compared too.
'use strict';
const fs = require('fs');
const path = require('path');
const { JSDOM, VirtualConsole } = require('jsdom');
const { load, setValue } = require('./official');

const [htaPath, inputsPath] = process.argv.slice(2);
const steps = JSON.parse(fs.readFileSync(inputsPath, 'utf8'));
const { html, loop } = load(htaPath);
const alerts = [];

const virtualConsole = new VirtualConsole();
virtualConsole.on('jsdomError', (e) => {
  if (!/Could not parse CSS|Not implemented/.test(e.message)) process.stderr.write('jsdom: ' + e.message + (process.env.RUNTIME_DEBUG && e.detail && e.detail.stack ? '\n' + e.detail.stack : '') + '\n');
});

function inert() {
  // Any ActiveX object: every property is a function returning another inert object.
  const target = function () {};
  return new Proxy(target, {
    get: (_, prop) => (prop === Symbol.toPrimitive ? () => '' : prop === 'length' ? 0 : inert()),
    apply: () => inert(),
    construct: () => inert(),
  });
}

// Scripting.FileSystemObject backed by the official package directory, so
// the page loads xml/atcCodes.xml, xml/rdo.xml and friends as it does on
// Windows. Writes are discarded.
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

// Microsoft.XMLDOM over the package files (js/tax-rate-helper.js loads
// xml/taxRate.xml this way). That file starts with a comment *before* its
// XML declaration; we parse it as BIR clearly intends (the declaration is
// dropped), so the official rates load. Whether IE's MSXML tolerates the
// misplaced declaration can't be checked here; if it doesn't, the official
// page keeps its "0%" defaults.
function xmlDom(window) {
  const doc = {
    async: true,
    parseError: { errorCode: 0, reason: '', line: 0 },
    documentElement: null,
    load(p) {
      const file = resolvePackagePath(String(p).replace(/^\.\.\//, ''));
      if (!fs.existsSync(file)) { this.parseError = { errorCode: 1, reason: 'missing', line: 0 }; return false; }
      return this.loadXML(fs.readFileSync(file, 'utf8'));
    },
    loadXML(text) {
      const cleaned = text.replace(/<\?xml[^?]*\?>/, '');
      const parsed = new window.DOMParser().parseFromString('<__root>' + cleaned + '</__root>', 'application/xml');
      if (parsed.getElementsByTagName('parsererror').length) { this.parseError = { errorCode: 1, reason: 'parse', line: 0 }; return false; }
      this.parsed = parsed;
      this.documentElement = parsed.documentElement.firstElementChild;
      return true;
    },
    getElementsByTagName(name) { return this.parsed ? this.parsed.getElementsByTagName(name) : []; },
    selectNodes(xpath) { return this.parsed ? this.parsed.getElementsByTagName(xpath.split('/').pop()) : []; },
  };
  return doc;
}

const dom = new JSDOM(html, {
  url: 'file://' + path.resolve(htaPath),
  runScripts: 'dangerously',
  resources: 'usable',
  virtualConsole,
  beforeParse(window) {
    window.ActiveXObject = function (progId) {
      if (/FileSystemObject/i.test(String(progId))) return fileSystemObject();
      if (/XMLDOM|DOMDocument/i.test(String(progId))) return xmlDom(window);
      return inert();
    };
    // JScript's Enumerator (drive listing): an empty collection.
    window.Enumerator = function () {
      return { atEnd: () => true, moveNext: () => {}, item: () => inert(), moveFirst: () => {} };
    };
    // IE XML nodes expose .text (textContent).
    if (!Object.getOwnPropertyDescriptor(window.Node.prototype, 'text')) {
      Object.defineProperty(window.Element.prototype, 'text', {
        configurable: true,
        get() { return this.textContent; },
      });
    }
    window.alert = (msg) => alerts.push(String(msg));
    window.confirm = () => true;
    window.prompt = () => '';
    window.print = () => {};
    window.showModalDialog = () => undefined;
    window.execScript = () => {};
    // VBScript helpers from js/eBIRTools.vbs.
    window.ValidateTinWChkDgt = () => 0;
    window.EncryptFile = () => 0;
    window.RenameAndSendFile = () => 0;
  },
});

// Many forms finish startup in a timer (2551Q: setInterval(sleeptime, 300)
// loads ATCs and runs init()). Let those run before typing, and let handler
// timers settle before serializing.
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
dom.window.addEventListener('load', async () => {
  await sleep(Number(process.env.RUNTIME_SETTLE_MS || 1500));
  const w = dom.window;
  const doc = w.document;
  const fire = (el, type) => el.dispatchEvent(new w.Event(type, { bubbles: true }));
  for (const step of steps) {
    try {
      if (step.call) { w.eval(step.call); continue; }
      if (step.wait) { await sleep(Number(step.wait)); continue; }
      const el = doc.getElementById(step.set || step.click);
      if (!el) { process.stderr.write('no control: ' + (step.set || step.click) + '\n'); continue; }
      if (step.click) { el.checked = true; el.click(); fire(el, 'change'); continue; }
      fire(el, 'focus');
      setValue(doc, el, step.value);
      fire(el, 'keyup');
      fire(el, 'change');
      fire(el, 'blur');
    } catch (e) {
      process.stderr.write(`step ${JSON.stringify(step)} threw: ${e.message}\n`);
    }
  }
  await sleep(500);
  let out;
  try {
    out = w.eval('var d = document;\n' + loop +
      "\nallXML += tab + d.getElementById('xmlClose').innerHTML + '0';\nallXML;");
  } catch (e) {
    process.stderr.write('submit loop threw: ' + e.message + '\n');
    process.exitCode = 1;
    return;
  }
  if (alerts.length) process.stderr.write('alerts: ' + JSON.stringify(alerts) + '\n');
  process.stdout.write(out);
  w.close();
});
