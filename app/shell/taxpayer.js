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
  PinCells,
  PageHeader,
  MetaLine,
  Tabs,
  Banner,
} from "./ui.js";
import { formatTin, shortDate, daysUntil } from "./store.js";
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

/** A host error as one sentence for a toast. */
function message(e) {
  // Host errors arrive as "`bir.fn`: text"; the person needs only the text.
  return String(e && e.message ? e.message : e)
    .replace(/^Error:\s*/, "")
    .replace(/^`[^`]+`:\s*/, "");
}

function tinDigits(text) {
  return String(text || "").replace(/\D/g, "");
}

/** Owns the taxpayer screens' state and talks to the store. */
export class TaxpayerActions {
  constructor(app) {
    this.app = app;
    this.open = { taxpayer: true, branch: true, address: true, security: false };
    this.editSec = null;
    this.inputs = {};
    this.inherit = {};
    this.typeOpen = false;
    this.typeValue = null;
    this.tinError = null;
    this.panelEdit = null;
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
    // Taxpayer security (Profile › Email, security and export).
    this.pinOtp = OtpState.new(4, { masked: true });
    this.totpOtp = OtpState.new(6);
    this.totpSecret = null;
  }

  /** PIN and authenticator state of the selected branch's taxpayer profile. */
  auth() {
    const b = this.app.branch();
    return b ? this.app.store.read(`auth:${b.tin}`, "profile_auth", [b.tin], { has_pin: false, has_totp: false }) : {};
  }

  setPin(cx) {
    const app = this.app;
    const pin = this.pinOtp.value();
    if (pin.length !== 4) return app.toast("Type all 4 digits of the new PIN first.", cx, "lock");
    try {
      app.store.write("set_profile_pin", app.branch().tin, pin);
      this.pinOtp.set_value("");
      app.toast(app.store.authState().enable_profile_pins ? "PIN saved." : "PIN saved. Turn on Taxpayer PINs in Settings to ask for it.", cx, "lock");
    } catch (e) {
      app.toast(message(e), cx, "triangle-alert");
    }
  }

  clearPin(cx) {
    try {
      this.app.store.write("set_profile_pin", this.app.branch().tin, null);
      this.app.toast("PIN removed.", cx, "lock");
    } catch (e) {
      this.app.toast(message(e), cx, "triangle-alert");
    }
  }

  startTotp(cx) {
    try {
      const t = this.app.taxpayer();
      this.totpSecret = this.app.store.call("generate_totp_secret", `eBIRForms ${t.tin_display || t.tin_root}`).secret;
      this.totpOtp.set_value("");
      cx.notify();
    } catch (e) {
      this.app.toast(message(e), cx, "triangle-alert");
    }
  }

  confirmTotp(cx) {
    try {
      this.app.store.write("set_profile_totp", this.app.branch().tin, this.totpSecret, this.totpOtp.value());
      this.totpSecret = null;
      this.totpOtp.set_value("");
      this.app.toast("Authenticator added.", cx, "lock");
    } catch (e) {
      this.app.toast(message(e), cx, "triangle-alert");
    }
  }

  clearTotp(cx) {
    try {
      this.app.store.write("set_profile_totp", this.app.branch().tin, null);
      this.totpSecret = null;
      this.app.toast("Authenticator removed.", cx, "lock");
    } catch (e) {
      this.app.toast(message(e), cx, "triangle-alert");
    }
  }

  /** Taxpayer or branch switched: drop every edit in progress. */
  reset() {
    this.editSec = null;
    this.tinError = null;
    this.panelEdit = null;
    this.forms = null;
    this.addYear = false;
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
    this.inputs = {};
    this.inherit = {};
    if (sec === "taxpayer") {
      this.inputs.name = InputState.new({ value: t.name });
      this.inputs.tin = InputState.new({ value: formatTin(t.tin_root), placeholder: "000-000-000" });
      this.typeValue = t.taxpayer_type;
      this.typeOpen = false;
    } else if (sec === "branch") {
      this.inputs.name = InputState.new({ value: b.name });
      this.inputs.rdo_code = InputState.new({ value: b.facts.rdo_code || "" });
      this.inherit.rdo_code = (b.inherited || []).includes("rdo_code");
    } else if (sec === "address") {
      for (const [f] of ADDRESS_FIELDS) {
        this.inputs[f] = InputState.new({ value: b.facts[f] || "" });
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
    try {
      if (sec === "taxpayer") {
        const raw = this.inputs.tin.value().trim();
        const digits = tinDigits(raw);
        if (!/^[\d-]+$/.test(raw) || digits.length !== 9) {
          this.tinError = "TIN is 9 digits, like 123-456-789.";
          return cx.notify();
        }
        const name = this.inputs.name.value().trim();
        let toastText = "Changes saved";
        if (digits !== t.tin_root) {
          // One transaction in bir-core: branches, tax years, returns and
          // templates move together; a TIN in use is refused there.
          app.store.write("move_taxpayer_tin", t.tin_root, digits);
          app.remapRecent((tin) => (tin.startsWith(t.tin_root) ? digits + tin.slice(9) : tin));
          app.selRoot = digits;
          toastText = `TIN changed to ${formatTin(digits)}. Every branch, tax year, return and template moved with it.`;
        }
        app.store.write("update_taxpayer", digits, { name: name || t.name, taxpayer_type: this.typeValue || t.taxpayer_type });
        app.writeLocal(SELECTION_KEY, { root: app.selRoot, code: app.selCode });
        this.editSec = null;
        this.tinError = null;
        return app.toast(toastText, cx);
      }
      if (sec === "branch") {
        const fields = {};
        if (!b.is_head_office && this.inherit.rdo_code) fields.rdo_code = null;
        else fields.rdo_code = this.inputs.rdo_code.value().trim();
        app.store.write("update_branch", b.tin, { name: this.inputs.name.value().trim() || b.name, fields });
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
      code: InputState.new({ value: b.code }),
      branchName: InputState.new({ value: b.name }),
      address: InputState.new({ value: b.facts.registered_address || "" }),
      error: null,
    };
    cx.notify();
  }

  cancelPanelEdit(cx) {
    this.panelEdit = null;
    cx.notify();
  }

  savePanelEdit(cx) {
    const app = this.app;
    const t = app.taxpayer();
    let b = app.branch();
    const e = this.panelEdit;
    const code = e.code.value().trim();
    try {
      if (!b.is_head_office && code !== b.code) {
        if (!/^\d{5}$/.test(code)) {
          e.error = "Branch code is 5 digits, like 00001.";
          return cx.notify();
        }
        if (code === "00000" || t.branches.some((x) => x.code === code)) {
          e.error = `Branch ${code} already exists.`;
          return cx.notify();
        }
        const oldTin = b.tin;
        b = app.store.write("move_branch_code", b.tin, code);
        app.remapRecent((tin) => (tin === oldTin ? t.tin_root + code : tin));
        app.selCode = code;
        app.writeLocal(SELECTION_KEY, { root: app.selRoot, code });
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
    this.newTp = {
      tin: InputState.new({ placeholder: "000-000-000" }),
      name: InputState.new({ placeholder: "As registered with BIR" }),
      type: "Individual",
      typeOpen: false,
      registered_address: InputState.new({}),
      zip_code: InputState.new({}),
      rdo_code: InputState.new({ placeholder: "050" }),
      phone: InputState.new({}),
      email: InputState.new({}),
      error: null,
    };
    this.app.panel = null;
    this.app.go("new_taxpayer", cx);
  }

  createTaxpayer(cx) {
    const app = this.app;
    const n = this.newTp;
    const digits = tinDigits(n.tin.value());
    const name = n.name.value().trim();
    if (digits.length !== 9) {
      n.error = "TIN is 9 digits, like 123-456-789.";
      return cx.notify();
    }
    if (!name) {
      n.error = "Type the registered name.";
      return cx.notify();
    }
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
      app.selRoot = digits;
      app.selCode = "00000";
      app.selYear = null;
      app.writeLocal(SELECTION_KEY, { root: digits, code: "00000" });
      this.reset();
      this.addYear = true;
      app.go("forms", cx);
      app.toast(`${name} added. Add a tax year to choose its forms.`, cx);
    } catch (e) {
      n.error = message(e);
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
      code: InputState.new({ value: suggestion }),
      name: InputState.new({ placeholder: "Cebu branch" }),
      inputs: {},
      inherit: {},
      error: null,
    };
    for (const f of ["rdo_code", ...ADDRESS_FIELDS.map(([f]) => f)]) {
      this.newBranch.inputs[f] = InputState.new({ value: head ? head.facts[f] || "" : "" });
      this.newBranch.inherit[f] = Boolean(head);
    }
    this.app.panel = null;
    this.app.go("new_branch", cx);
  }

  createBranch(cx) {
    const app = this.app;
    const nb = this.newBranch;
    const code = nb.code.value().trim();
    if (!/^\d{5}$/.test(code)) {
      nb.error = "Branch code is 5 digits, like 00001.";
      return cx.notify();
    }
    const name = nb.name.value().trim();
    if (!name) {
      nb.error = "Type a branch name, like Cebu branch.";
      return cx.notify();
    }
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
      nb.error = message(e);
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
    if (editing) {
      input = TextField(cx, ui.inputs[f], {
        id: `tp-${sec}-${f}`,
        label,
        disabled: inheritable && ui.inherit[f],
        error: f === "tin" && Boolean(ui.tinError),
      });
    }
    return {
      full: opts.full,
      el: Field(cx, label, {
        value,
        input,
        error: f === "tin" ? ui.tinError : null,
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
        field("branch", "code", "Branch code", b.code, { readonly: true }),
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

  const auth = ui.auth();
  const pinControls = div()
    .flex()
    .flex_row()
    .flex_wrap()
    .items_center()
    .gap(SPACE[2])
    .child(PinCells(cx, ui.pinOtp, "tp-security-pin-cells"))
    .child(AppButton(cx, "tp-security-pin-save", auth.has_pin ? "Change PIN" : "Set PIN", { size: "sm", onClick: (_e, cx) => ui.setPin(cx) }));
  if (auth.has_pin) {
    pinControls.child(AppButton(cx, "tp-security-pin-clear", "Remove", { variant: "ghost", size: "sm", onClick: (_e, cx) => ui.clearPin(cx) }));
  }
  let totpControl;
  if (ui.totpSecret) {
    totpControl = div()
      .flex()
      .flex_row()
      .flex_wrap()
      .items_center()
      .gap(SPACE[2])
      .child(PinCells(cx, ui.totpOtp, "tp-security-totp-cells"))
      .child(AppButton(cx, "tp-security-totp-confirm", "Confirm", { variant: "primary", size: "sm", onClick: (_e, cx) => ui.confirmTotp(cx) }))
      .child(
        AppButton(cx, "tp-security-totp-cancel", "Cancel", {
          variant: "ghost",
          size: "sm",
          onClick: (_e, cx) => {
            ui.totpSecret = null;
            cx.notify();
          },
        }),
      );
  } else {
    totpControl = AppButton(cx, "tp-security-totp-btn", auth.has_totp ? "Remove" : "Add", {
      variant: auth.has_totp ? "ghost" : "secondary",
      size: "sm",
      onClick: (_e, cx) => (auth.has_totp ? ui.clearTotp(cx) : ui.startTotp(cx)),
    });
  }
  const pinsOn = app.store.authState().enable_profile_pins;
  const rows = [
    SettingRow(
      cx,
      "tp-security-pin",
      "Taxpayer PIN",
      auth.has_pin
        ? pinsOn
          ? "Asked before this taxpayer opens"
          : "Set. Turn on Taxpayer PINs in Settings to ask for it"
        : "Ask for a 4-digit PIN before this taxpayer opens",
      pinControls,
    ),
    SettingRow(
      cx,
      "tp-security-totp",
      "Authenticator",
      ui.totpSecret
        ? `Add this key to your authenticator app, then type the code it shows: ${ui.totpSecret}`
        : auth.has_totp
          ? "A code from your authenticator app also unlocks this taxpayer"
          : "Use a 6-digit code from an authenticator app instead of the PIN",
      totpControl,
    ),
    SettingRow(
      cx,
      "tp-security-archive",
      "Archive or delete",
      "Hide this taxpayer from new work, or remove it",
      AppButton(cx, "tp-security-archive-btn", "Open", { size: "sm", onClick: (_e, cx) => app.openLegacyEditor(cx) }),
    ),
  ];
  const securityBody = div().flex().flex_col();
  rows.forEach((row, i) => securityBody.child(i ? row.border_t(1).border_color(tokens(cx).border) : row));

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

  return DisclosureGroup(cx, "tp-profile", [
    section("taxpayer", "Taxpayer", "Shared by every branch", taxpayerBody, true),
    section("branch", "Branch", b.is_head_office ? "Head office" : `Branch ${b.code}`, branchBody, true),
    section(
      "address",
      "Address and contact",
      b.is_head_office ? "Used on every return" : inhCount ? `${inhCount} of 4 from head office` : "All set for this branch",
      addressBody,
      true,
    ),
    section("security", "Email, security and export", "Set once for the taxpayer", securityBody, false),
  ]);
}

// ---------------------------------------------------------------- forms tab

/** Next open period and its due date for a form in the selected year. */
function schedule(app, code, year) {
  const b = app.branch();
  const rows = app.store
    .deadlines(`${year}-01-01`, `${year + 1}-12-31`)
    .filter((d) => d.tin === b.tin && d.form_code === code && d.taxable_year === year);
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
  const input = (key, label) => ({ el: Field(cx, label, { input: TextField(cx, n[key], { id: `new-tp-${key}`, label }) }) });
  const cells = [
    input("tin", "TIN"),
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
    input("rdo_code", "RDO code"),
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
  const labels = { rdo_code: "RDO code", registered_address: "Registered address", zip_code: "ZIP code", phone: "Phone", email: "Email" };
  const cells = [
    { el: Field(cx, "Branch code", { input: TextField(cx, nb.code, { id: "new-branch-code", label: "Branch code" }) }) },
    { el: Field(cx, "Branch name", { input: TextField(cx, nb.name, { id: "new-branch-name", label: "Branch name" }) }) },
  ];
  for (const f of Object.keys(nb.inputs)) {
    cells.push({
      full: f === "registered_address" || f === "email",
      el: Field(cx, labels[f], {
        input: TextField(cx, nb.inputs[f], { id: `new-branch-${f}`, label: labels[f], disabled: nb.inherit[f] }),
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

