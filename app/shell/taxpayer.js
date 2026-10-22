// The taxpayer page (DESIGN_SYSTEM.md "Main area for a taxpayer" and "Tax
// profile: tax years, forms, TIN and templates"): a header with Copy TIN and
// New return, and the Profile, Forms and Templates tabs. Also the New
// taxpayer and New branch pages, and the right panel's in-place edit.
//
// The rules that must hold for every caller live in bir-core, not here: the
// tax-year bound (2010 to next year) and the one-transaction TIN move. This
// file only shapes input and reports what the data layer answers.

import { div } from "gpui-kit";
import { InputState, OtpState } from "gpui-base";
import {
  tokens,
  SPACE,
  RADIUS,
  Text,
  Code,
  Icon,
  AppButton,
  Avatar,
  Chip,
  Segmented,
  Switch,
  CheckboxLabel,
  Checkbox,
  TextField,
  Dropdown,
  Field,
  FieldGrid,
  Disclosure,
  DisclosureGroup,
  FormActions,
  ListGroup,
  ListRow,
  NameCell,
  EmptyRow,
  SettingRow,
  Card,
  SectionTitle,
  PageHeader,
  MetaLine,
  Tabs,
  Banner,
  DigitEntry,
  DigitCells,
  PinCells,
  ComboState,
  ComboField,
} from "./ui.js";
import { errorText as message, formatTin, shortDate, daysUntil } from "./store.js";
import { searchText } from "./library.js";

const SELECTION_KEY = "redesign.selection";
const TAXPAYER_TYPES = ["Individual", "Corporation", "Partnership", "Cooperative", "Estate", "Trust"];
const FREQS = ["All", "Monthly", "Quarterly", "Annual", "As needed"];
const ADDRESS_FIELDS = [
  ["registered_address", "Registered address", true],
  ["zip_code", "ZIP code", false],
  ["phone", "Phone", false],
  ["email", "Email", true],
];

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

/**
 * What is wrong with an RDO, ZIP code or email as typed, or null. Empty is
 * fine: none of them is required to create a taxpayer or branch.
 */
function factError(field, state) {
  const v = state.value().trim();
  if (!v) return null;
  if (field === "rdo_code" && state instanceof ComboState && !state.isKnown()) return "Pick an RDO from the list. Type its code or district to search.";
  if (field === "zip_code" && !/^\d{4}$/.test(v)) return "ZIP code is 4 digits, like 1226. Type a code or a place to search.";
  if (field === "email" && !EMAIL_RE.test(v)) return "Type an email like name@example.com.";
  return null;
}

/** An RDO or ZIP pick list over the reference lists, other facts a text box. */
function factState(app, field, value) {
  if (field === "rdo_code") return new ComboState(app.store.rdoOptions(), value, "Type a code or district, like 050 or Makati");
  if (field === "zip_code") return new ComboState(app.store.zipOptions(), value, "Type a code or place, like 1226 or Makati");
  return InputState.new({ value });
}

/** Owns the taxpayer screens' state and talks to the store. */
export class TaxpayerActions {
  constructor(app) {
    this.app = app;
    this.open = { taxpayer: true, branch: true, address: true, inbox: true };
    this.editSec = null;
    this.inputs = {};
    this.inherit = {};
    this.typeOpen = false;
    this.typeValue = null;
    this.tinError = null;
    /** Per-field RDO, ZIP and email errors of the section being edited. @type {Record<string, string>} */
    this.factErrors = {};
    this.panelEdit = null;
    // Confirmation inbox (P2-5): staged credentials edit + verify/check status.
    this.inbox = null;
    this.inboxMsg = null;
    this.inboxBusy = false;
    this.forms = null; // staged Set while editing forms
    this.formsQuery = InputState.new({ placeholder: "Find a form, like 1601C or VAT" });
    this.formsQuery.on("change", (_e, cx) => cx.notify());
    this.formsFreq = "All";
    this.addYear = false;
    this.addYearSel = null;
    this.addYearCopy = true;
    this.addYearOpen = false;
    this.newTp = null;
    this.newBranch = null;
    this.deleteArmed = false;
    // Profile export (P3): the taxpayer PIN gates saving the archive when one
    // is set. Typed here, sent to the host, cleared after every attempt — the
    // page never keeps it.
    this.exportOtp = OtpState.new(4, { masked: true });
  }

  /** Archive and delete: the admin only, never on the admin's own account. */
  canRemove() {
    const app = this.app;
    const t = app.taxpayer();
    return Boolean(t) && app.isAdmin() && t.tin_root !== app.adminRoot();
  }

  toggleArchive(cx) {
    const app = this.app;
    const t = app.taxpayer();
    try {
      app.store.write("set_taxpayer_archived", t.tin_root, !t.archived);
      app.toast(t.archived ? `${t.name} is back in new work.` : `${t.name} archived.`, cx, "archive");
    } catch (e) {
      app.toast(message(e), cx, "triangle-alert");
    }
  }

  disarmDelete(cx) {
    this.deleteArmed = false;
    cx.notify();
  }

  /** First press arms it; the second deletes. */
  deleteTaxpayer(cx) {
    const app = this.app;
    if (!this.deleteArmed) {
      this.deleteArmed = true;
      return cx.notify();
    }
    const t = app.taxpayer();
    try {
      app.store.write("delete_taxpayer", t.tin_root);
      this.deleteArmed = false;
      app.selRoot = null;
      app.selCode = null;
      app.writeLocal(SELECTION_KEY, { root: null, code: null });
      app.go("home", cx);
      app.toast(`${t.name} deleted.`, cx, "trash");
    } catch (e) {
      app.toast(message(e), cx, "triangle-alert");
    }
  }

  /**
   * Export this profile through the host's save dialog. The archive never
   * carries credentials; when the taxpayer has a PIN the typed cells gate the
   * export and are cleared after every attempt, so the PIN never sits in page
   * state. A cancelled dialog is quiet.
   */
  exportProfile(cx) {
    const app = this.app;
    const t = app.taxpayer();
    const hasPin = Boolean(app.taxpayerCreds(t.tin_root).has_pin);
    const pin = hasPin ? this.exportOtp.value() : null;
    if (hasPin && pin.length !== 4) {
      this.exportOtp.set_value("");
      return app.toast("Type all 4 digits of the profile PIN first.", cx, "lock");
    }
    this.exportOtp.set_value("");
    try {
      const r = app.store.write("export_profile", app.branch().tin, pin);
      if (r.kind === "exported") app.toast("Profile exported.", cx, "download");
    } catch (e) {
      app.toast(message(e), cx, "triangle-alert");
    }
  }

  /** Taxpayer or branch switched: drop every edit in progress. */
  reset() {
    this.deleteArmed = false;
    this.editSec = null;
    this.tinError = null;
    this.panelEdit = null;
    this.forms = null;
    this.addYear = false;
    this.inbox = null;
    this.inboxMsg = null;
    this.inboxBusy = false;
  }

  /** Year switched: staged form edits and the add-year band are dropped. */
  leaveYear() {
    this.forms = null;
    this.addYear = false;
  }

  leaveView() {
    this.editSec = null;
    this.tinError = null;
    this.forms = null;
    this.addYear = false;
  }

  // ------------------------------------------------------------ profile sections

  toggleSection(id, cx) {
    this.open[id] = !this.open[id];
    if (!this.open[id] && this.editSec === id) this.editSec = null;
    cx.notify();
  }

  startEdit(sec, cx) {
    const t = this.app.taxpayer();
    const b = this.app.branch();
    this.editSec = sec;
    this.tinError = null;
    this.factErrors = {};
    this.inputs = {};
    this.inherit = {};
    if (sec === "taxpayer") {
      this.inputs.name = InputState.new({ value: t.name });
      this.inputs.tin = new DigitEntry([3, 3, 3], t.tin_root);
      this.typeValue = t.taxpayer_type;
      this.typeOpen = false;
    } else if (sec === "branch") {
      // The branch code is edited here (the taxpayer's TIN shows 3 · 3 · 3).
      if (!b.is_head_office) this.inputs.code = new DigitEntry([5], b.code);
      this.inputs.name = InputState.new({ value: b.name });
      this.inputs.rdo_code = factState(this.app, "rdo_code", b.facts.rdo_code || "");
      this.inherit.rdo_code = (b.inherited || []).includes("rdo_code");
    } else if (sec === "address") {
      for (const [f] of ADDRESS_FIELDS) {
        this.inputs[f] = factState(this.app, f, b.facts[f] || "");
        this.inherit[f] = (b.inherited || []).includes(f);
      }
    }
    cx.notify();
  }

  cancelEdit(cx) {
    this.editSec = null;
    this.tinError = null;
    cx.notify();
  }

  saveSection(sec, cx) {
    const app = this.app;
    const t = app.taxpayer();
    const b = app.branch();
    this.factErrors = {};
    for (const f of ["rdo_code", ...ADDRESS_FIELDS.map(([f]) => f)]) {
      const state = this.inputs[f];
      if (!state || (!b.is_head_office && this.inherit[f])) continue;
      const error = factError(f, state);
      if (error) this.factErrors[f] = error;
    }
    if (Object.keys(this.factErrors).length) return cx.notify();
    try {
      if (sec === "taxpayer") {
        const digits = this.inputs.tin.value();
        if (digits.length !== 9) {
          this.tinError = "TIN is 9 digits, like 123-456-789.";
          return cx.notify();
        }
        const name = this.inputs.name.value().trim();
        let toastText = "Changes saved";
        // Name and type first, under the TIN they were opened with: the move
        // is the step a TIN in use refuses, and a refusal must not leave a
        // moved TIN with the rest unsaved.
        app.store.write("update_taxpayer", t.tin_root, { name: name || t.name, taxpayer_type: this.typeValue || t.taxpayer_type });
        if (digits !== t.tin_root) {
          // One transaction in bir-core: branches, tax years, returns and
          // templates move together; a TIN in use is refused there.
          try {
            app.store.write("move_taxpayer_tin", t.tin_root, digits);
          } catch (e) {
            this.tinError = `Name and type saved. The TIN did not change: ${message(e)}`;
            return cx.notify();
          }
          app.remapRecent((tin) => (tin.startsWith(t.tin_root) ? digits + tin.slice(9) : tin));
          app.selRoot = digits;
          toastText = `TIN changed to ${formatTin(digits)}. Every branch, tax year, return and template moved with it.`;
        }
        app.writeLocal(SELECTION_KEY, { root: app.selRoot, code: app.selCode });
        this.editSec = null;
        this.tinError = null;
        return app.toast(toastText, cx);
      }
      if (sec === "branch") {
        if (this.inputs.code) {
          const error = this.moveBranchCode(this.inputs.code.value());
          if (error) {
            this.tinError = error;
            return cx.notify();
          }
        }
        const current = app.branch();
        const fields = {};
        if (!current.is_head_office && this.inherit.rdo_code) fields.rdo_code = null;
        else fields.rdo_code = this.inputs.rdo_code.value().trim();
        app.store.write("update_branch", current.tin, { name: this.inputs.name.value().trim() || current.name, fields });
      } else if (sec === "address") {
        const fields = {};
        for (const [f] of ADDRESS_FIELDS) {
          fields[f] = !b.is_head_office && this.inherit[f] ? null : this.inputs[f].value().trim();
        }
        app.store.write("update_branch", b.tin, { fields });
      }
      this.editSec = null;
      app.toast("Changes saved", cx);
    } catch (e) {
      this.tinError = null;
      app.toast(message(e), cx, "triangle-alert");
    }
  }

  setInherit(field, on, cx) {
    this.inherit[field] = on;
    if (on) {
      const head = this.app.taxpayer().branches.find((x) => x.is_head_office);
      if (head && this.inputs[field]) this.inputs[field].set_value(head.facts[field] || "");
    }
    cx.notify();
  }

  // ------------------------------------------------------------ right panel edit

  startPanelEdit(cx) {
    const t = this.app.taxpayer();
    const b = this.app.branch();
    this.panelEdit = {
      name: InputState.new({ value: t.name }),
      code: new DigitEntry([5], b.code),
      branchName: InputState.new({ value: b.name }),
      address: InputState.new({ value: b.facts.registered_address || "" }),
      error: null,
    };
    cx.notify();
  }

  /**
   * Move the selected branch to a new 5-digit code (returns, years and
   * templates move with it in one bir-core transaction). Returns an error
   * message, or null when moved or unchanged.
   */
  moveBranchCode(code) {
    const app = this.app;
    const t = app.taxpayer();
    const b = app.branch();
    if (b.is_head_office || code === b.code) return null;
    if (!/^\d{5}$/.test(code)) return "Branch code is 5 digits, like 00001.";
    if (code === "00000" || t.branches.some((x) => x.code === code)) return `Branch ${code} already exists.`;
    const oldTin = b.tin;
    app.store.write("move_branch_code", b.tin, code);
    app.remapRecent((tin) => (tin === oldTin ? t.tin_root + code : tin));
    app.selCode = code;
    app.writeLocal(SELECTION_KEY, { root: app.selRoot, code });
    return null;
  }

  cancelPanelEdit(cx) {
    this.panelEdit = null;
    cx.notify();
  }

  savePanelEdit(cx) {
    const app = this.app;
    const t = app.taxpayer();
    const b = app.branch();
    const e = this.panelEdit;
    try {
      if (!b.is_head_office) {
        const error = this.moveBranchCode(e.code.value());
        if (error) {
          e.error = error;
          return cx.notify();
        }
      }
      const name = e.name.value().trim();
      if (name && name !== t.name) app.store.write("update_taxpayer", t.tin_root, { name });
      const fields = {};
      const address = e.address.value().trim();
      const current = app.branch();
      if (address !== (current.facts.registered_address || "")) fields.registered_address = address;
      app.store.write("update_branch", current.tin, { name: e.branchName.value().trim() || current.name, fields });
      this.panelEdit = null;
      app.toast("Tax profile saved", cx);
    } catch (err) {
      e.error = message(err);
      cx.notify();
    }
  }

  // ------------------------------------------------------------ confirmation inbox (P2-5)

  /** The mailbox this branch's BIR receipts are tracked through. */
  inboxState() {
    return this.app.store.inboxState(this.app.branch().tin);
  }

  startInboxEdit(cx) {
    const s = this.inboxState();
    const password = InputState.new({
      placeholder: s.has_app_password ? "Leave empty to keep the stored password" : "16-letter app password",
    });
    password.set_masked(true);
    this.inbox = {
      method: s.auth_method === "GoogleOAuth" ? "GoogleOAuth" : "AppPassword",
      email: InputState.new({ value: s.imap_login || "" }),
      host: InputState.new({ value: s.imap_host || "imap.gmail.com" }),
      password,
      error: null,
    };
    cx.notify();
  }

  cancelInboxEdit(cx) {
    this.inbox = null;
    cx.notify();
  }

  /** Stage the method pick without saving; it applies on the next write. */
  pickInboxMethod(method, cx) {
    if (!this.inbox) this.startInboxEdit(cx);
    this.inbox.method = method;
    cx.notify();
  }

  saveInbox(cx) {
    const app = this.app;
    const ib = this.inbox;
    // OAuth "saving" is the Connect button's grant; only App Password has a
    // credentials form to persist.
    if (!ib || ib.method !== "AppPassword") return;
    const email = ib.email.value().trim();
    if (!EMAIL_RE.test(email)) {
      ib.error = "Type an email like name@example.com.";
      return cx.notify();
    }
    try {
      app.store.write("set_inbox_app_password", app.branch().tin, {
        imap_email: email,
        imap_host: ib.host.value().trim() || null,
        app_password: ib.password.value().trim() || null,
      });
      this.inbox = null;
      app.toast("Inbox credentials saved. Verify the connection to start tracking.", cx);
    } catch (e) {
      ib.error = message(e);
      cx.notify();
    }
  }

  inboxToggleTracking(on, cx) {
    const app = this.app;
    try {
      app.store.write("set_inbox_tracking", app.branch().tin, on);
      app.toast(on ? "Automated receipt tracking on." : "Automated receipt tracking off.", cx);
    } catch (e) {
      app.toast(message(e), cx, "triangle-alert");
    }
  }

  inboxDisconnect(cx) {
    const app = this.app;
    try {
      app.store.write("disconnect_inbox_oauth", app.branch().tin);
      app.toast("Google account disconnected for every profile sharing this inbox.", cx);
    } catch (e) {
      app.toast(message(e), cx, "triangle-alert");
    }
  }

  /** Browser OAuth leg runs on a host thread; the click's cx ends with the click. */
  inboxConnect(cx) {
    const app = this.app;
    const tin = app.branch().tin;
    if (this.inboxBusy) return;
    this.inboxBusy = true;
    this.inboxMsg = { ok: true, text: "Google sign-in opened in your browser…" };
    cx.notify();
    cx.spawn(async (acx) => {
      try {
        await app.store.call("connect_inbox_oauth", tin);
        app.store.bump();
        this.inboxMsg = null;
        app.toast("Google account connected. Email tracking is on.", acx, "key-round");
      } catch (e) {
        this.inboxMsg = { ok: false, text: message(e) };
        app.toast(message(e), acx, "triangle-alert");
      }
      this.inboxBusy = false;
      acx.notify();
    });
  }

  verifyInbox(cx) {
    const app = this.app;
    const tin = app.branch().tin;
    if (this.inboxBusy) return;
    this.inboxBusy = true;
    this.inboxMsg = { ok: true, text: "Testing connection…" };
    cx.notify();
    cx.spawn(async (acx) => {
      try {
        const r = await app.store.call("test_inbox_connection", tin);
        app.store.bump();
        this.inboxMsg = { ok: r.ok, text: r.message };
        if (!r.ok) app.store.logScreen("error", `inbox verify: ${r.message}`);
        app.toast(r.message, acx, r.ok ? "circle-check" : "triangle-alert");
      } catch (e) {
        this.inboxMsg = { ok: false, text: message(e) };
        app.toast(message(e), acx, "triangle-alert");
      }
      this.inboxBusy = false;
      acx.notify();
    });
  }

  checkInboxNow(cx) {
    const app = this.app;
    const tin = app.branch().tin;
    if (this.inboxBusy) return;
    this.inboxBusy = true;
    this.inboxMsg = { ok: true, text: "Checking the inbox for BIR confirmations…" };
    cx.notify();
    cx.spawn(async (acx) => {
      try {
        const r = await app.store.call("check_inbox_now", tin);
        app.store.bump();
        this.inboxMsg = r.ok
          ? { ok: true, text: r.still_pending ? "Checked. A submitted return is still waiting for its confirmation." : "Checked. Everything submitted is confirmed." }
          : { ok: false, text: r.error || "The check failed." };
        if (!r.ok) app.store.logScreen("error", `inbox check: ${this.inboxMsg.text}`);
        app.toast(this.inboxMsg.text, acx, r.ok ? "circle-check" : "triangle-alert");
      } catch (e) {
        this.inboxMsg = { ok: false, text: message(e) };
        app.toast(message(e), acx, "triangle-alert");
      }
      this.inboxBusy = false;
      acx.notify();
    });
  }

  // ------------------------------------------------------------ tax years

  allowedYears() {
    const { min, max } = this.app.store.yearBounds();
    const have = this.app.years();
    const out = [];
    for (let y = max; y >= min; y--) if (!have.includes(y)) out.push(y);
    return out;
  }

  openAddYear(cx) {
    this.addYear = !this.addYear;
    this.addYearSel = null;
    this.addYearCopy = true;
    this.forms = null;
    cx.notify();
  }

  addTaxYear(cx) {
    const app = this.app;
    const b = app.branch();
    const ys = app.years();
    const year = this.addYearSel;
    try {
      const copy = ys.length > 0 && this.addYearCopy;
      const r = app.store.write("add_tax_year", b.tin, year, copy);
      app.selYear = year;
      this.addYear = false;
      this.forms = null;
      const n = (r.forms || []).length;
      app.toast(
        r.copied_from ? `${year} added with ${n} form${n === 1 ? "" : "s"} from ${r.copied_from}.` : `${year} added. Choose the forms it files.`,
        cx,
      );
    } catch (e) {
      app.toast(message(e), cx, "triangle-alert");
    }
  }

  // ------------------------------------------------------------ forms (staged)

  editForms(cx) {
    this.forms = new Set(this.app.yearForms());
    this.formsQuery.set_value("");
    this.formsFreq = "All";
    this.addYear = false;
    if (this.app.view !== "forms") this.app.go("forms", cx);
    else cx.notify();
  }

  toggleForm(code, on, cx) {
    if (on) this.forms.add(code);
    else this.forms.delete(code);
    cx.notify();
  }

  applyPack(pack, cx) {
    for (const code of pack.forms) this.forms.add(code);
    this.app.toast(`${pack.label} forms selected. Save to keep them.`, cx);
  }

  saveForms(cx) {
    const app = this.app;
    const year = app.year();
    const before = new Set(app.yearForms());
    const after = this.forms;
    const added = [...after].filter((c) => !before.has(c)).length;
    const removed = [...before].filter((c) => !after.has(c)).length;
    try {
      app.store.write("set_yearly_forms", app.branch().tin, year, [...after]);
      this.forms = null;
      app.toast(`${year} forms updated: ${added} added, ${removed} removed.`, cx);
    } catch (e) {
      app.toast(message(e), cx, "triangle-alert");
    }
  }

  cancelForms(cx) {
    this.forms = null;
    cx.notify();
  }

  // ------------------------------------------------------------ templates

  makeTemplate(code, cx) {
    const app = this.app;
    try {
      app.store.write("create_template_from_latest", app.branch().tin, code);
      app.toast(`${code} template saved from the newest return.`, cx, "layout-template");
    } catch (e) {
      const text = message(e);
      const shown = /no field classification/i.test(text)
        ? `Templates for ${code} are not available yet. So far only 1601C and 2551Q returns can keep their setup answers.`
        : /no return|not ?found|no .*draft/i.test(text)
          ? `Open and save a ${code} return first. The template keeps its setup answers.`
          : text;
      app.toast(shown, cx, "triangle-alert");
    }
  }

  deleteTemplate(code, cx) {
    const app = this.app;
    try {
      app.store.write("delete_form_template", app.branch().tin, code);
      app.toast(`${code} template deleted.`, cx, "trash");
    } catch (e) {
      app.toast(message(e), cx, "triangle-alert");
    }
  }

  /** D17: "Use as template" on a filed return. */
  useAsTemplate(row, cx) {
    const app = this.app;
    try {
      app.store.write("create_template_from_return", row.id);
      app.toast(`${row.form_code} template saved from ${row.period_label}.`, cx, "layout-template");
    } catch (e) {
      app.toast(message(e), cx, "triangle-alert");
    }
  }

  // ------------------------------------------------------------ new taxpayer / branch

  openNewTaxpayer(cx) {
    const app = this.app;
    const n = {
      tin: new DigitEntry([3, 3, 3]),
      name: InputState.new({ placeholder: "As registered with BIR" }),
      type: "Individual",
      typeOpen: false,
      registered_address: InputState.new({}),
      zip_code: factState(app, "zip_code", ""),
      rdo_code: factState(app, "rdo_code", ""),
      phone: InputState.new({}),
      email: InputState.new({ placeholder: "name@example.com" }),
      error: null,
      // A field shows its error once it was left (or filled, for the TIN),
      // and every field does after the first Create.
      touched: new Set(),
      submitted: false,
      /** Errors the host returned on Create, by field. @type {Record<string, string>} */
      hostErrors: {},
    };
    const edited = (key) => (cx) => {
      delete n.hostErrors[key];
      n.error = null;
      cx.notify();
    };
    n.tin.onChange = (cx) => {
      if (n.tin.isComplete()) n.touched.add("tin");
      edited("tin")(cx);
    };
    for (const key of ["name", "registered_address", "phone", "email"]) {
      n[key].on("change", (_e, cx) => edited(key)(cx));
      n[key].on("blur", (_e, cx) => {
        n.touched.add(key);
        cx.notify();
      });
    }
    for (const key of ["zip_code", "rdo_code"]) {
      n[key].onChange = edited(key);
      n[key].input.on("blur", (_e, cx) => {
        n.touched.add(key);
        cx.notify();
      });
    }
    this.newTp = n;
    app.panel = null;
    app.go("new_taxpayer", cx);
  }

  /** Every problem with the new taxpayer as typed, by field. */
  newTpProblems() {
    const n = this.newTp;
    const e = {};
    const digits = n.tin.value();
    if (digits.length !== 9) e.tin = "TIN is 9 digits, like 123-456-789.";
    else {
      const taken = this.app.store.taxpayers().find((t) => t.tin_root === digits);
      if (taken) e.tin = `TIN ${formatTin(digits)} is already ${taken.name}.`;
    }
    if (!n.name.value().trim()) e.name = "Type the registered name.";
    for (const f of ["zip_code", "rdo_code", "email"]) {
      const error = factError(f, n[f]);
      if (error) e[f] = error;
    }
    return e;
  }

  /** The errors to show now: touched fields, all after Create, and the host's. */
  newTpErrors() {
    const n = this.newTp;
    const all = this.newTpProblems();
    const shown = { ...n.hostErrors };
    for (const [k, v] of Object.entries(all)) {
      // A TIN already in use is worth saying as soon as it is typed.
      const duplicate = k === "tin" && n.tin.isComplete();
      if (n.submitted || n.touched.has(k) || duplicate) shown[k] = shown[k] || v;
    }
    return shown;
  }

  createTaxpayer(cx) {
    const app = this.app;
    const n = this.newTp;
    n.submitted = true;
    n.error = null;
    if (Object.keys(this.newTpProblems()).length) return cx.notify();
    const digits = n.tin.value();
    const name = n.name.value().trim();
    try {
      app.store.write("create_taxpayer", {
        tin_root: digits,
        name,
        taxpayer_type: n.type,
        head_office: {
          registered_address: n.registered_address.value().trim(),
          zip_code: n.zip_code.value().trim(),
          rdo_code: n.rdo_code.value().trim(),
          phone: n.phone.value().trim(),
          email: n.email.value().trim(),
        },
      });
      this.newTp = null;
      // First launch chose "Just me": this taxpayer is the owner, the admin.
      app.finishSoloSetup(digits);
      app.selRoot = digits;
      app.selCode = "00000";
      app.selYear = null;
      app.writeLocal(SELECTION_KEY, { root: digits, code: "00000" });
      this.reset();
      this.addYear = true;
      app.go("forms", cx);
      app.toast(`${name} added. Add a tax year to choose its forms.`, cx);
    } catch (e) {
      const text = message(e);
      if (/\bTIN\b/i.test(text)) n.hostErrors.tin = text;
      else n.error = text;
      cx.notify();
    }
  }

  openNewBranch(cx) {
    const t = this.app.taxpayer();
    let suggestion = "";
    try {
      suggestion = this.app.store.call("suggest_next_branch_code", t.tin_root);
    } catch (_e) {}
    const head = t.branches.find((b) => b.is_head_office) || t.branches[0];
    this.newBranch = {
      code: new DigitEntry([5], suggestion),
      name: InputState.new({ placeholder: "Cebu branch" }),
      inputs: {},
      inherit: {},
      error: null,
      /** @type {Record<string, string>} */
      errors: {},
    };
    for (const f of ["rdo_code", ...ADDRESS_FIELDS.map(([f]) => f)]) {
      this.newBranch.inputs[f] = factState(this.app, f, head ? head.facts[f] || "" : "");
      this.newBranch.inherit[f] = Boolean(head);
    }
    this.app.panel = null;
    this.app.go("new_branch", cx);
  }

  createBranch(cx) {
    const app = this.app;
    const nb = this.newBranch;
    const code = nb.code.value();
    const name = nb.name.value().trim();
    nb.error = null;
    nb.errors = {};
    if (!/^\d{5}$/.test(code)) nb.errors.code = "Branch code is 5 digits, like 00001.";
    if (!name) nb.errors.name = "Type a branch name, like Cebu branch.";
    for (const f of Object.keys(nb.inputs)) {
      const error = nb.inherit[f] ? null : factError(f, nb.inputs[f]);
      if (error) nb.errors[f] = error;
    }
    if (Object.keys(nb.errors).length) return cx.notify();
    const fields = {};
    for (const f of Object.keys(nb.inputs)) fields[f] = nb.inherit[f] ? null : nb.inputs[f].value().trim();
    try {
      app.store.write("create_branch", app.taxpayer().tin_root, { branch_code: code, name, fields });
      this.newBranch = null;
      app.selCode = code;
      app.selYear = null;
      app.writeLocal(SELECTION_KEY, { root: app.selRoot, code });
      app.go("profile", cx);
      app.toast(`${name} added.`, cx);
    } catch (e) {
      const text = message(e);
      if (/branch code/i.test(text)) nb.errors.code = text;
      else nb.error = text;
      cx.notify();
    }
  }
}

// ---------------------------------------------------------------- header

function header(cx, app, view) {
  const t = app.taxpayer();
  const b = app.branch();
  const tin = formatTin(t.tin_root, b.code);
  return div()
    .flex()
    .flex_col()
    .child(
      PageHeader(cx, {
        title: t.name,
        avatar: Avatar(cx, t.name, 40, t.tin_root),
        meta: MetaLine(cx, [Code(cx, tin, "fg2").font_weight(400), b.name, t.taxpayer_type]),
        actions: [
          AppButton(cx, "tp-copy-tin", "Copy TIN", {
            size: "sm",
            icon: "copy",
            onClick: (_e, cx) => {
              cx.write_to_clipboard(tin);
              app.toast(`Copied ${tin}`, cx, "copy");
            },
          }),
          AppButton(cx, "tp-new-return", "New return", { variant: "primary", size: "sm", icon: "plus", onClick: (_e, cx) => app.startNewReturn(cx) }),
        ],
      }),
    )
    .child(
      Tabs(
        cx,
        "tp-tabs",
        [
          { value: "profile", label: "Profile" },
          { value: "forms", label: "Forms", count: app.yearForms().length },
          { value: "templates", label: "Templates", count: app.templates().length },
        ],
        view,
        (v, cx) => app.go(v, cx),
      ),
    );
}

// ---------------------------------------------------------------- profile tab

function profileTab(cx, app, width) {
  const ui = app.taxpayerUi;
  const t = app.taxpayer();
  const b = app.branch();
  const head = t.branches.find((x) => x.is_head_office);
  const inner = width - 36 - 20 - 2; // disclosure body padding (36 left, 20 right) and borders
  const inherited = (f) => !b.is_head_office && (b.inherited || []).includes(f);

  const field = (sec, f, label, value, opts = {}) => {
    const editing = ui.editSec === sec && !opts.readonly;
    const inheritable = opts.inheritable && !b.is_head_office && head;
    let input = null;
    if (editing && ui.inputs[f] instanceof DigitEntry) {
      input = DigitCells(cx, ui.inputs[f], `tp-${sec}-${f}`, { error: Boolean(ui.tinError) });
    } else if (editing && ui.inputs[f] instanceof ComboState) {
      input = ComboField(cx, ui.inputs[f], {
        id: `tp-${sec}-${f}`,
        label,
        disabled: inheritable && ui.inherit[f],
        error: Boolean(ui.factErrors[f]),
      });
    } else if (editing) {
      input = TextField(cx, ui.inputs[f], {
        id: `tp-${sec}-${f}`,
        label,
        disabled: inheritable && ui.inherit[f],
        error: (f === "tin" && Boolean(ui.tinError)) || (editing && Boolean(ui.factErrors[f])),
      });
    }
    return {
      full: opts.full,
      el: Field(cx, label, {
        value,
        input,
        error: f === "tin" || f === "code" ? ui.tinError : editing ? ui.factErrors[f] || null : null,
        inherited: !editing && inheritable && inherited(f),
        inheritSwitch:
          editing && inheritable
            ? Switch(cx, `tp-inherit-${f}`, Boolean(ui.inherit[f]), (v, cx) => ui.setInherit(f, v, cx), `${label}: same as head office`)
            : null,
      }),
    };
  };

  const editAction = (sec) => (ui.editSec === sec ? FormActions(cx, `tp-${sec}`, (_e, cx) => ui.cancelEdit(cx), (_e, cx) => ui.saveSection(sec, cx)) : div());

  // Taxpayer (shared by every branch).
  const typeCell =
    ui.editSec === "taxpayer"
      ? {
          el: Field(cx, "Taxpayer type", {
            input: Dropdown(cx, "tp-type", {
              label: "Taxpayer type",
              width: Math.min(260, inner),
              options: TAXPAYER_TYPES.map((x) => ({ value: x, label: x })),
              value: ui.typeValue,
              open: ui.typeOpen,
              onOpenChange: (open, cx) => {
                ui.typeOpen = open;
                cx.notify();
              },
              onChange: (v, cx) => {
                ui.typeValue = v;
                cx.notify();
              },
            }),
          }),
        }
      : { el: Field(cx, "Taxpayer type", { value: t.taxpayer_type }) };
  const taxpayerBody = div()
    .flex()
    .flex_col()
    .child(
      FieldGrid(cx, inner, [
        field("taxpayer", "name", "Registered name", t.name, { full: true }),
        field("taxpayer", "tin", "TIN", formatTin(t.tin_root)),
        typeCell,
      ]),
    );
  if (ui.editSec === "taxpayer") {
    taxpayerBody.child(Text(cx, "meta", "Changing the TIN moves every branch, tax year, return and template to the new number.", "fg3").mt(SPACE[4]).whitespace_normal());
  }
  taxpayerBody.child(editAction("taxpayer"));

  const branchBody = div()
    .flex()
    .flex_col()
    .child(
      FieldGrid(cx, inner, [
        field("branch", "code", "Branch code", b.code, { readonly: b.is_head_office }),
        field("branch", "name", "Branch name", b.name),
        field("branch", "rdo_code", "RDO", b.facts.rdo_code, { inheritable: true }),
      ]),
    )
    .child(editAction("branch"));

  const inhCount = ADDRESS_FIELDS.filter(([f]) => inherited(f)).length;
  const addressBody = div()
    .flex()
    .flex_col()
    .child(
      FieldGrid(
        cx,
        inner,
        ADDRESS_FIELDS.map(([f, label, full]) => field("address", f, label, b.facts[f], { inheritable: true, full })),
      ),
    )
    .child(editAction("address"));

  const section = (id, title, meta, body, editable) =>
    Disclosure(cx, `tp-sec-${id}`, {
      title,
      meta,
      open: ui.open[id],
      editing: ui.editSec === id,
      onToggle: (_e, cx) => ui.toggleSection(id, cx),
      onEdit: editable ? (_e, cx) => ui.startEdit(id, cx) : null,
      body,
    });

  const ib = app.store.inboxState(b.tin);
  const inboxMeta = ib
    ? ib.tracking_enabled
      ? `Tracking on · ${ib.inbox_email}`
      : ib.oauth_connected || ib.has_app_password
        ? `Connected · ${ib.inbox_email || ib.oauth_inbox_email || ""}`
        : "Not connected"
    : "Not connected";
  const group = DisclosureGroup(cx, "tp-profile", [
    section("taxpayer", "Taxpayer", "Shared by every branch", taxpayerBody, true),
    section("branch", "Branch", b.is_head_office ? "Head office" : `Branch ${b.code}`, branchBody, true),
    section(
      "address",
      "Address and contact",
      b.is_head_office ? "Used on every return" : inhCount ? `${inhCount} of 4 from head office` : "All set for this branch",
      addressBody,
      true,
    ),
    section("inbox", "Confirmation inbox", inboxMeta, inboxBody(cx, app, inner), false),
  ]);
  const out = div().flex().flex_col().child(group);
  if (ui.canRemove()) out.child(removeCard(cx, app, t));
  return out;
}

/**
 * BIR receipt tracking for this branch's mailbox (P2-5): the auth method,
 * credentials, OAuth connect/disconnect, verify, the tracking switch and a
 * user-triggered "check now" — all through bir-core's existing fetch→confirm
 * path. Secrets never render: the password field is masked and the host
 * answers only `has_app_password`.
 */
function inboxBody(cx, app, inner) {
  const ui = app.taxpayerUi;
  const s = app.store.inboxState(app.branch().tin);
  const editing = Boolean(ui.inbox);
  const method = editing ? ui.inbox.method : s.auth_method;

  const cells = [
    {
      full: true,
      el: Field(cx, "Authentication method", {
        input: Segmented(
          cx,
          "tp-inbox-method",
          [
            { value: "GoogleOAuth", label: "Google account (OAuth2)" },
            { value: "AppPassword", label: "App password" },
          ],
          method,
          (v, cx) => ui.pickInboxMethod(v, cx),
          { label: "Inbox authentication method" },
        ),
      }),
    },
  ];
  if (method === "AppPassword") {
    cells.push({
      el: Field(cx, "Inbox email", {
        value: s.imap_login || s.inbox_email || "Not set",
        input: editing ? TextField(cx, ui.inbox.email, { id: "tp-inbox-email", label: "Inbox email" }) : null,
      }),
    });
    cells.push({
      el: Field(cx, "IMAP host", {
        value: s.imap_host,
        input: editing ? TextField(cx, ui.inbox.host, { id: "tp-inbox-host", label: "IMAP host" }) : null,
      }),
    });
    cells.push({
      full: true,
      el: Field(cx, "App password", {
        value: s.has_app_password ? "Stored" : "Not set",
        input: editing ? TextField(cx, ui.inbox.password, { id: "tp-inbox-password", label: "App password", error: Boolean(ui.inbox.error) }) : null,
        error: editing ? ui.inbox.error : null,
      }),
    });
  } else {
    cells.push({
      full: true,
      el: Field(cx, "Google account", {
        value: s.oauth_connected ? `Connected as ${s.oauth_inbox_email || s.inbox_email}` : "Not connected",
      }),
    });
  }
  if (s.last_check_at) {
    cells.push({
      el: Field(cx, "Last check", {
        value: `${shortDate(s.last_check_at)}${s.last_check_status ? ` — ${s.last_check_status}` : ""}`,
      }),
    });
  }
  const body = div().flex().flex_col().child(FieldGrid(cx, inner, cells));

  if (method === "GoogleOAuth") {
    body.child(
      Text(
        cx,
        "meta",
        "Profiles that share this inbox share one Google OAuth credential set. Re-authorize or Disconnect updates every profile using this email.",
        "fg3",
      )
        .mt(SPACE[2])
        .whitespace_normal(),
    );
  }
  if (method === "AppPassword" && editing) {
    body.child(
      Text(
        cx,
        "meta",
        "An app password needs 2-Step Verification on the mail account first (Google / Outlook / Yahoo security settings).",
        "fg3",
      )
        .mt(SPACE[2])
        .whitespace_normal(),
    );
  }

  const actions = div().flex().flex_row().flex_wrap().items_center().gap(SPACE[2]).mt(SPACE[3]);
  if (editing) {
    actions.child(AppButton(cx, "tp-inbox-cancel", "Cancel", { variant: "ghost", size: "sm", onClick: (_e, cx) => ui.cancelInboxEdit(cx) }));
    if (method === "AppPassword") {
      actions.child(AppButton(cx, "tp-inbox-save", "Save", { variant: "primary", size: "sm", icon: "save", onClick: (_e, cx) => ui.saveInbox(cx) }));
    }
  } else if (method === "AppPassword") {
    actions.child(AppButton(cx, "tp-inbox-edit", "Edit credentials", { variant: "ghost", size: "sm", icon: "pencil", onClick: (_e, cx) => ui.startInboxEdit(cx) }));
  }
  actions.child(
    AppButton(cx, "tp-inbox-verify", "Verify connection", {
      size: "sm",
      onClick: (_e, cx) => ui.verifyInbox(cx),
    }),
  );
  actions.child(
    AppButton(cx, "tp-inbox-check", "Check now", {
      variant: "ghost",
      size: "sm",
      icon: "search",
      onClick: (_e, cx) => ui.checkInboxNow(cx),
    }),
  );
  body.child(actions);
  body.child(
    Text(
      cx,
      "meta",
      "Connect, disconnect, and receipt tracking for this inbox are in Settings, under Account.",
      "fg3",
    )
      .mt(SPACE[2])
      .whitespace_normal(),
  );

  if (ui.inboxMsg) {
    body.child(
      Text(cx, "body", ui.inboxMsg.text, ui.inboxMsg.ok ? "success" : "danger")
        .mt(SPACE[2])
        .whitespace_normal(),
    );
  }
  body.child(
    Text(
      cx,
      "meta",
      s.tracking_enabled
        ? s.still_pending
          ? "● Automated BIR receipt tracking is active — a submitted return is still waiting for its confirmation."
          : "● Automated BIR receipt tracking is active."
        : "○ Verify your connection to activate email tracking.",
      s.tracking_enabled ? "success" : "fg3",
    )
      .mt(SPACE[2])
      .whitespace_normal(),
  );
  return body;
}

/**
 * Export card: everyone on the page may save this profile's credential-free
 * archive. A taxpayer PIN, when one is set, is typed into PIN cells and sent
 * to the host's export_profile verb; the cells are cleared by exportProfile
 * after every attempt, and the save dialog itself is the host's.
 */
/** Profile export, drawn in Settings › Import / Export. The save dialog is the host's. */
export function profileExportCard(cx, app, tp) {
  const ui = app.taxpayerUi;
  const hasPin = Boolean(app.taxpayerCreds(tp.tin_root).has_pin);
  return Card(cx, "tp-export", [
    SettingRow(
      cx,
      "tp-export",
      "Export this profile",
      hasPin ? "The archive leaves out credentials; the profile PIN gates saving it" : "Saves a credential-free archive of every branch and return",
      div()
        .flex()
        .flex_row()
        .items_center()
        .gap(SPACE[2])
        .when(hasPin, (el) => el.child(PinCells(cx, ui.exportOtp, "tp-export-pin")))
        .child(AppButton(cx, "tp-export-btn", "Export", { size: "sm", icon: "download", onClick: (_e, cx) => ui.exportProfile(cx) })),
    ),
  ]);
}

/**
 * Archive or delete, at the foot of the profile: the admin's alone, and
 * never for the admin's own account, so the way back in cannot be removed.
 */
function removeCard(cx, app, tp) {
  const ui = app.taxpayerUi;
  return div()
    .flex()
    .flex_col()
    .child(SectionTitle(cx, "Archive or delete"))
    .child(
      Card(cx, "tp-remove", [
        SettingRow(
          cx,
          "tp-archive",
          tp.archived ? "Archived" : "Archive this taxpayer",
          tp.archived ? "Kept with its returns, out of new work. Unarchive to file for it again" : "Keeps every return; it just leaves new work",
          AppButton(cx, "tp-archive-btn", tp.archived ? "Unarchive" : "Archive", { size: "sm", icon: "archive", onClick: (_e, cx) => ui.toggleArchive(cx) }),
        ),
        SettingRow(
          cx,
          "tp-delete",
          "Delete this taxpayer",
          ui.deleteArmed ? "Every branch and return goes. This cannot be undone" : "Removes it and every branch and return",
          div()
            .flex()
            .flex_row()
            .gap(SPACE[2])
            .when(ui.deleteArmed, (el) =>
              el.child(AppButton(cx, "tp-delete-cancel", "Keep it", { variant: "ghost", size: "sm", onClick: (_e, cx) => ui.disarmDelete(cx) })),
            )
            .child(
              AppButton(cx, "tp-delete-btn", ui.deleteArmed ? "Delete for good" : "Delete", {
                variant: "danger",
                size: "sm",
                icon: "trash",
                onClick: (_e, cx) => ui.deleteTaxpayer(cx),
              }),
            ),
        ),
      ]),
    );
}

// ---------------------------------------------------------------- forms tab

/** Next open period and its due date for a form in the selected year. */
function schedule(app, code, year) {
  const b = app.branch();
  const rows = app.store.formDeadlines(b.tin, code, year, `${year}-01-01`, `${year + 1}-12-31`);
  const open = rows.filter((d) => d.status === "Not started" || d.status === "Draft");
  const upcoming = open.find((d) => daysUntil(d.due_date) >= 0) || open[open.length - 1];
  if (!rows.length) return { next: null, due: "", none: true };
  if (!upcoming) return { next: null, due: "" };
  return { next: upcoming.period_label, due: shortDate(upcoming.due_date) };
}

function addYearBand(cx, app) {
  const ui = app.taxpayerUi;
  const ys = app.years();
  const allowed = ui.allowedYears();
  const { min, max } = app.store.yearBounds();
  if (!allowed.length) {
    return Banner(cx, "tp-add-year", [
      Text(cx, "body", `Every tax year from ${min} to ${max} is already added.`).flex_1(),
      ys.length ? AppButton(cx, "tp-add-year-cancel", "Cancel", { variant: "ghost", size: "sm", onClick: (_e, cx) => ui.openAddYear(cx) }) : div(),
    ]);
  }
  const last = ys[ys.length - 1];
  const now = new Date().getFullYear();
  if (!allowed.includes(ui.addYearSel)) {
    ui.addYearSel = last ? (allowed.includes(last + 1) ? last + 1 : allowed[0]) : allowed.includes(now) ? now : allowed[0];
  }
  const items = [
    Text(cx, "control", "Add tax year").font_weight(600),
    Dropdown(cx, "tp-add-year-select", {
      label: "Tax year to add",
      width: 96,
      options: allowed.map((y) => ({ value: y, label: String(y) })),
      value: ui.addYearSel,
      open: ui.addYearOpen,
      onOpenChange: (open, cx) => {
        ui.addYearOpen = open;
        cx.notify();
      },
      onChange: (v, cx) => {
        ui.addYearSel = v;
        cx.notify();
      },
    }),
  ];
  if (last) {
    const n = app.store.yearlyForms(app.branch().tin, last).length;
    items.push(
      CheckboxLabel(cx, "tp-add-year-copy", ui.addYearCopy, (v, cx) => {
        ui.addYearCopy = v;
        cx.notify();
      }, `Copy ${n} form${n === 1 ? "" : "s"} from ${last}`),
    );
  }
  items.push(div().flex_1());
  if (ys.length) items.push(AppButton(cx, "tp-add-year-cancel", "Cancel", { variant: "ghost", size: "sm", onClick: (_e, cx) => ui.openAddYear(cx) }));
  items.push(AppButton(cx, "tp-add-year-add", `Add ${ui.addYearSel}`, { variant: "primary", size: "sm", onClick: (_e, cx) => ui.addTaxYear(cx) }));
  return Banner(cx, "tp-add-year", items);
}

function formRow(cx, app, f, year, opts) {
  const compact = opts.width < 600;
  const tpl = app.templates().some((x) => x.form_code === f.c);
  const on = !opts.edit || opts.edit.has(f.c);
  const sc = on ? schedule(app, f.c, year) : { next: null, due: "" };
  const sub =
    opts.edit && !on ? "Not filed this year" : sc.next ? `Next: ${sc.next}` : sc.none ? `Filed in ${year}` : "Every period filed";
  const row = ListRow(cx, `tp-form-${f.c}`);
  if (opts.edit) {
    row.child(Checkbox(cx, `tp-form-${f.c}-check`, on, (v, cx) => app.taxpayerUi.toggleForm(f.c, v, cx), `File ${f.c} in ${year}`));
  }
  row
    .child(Code(cx, f.c, on ? "fg" : "fg3").w(68).flex_none())
    .child(NameCell(cx, f.t, sub, { dim: !on }));
  if (!compact) {
    row.child(div().w(84).flex_none().flex().flex_row().child(Chip(cx, f.freq)));
    row.child(Text(cx, "meta", on && sc.due ? `Due ${sc.due}` : "", "fg2").w(104).flex_none());
  }
  row.child(
    div()
      .w(compact ? 96 : 112)
      .flex_none()
      .flex()
      .flex_row()
      .child(on ? (tpl ? Chip(cx, "Template", "success", "layout-template") : Chip(cx, "No template")) : div()),
  );
  return row;
}

function formsTab(cx, app, width) {
  const ui = app.taxpayerUi;
  const b = app.branch();
  const ys = app.years();
  const year = app.year();
  if (!ys.length) {
    return div()
      .flex()
      .flex_col()
      .child(Text(cx, "body", `${b.name} has no tax year yet. Add one to choose the forms it files.`, "fg2").mb(SPACE[4]).whitespace_normal())
      .child(addYearBand(cx, app));
  }
  const edit = ui.forms;
  const right = edit
    ? [
        Text(cx, "meta", `${edit.size} selected`, "fg2"),
        AppButton(cx, "tp-forms-cancel", "Cancel", { variant: "ghost", size: "sm", onClick: (_e, cx) => ui.cancelForms(cx) }),
        AppButton(cx, "tp-forms-save", "Save forms", { variant: "primary", size: "sm", onClick: (_e, cx) => ui.saveForms(cx) }),
      ]
    : [AppButton(cx, "tp-forms-edit", "Edit forms", { size: "sm", icon: "pencil", onClick: (_e, cx) => ui.editForms(cx) })];
  const toolbar = div()
    .flex()
    .flex_row()
    .flex_wrap()
    .items_center()
    .gap_x(SPACE[3])
    .gap_y(SPACE[2])
    .mb(SPACE[4])
    .child(
      Segmented(
        cx,
        "tp-years",
        ys.map((y) => ({ value: y, label: String(y) })),
        year,
        (v, cx) => app.selectYear(v, cx),
        { label: "Tax year" },
      ),
    )
    .child(AppButton(cx, "tp-add-year-open", "Add year", { variant: "ghost", size: "sm", icon: "plus", onClick: (_e, cx) => ui.openAddYear(cx) }))
    .child(div().flex_1());
  for (const r of right) toolbar.child(r);

  const page = div().flex().flex_col().child(toolbar);
  if (ui.addYear) page.child(addYearBand(cx, app));
  const index = app.store.screenIndex();

  if (!edit) {
    const codes = new Set(app.yearForms());
    const list = index.filter((f) => codes.has(f.c));
    if (!list.length) {
      return page.child(
        ListGroup(cx, "tp-forms-list", [
          EmptyRow(
            cx,
            "tp-forms-empty",
            `No forms for ${year} yet.`,
            AppButton(cx, "tp-forms-choose", "Choose forms", { size: "sm", onClick: (_e, cx) => ui.editForms(cx) }),
          ),
        ]),
      );
    }
    return page.child(ListGroup(cx, "tp-forms-list", list.map((f) => formRow(cx, app, f, year, { width }))));
  }

  const t = tokens(cx);
  page.child(
    div()
      .flex()
      .flex_row()
      .flex_wrap()
      .items_center()
      .gap_x(SPACE[3])
      .gap_y(SPACE[2])
      .mb(SPACE[4])
      .child(
        div()
          .flex()
          .flex_row()
          .items_center()
          .gap(SPACE[2])
          .w(Math.min(280, width))
          .h(32)
          .pl(10)
          .rounded(RADIUS.row)
          .border(1)
          .border_color(t.border)
          .bg(t.surface)
          .child(Icon(cx, "search", { color: "fg3" }))
          .child(TextField(cx, ui.formsQuery, { id: "tp-forms-search", label: "Find a form" }).border(0).px(0).h(30).flex_1()),
      )
      .child(
        Segmented(
          cx,
          "tp-forms-freq",
          FREQS.map((x) => ({ value: x, label: x })),
          ui.formsFreq,
          (v, cx) => {
            ui.formsFreq = v;
            cx.notify();
          },
          { label: "Frequency" },
        ),
      ),
  );
  // Starter packs appear only in edit mode, and only ever add.
  const packs = div().flex().flex_row().flex_wrap().items_center().gap(6).mb(SPACE[4]).child(Text(cx, "meta", "Starter packs", "fg3").mr(2));
  for (const pack of app.store.starterPacks(b.tin)) {
    packs.child(AppButton(cx, `tp-pack-${pack.id}`, pack.label, { size: "sm", onClick: (_e, cx) => ui.applyPack(pack, cx) }));
  }
  page.child(packs);
  const q = ui.formsQuery.value().trim().toLowerCase();
  const list = index.filter((f) => (ui.formsFreq === "All" || f.freq === ui.formsFreq) && (!q || searchText(f).includes(q)));
  if (!list.length) {
    return page.child(ListGroup(cx, "tp-forms-list", [EmptyRow(cx, "tp-forms-nomatch", "No form matches. Try a code like 1601C or a word like VAT.")]));
  }
  return page.child(ListGroup(cx, "tp-forms-list", list.map((f) => formRow(cx, app, f, year, { width, edit }))));
}

// ---------------------------------------------------------------- templates tab

function templatesTab(cx, app, width) {
  const ui = app.taxpayerUi;
  const b = app.branch();
  const ys = app.years();
  if (!ys.length) {
    return ListGroup(cx, "tp-templates", [
      EmptyRow(
        cx,
        "tp-templates-noyear",
        `${b.name} has no tax year yet. Add one to choose the forms it files.`,
        AppButton(cx, "tp-templates-add-year", "Add year", {
          size: "sm",
          icon: "plus",
          onClick: (_e, cx) => {
            ui.addYear = true;
            app.go("forms", cx);
          },
        }),
      ),
    ]);
  }
  const year = app.year();
  const codes = new Set(app.yearForms());
  const index = app.store.screenIndex().filter((f) => codes.has(f.c));
  const templates = app.templates();
  const page = div()
    .flex()
    .flex_col()
    .child(
      Text(
        cx,
        "body",
        `One template per form for ${b.name}, kept across tax years. These are the forms it files in ${year}. A template keeps setup answers and never keeps amounts.`,
        "fg2",
      )
        .mb(SPACE[4])
        .whitespace_normal(),
    );
  if (!index.length) {
    return page.child(
      ListGroup(cx, "tp-templates", [
        EmptyRow(cx, "tp-templates-empty", `No forms for ${year} yet.`, AppButton(cx, "tp-templates-choose", "Choose forms", { size: "sm", onClick: (_e, cx) => ui.editForms(cx) })),
      ]),
    );
  }
  const compact = width < 600;
  const rows = index.map((f) => {
    const tpl = templates.find((x) => x.form_code === f.c);
    const row = ListRow(cx, `tp-template-${f.c}`)
      .child(Code(cx, f.c, tpl ? "fg" : "fg3").w(68).flex_none())
      .child(NameCell(cx, f.t, tpl ? `Updated ${shortDate(tpl.updated_at)}` : "No template yet", { dim: !tpl }));
    if (!compact) row.child(Text(cx, "meta", tpl ? `${tpl.answers_kept} answers kept` : "", "fg2").w(128).flex_none());
    const actions = div().flex().flex_row().justify_end().gap(SPACE[1]).flex_none();
    if (tpl) {
      actions
        .child(AppButton(cx, `tp-template-${f.c}-update`, "Update", { variant: "ghost", size: "sm", onClick: (_e, cx) => ui.makeTemplate(f.c, cx) }))
        .child(AppButton(cx, `tp-template-${f.c}-delete`, "Delete", { variant: "ghost", size: "sm", onClick: (_e, cx) => ui.deleteTemplate(f.c, cx) }));
    } else {
      actions.child(AppButton(cx, `tp-template-${f.c}-create`, "Create", { size: "sm", icon: "plus", onClick: (_e, cx) => ui.makeTemplate(f.c, cx) }));
    }
    return row.child(actions);
  });
  return page.child(ListGroup(cx, "tp-templates", rows));
}

export function renderTaxpayerPage(cx, app, view, width) {
  const page = div().flex().flex_col().child(header(cx, app, view));
  if (view === "forms") return page.child(formsTab(cx, app, width));
  if (view === "templates") return page.child(templatesTab(cx, app, width));
  return page.child(profileTab(cx, app, width));
}

// ---------------------------------------------------------------- new taxpayer / branch

function formCard(cx, id, width, cells, actions, error) {
  const t = tokens(cx);
  const card = div()
    .id(id)
    .flex()
    .flex_col()
    .p(SPACE[5])
    .rounded(RADIUS.card)
    .border(1)
    .border_color(t.border)
    .child(FieldGrid(cx, width - 42, cells));
  if (error) card.child(Text(cx, "meta", error, "danger").mt(SPACE[4]).whitespace_normal());
  return card.child(actions);
}

export function renderNewTaxpayer(cx, app, width) {
  const ui = app.taxpayerUi;
  if (app.view === "new_branch") return renderNewBranch(cx, app, width);
  const n = ui.newTp;
  if (!n) return div();
  const errors = ui.newTpErrors();
  const input = (key, label) => {
    const opts = { id: `new-tp-${key}`, label, error: Boolean(errors[key]) };
    const el = n[key] instanceof ComboState ? ComboField(cx, n[key], opts) : TextField(cx, n[key], opts);
    return { el: Field(cx, label, { input: el, error: errors[key] }) };
  };
  const cells = [
    { el: Field(cx, "TIN", { input: DigitCells(cx, n.tin, "new-tp-tin", { error: Boolean(errors.tin) }), error: errors.tin }) },
    {
      el: Field(cx, "Taxpayer type", {
        input: Dropdown(cx, "new-tp-type", {
          label: "Taxpayer type",
          width: 220,
          options: TAXPAYER_TYPES.map((x) => ({ value: x, label: x })),
          value: n.type,
          open: n.typeOpen,
          onOpenChange: (open, cx) => {
            n.typeOpen = open;
            cx.notify();
          },
          onChange: (v, cx) => {
            n.type = v;
            cx.notify();
          },
        }),
      }),
    },
    { ...input("name", "Registered name"), full: true },
    { ...input("registered_address", "Head-office address"), full: true },
    input("zip_code", "ZIP code"),
    input("rdo_code", "RDO"),
    input("phone", "Phone"),
    input("email", "Email"),
  ];
  return div()
    .flex()
    .flex_col()
    .child(
      PageHeader(cx, {
        title: "New taxpayer",
        lede: "The 9-digit TIN, the registered name and the head office's address. Branches inherit the address unless you change it.",
      }),
    )
    .child(
      div()
        .mt(28)
        .child(
          formCard(
            cx,
            "new-tp-card",
            width,
            cells,
            FormActions(
              cx,
              "new-tp",
              (_e, cx) => {
                ui.newTp = null;
                // Leaving first launch's "Just me" taxpayer goes back to the choice.
                if (app.setupPending) app.cancelSetup(cx);
                app.goBack(cx);
              },
              (_e, cx) => ui.createTaxpayer(cx),
              "Create taxpayer",
            ),
            n.error,
          ),
        ),
    );
}

function renderNewBranch(cx, app, width) {
  const ui = app.taxpayerUi;
  const nb = ui.newBranch;
  if (!nb) return div();
  const t = app.taxpayer();
  const labels = { rdo_code: "RDO", registered_address: "Registered address", zip_code: "ZIP code", phone: "Phone", email: "Email" };
  const errors = nb.errors || {};
  const cells = [
    { el: Field(cx, "Branch code", { input: DigitCells(cx, nb.code, "new-branch-code", { error: Boolean(errors.code) }), error: errors.code }) },
    { el: Field(cx, "Branch name", { input: TextField(cx, nb.name, { id: "new-branch-name", label: "Branch name", error: Boolean(errors.name) }), error: errors.name }) },
  ];
  for (const f of Object.keys(nb.inputs)) {
    const opts = { id: `new-branch-${f}`, label: labels[f], disabled: nb.inherit[f], error: Boolean(errors[f]) };
    cells.push({
      full: f === "registered_address" || f === "email",
      el: Field(cx, labels[f], {
        input: nb.inputs[f] instanceof ComboState ? ComboField(cx, nb.inputs[f], opts) : TextField(cx, nb.inputs[f], opts),
        error: errors[f],
        inheritSwitch: Switch(
          cx,
          `new-branch-inherit-${f}`,
          nb.inherit[f],
          (v, cx) => {
            nb.inherit[f] = v;
            cx.notify();
          },
          `${labels[f]}: same as head office`,
        ),
      }),
    });
  }
  return div()
    .flex()
    .flex_col()
    .child(PageHeader(cx, { title: "New branch", lede: `A branch of ${t.name}. It starts with the head office's details; switch any of them off to set its own.` }))
    .child(
      div()
        .mt(28)
        .child(
          formCard(
            cx,
            "new-branch-card",
            width,
            cells,
            FormActions(
              cx,
              "new-branch",
              (_e, cx) => {
                ui.newBranch = null;
                app.goBack(cx);
              },
              (_e, cx) => ui.createBranch(cx),
              "Add branch",
            ),
            nb.error,
          ),
        ),
    );
}

