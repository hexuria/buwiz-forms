// One taxpayer page's state while it shows (old TaxpayerActions' per-view fields): the section being edited, the fact row
// being edited, Save as a setup, the delete arm, the inbox credentials, the staged forms and the Add year band. Made when
// the page is made (a move), so leaving the page drops every edit, as the old leaveView did. Data, never signals in rows.
import { batch, signal } from "nsh";
import type { BranchFieldName, InboxAuthMethod, Taxpayer } from "../../bir";
import { errorText } from "../../lib/errors";
import { FACT_LABELS, type FactName } from "../../lib/facts";
import { app, back, store, toast } from "../../shell";
import { leaveYearShared, resetShared, setSectionOpen } from "./actions";
import {
  ADDRESS_FIELDS, BRANCH_CODE_ERROR, EMAIL_RE, factError, parseAtcCodes, pastDateError, shownErrors, TIN_ERROR, type EditableSection,
} from "./logic";

/** The section being edited: every value as typed, which fields follow the head office, touched fields, errors. */
export interface SectionEdit {
  sec: EditableSection;
  values: Record<string, string>;
  type: Taxpayer["taxpayer_type"];
  inherit: Partial<Record<BranchFieldName, boolean>>;
  touched: string[];
  submitted: boolean;
  /** The TIN or branch code move's own error (or the host's answer to it). */
  tinError: string | null;
  /** The registered activity has the year's line of business and start date (a year exists). */
  yearFields: boolean;
  /** The tax year those fields were read from (null without them): the edit belongs to it, and a save under another year is refused. */
  year: number | null;
  focusField?: string;
  fromForm?: boolean;
}

/** The fact row being edited. */
export interface FactEdit {
  fact: FactName;
  tin: string;
  year: number;
  value: unknown;
  /** The ATC codes as typed. */
  atc: string;
  error: string | null;
  /** Opened from a return (the 2551Q card): a save goes back to it. */
  fromForm: boolean;
}

/** The inbox credentials being edited. */
export interface InboxEdit {
  method: InboxAuthMethod;
  email: string;
  host: string;
  password: string;
  error: string | null;
}

/** RDO codes of the reference list. */
export function knownRdo(code: string): boolean {
  const list = store.rdoOptions();
  return !list.length || list.some((o) => o.value === code);
}

/** The page's state and actions. */
export function createPageState() {
  const edit = signal<SectionEdit | null>(null);
  const factEdit = signal<FactEdit | null>(null);
  const saveSetup = signal<{ name: string; error: string | null } | null>(null);
  const deleteArmed = signal(false);
  const inbox = signal<InboxEdit | null>(null);
  /** The forms ticked while editing the year's list, or null. */
  const forms = signal<string[] | null>(null);
  const query = signal("");
  const freq = signal("All");
  const addYear = signal(false);
  const addYearSel = signal<number | null>(null);
  const addYearCopy = signal(true);

  const yearFacts = () => {
    const b = app.branch();
    const y = app.year();
    return b && y ? store.yearFacts(b.tin, y) : null;
  };

  // ------------------------------------------------------------ sections

  function startEdit(sec: EditableSection, opts: { focusField?: string; fromForm?: boolean } = {}): void {
    const t = app.taxpayer();
    const b = app.branch();
    if (!t || !b) return;
    const values: Record<string, string> = {};
    const inherit: SectionEdit["inherit"] = {};
    let yearFields = false;
    let year: number | null = null;
    if (sec === "taxpayer") {
      values.name = t.name;
      values.tin = t.tin_root;
      if (t.taxpayer_type === "Individual") values.birth_date = t.birth_date || "";
    } else if (sec === "branch") {
      if (!b.is_head_office) values.code = b.code;
      values.name = b.name;
      values.rdo_code = b.facts.rdo_code || "";
      inherit.rdo_code = (b.inherited || []).includes("rdo_code");
      const view = yearFacts();
      if (view) {
        yearFields = true;
        year = view.year;
        values.line_of_business = view.facts.line_of_business || "";
        values.business_start_date = view.facts.business_start_date || "";
      }
    } else {
      for (const [f] of ADDRESS_FIELDS) {
        values[f] = b.facts[f] || "";
        inherit[f] = (b.inherited || []).includes(f);
      }
    }
    batch(() => {
      setSectionOpen(sec, true);
      edit.set({ sec, values, type: t.taxpayer_type, inherit, touched: [], submitted: false, tinError: null, yearFields, year, ...opts });
    });
  }

  /**
   * The field a link came to set got there (the phone date sheet opened by itself): the edit stops naming it, so a later remount of
   * its row in the same edit (a resize between the wide and phone layouts) does not open the sheet or take the keys again.
   */
  function focusTaken(field: string): void {
    const e = edit();
    if (e?.focusField === field) edit.set({ ...e, focusField: undefined });
  }

  function cancelEdit(): void {
    edit.set(null);
  }

  function setValue(field: string, value: string): void {
    const e = edit();
    if (!e) return;
    edit.set({ ...e, values: { ...e.values, [field]: value }, tinError: field === "tin" || field === "code" ? null : e.tinError });
  }

  function touch(field: string): void {
    const e = edit();
    if (!e || e.touched.includes(field)) return;
    edit.set({ ...e, touched: [...e.touched, field] });
  }

  function setType(type: Taxpayer["taxpayer_type"]): void {
    const e = edit();
    if (e) edit.set({ ...e, type });
  }

  /** "Same as head office" on: the box shows the head office's value and is not saved for this branch. */
  function setInherit(field: BranchFieldName, on: boolean): void {
    const e = edit();
    if (!e) return;
    const head = app.taxpayer()?.branches.find((x) => x.is_head_office);
    const values = on && head ? { ...e.values, [field]: head.facts[field] || "" } : e.values;
    edit.set({ ...e, values, inherit: { ...e.inherit, [field]: on } });
  }

  /** Every problem with the section as typed, by field. */
  function problems(e: SectionEdit): Record<string, string> {
    const b = app.branch();
    const t = app.taxpayer();
    const out: Record<string, string> = {};
    const own = (f: BranchFieldName) => Boolean(b?.is_head_office) || !e.inherit[f];
    if (e.sec === "taxpayer" && e.values.tin.length !== 9) out.tin = TIN_ERROR;
    if (e.sec === "branch" && e.values.code !== undefined && b && e.values.code !== b.code) {
      if (!/^\d{5}$/.test(e.values.code)) out.code = BRANCH_CODE_ERROR;
      else if (e.values.code === "00000" || t?.branches.some((x) => x.code === e.values.code)) out.code = `Branch ${e.values.code} already exists.`;
    }
    for (const f of ["rdo_code", "zip_code", "email"] as const) {
      if (e.values[f] === undefined || !own(f)) continue;
      const error = factError(f, e.values[f], knownRdo);
      if (error) out[f] = error;
    }
    for (const f of ["birth_date", "business_start_date"]) {
      const error = e.values[f] !== undefined ? pastDateError(e.values[f].trim()) : null;
      if (error) out[f] = error;
    }
    return out;
  }

  /** The errors the section shows now (touched or after Save), with the move's own. */
  function errors(): Record<string, string> {
    const e = edit();
    if (!e) return {};
    const shown = shownErrors(problems(e), e.touched, e.submitted);
    if (e.tinError) shown[e.sec === "taxpayer" ? "tin" : "code"] = e.tinError;
    return shown;
  }

  function saveSection(): void {
    const e0 = edit();
    const t = app.taxpayer();
    const b = app.branch();
    if (!e0 || !t || !b) return;
    // An edit that holds a year's facts is that year's: if the year changed under it, nothing of it is written into the new one.
    if (e0.yearFields && e0.year !== (yearFacts()?.year ?? null)) {
      edit.set(null);
      toast("The tax year changed while this was open, so nothing was saved. Open the edit again for this year.", "triangle-alert");
      return;
    }
    const e = { ...e0, submitted: true };
    edit.set(e);
    if (Object.keys(problems(e)).length) return;
    const v = e.values;
    try {
      if (e.sec === "taxpayer") {
        const digits = v.tin;
        let text = "Changes saved";
        // Name and type first, under the TIN they were opened with: the move is the step a TIN in use refuses, and a
        // refusal must not leave a moved TIN with the rest unsaved.
        const update: { name: string; taxpayer_type: Taxpayer["taxpayer_type"]; birth_date?: string | null } = {
          name: v.name.trim() || t.name,
          taxpayer_type: e.type || t.taxpayer_type,
        };
        if (v.birth_date !== undefined) {
          const born = v.birth_date.trim() || null;
          if (born !== (t.birth_date || null)) update.birth_date = born;
        }
        store.write("update_taxpayer", t.tin_root, update);
        if (digits !== t.tin_root) {
          // One transaction in bir-core: branches, tax years, returns and templates move together.
          try {
            store.write("move_taxpayer_tin", t.tin_root, digits);
          } catch (err) {
            edit.set({ ...e, tinError: `Name and type saved. The TIN did not change: ${errorText(err)}` });
            return;
          }
          const old = t.tin_root;
          app.remapRecent((tin) => (tin.startsWith(old) ? digits + tin.slice(9) : tin));
          const year = app.year();
          app.selectTaxpayer(digits, { quiet: true, code: b.code });
          if (year) app.selectYear(year);
          text = `TIN changed to ${digits.replace(/^(\d{3})(\d{3})(\d{3})$/, "$1-$2-$3")}. Every branch, tax year, return and template moved with it.`;
        }
        edit.set(null);
        toast(text);
        // A moved TIN leaves the return's route behind, so only an unmoved one goes back to it.
        if (e.fromForm && digits === t.tin_root && app.canBack()) back();
        return;
      }
      if (e.sec === "branch") {
        let current = b;
        if (v.code !== undefined && v.code !== b.code) {
          const oldTin = b.tin;
          const year = app.year();
          const moved = store.write("move_branch_code", b.tin, v.code);
          app.remapRecent((tin) => (tin === oldTin ? t.tin_root + v.code : tin));
          app.selectBranch(v.code);
          if (year) app.selectYear(year);
          current = moved;
        }
        const fields: Partial<Record<BranchFieldName, string | null>> = {
          rdo_code: !current.is_head_office && e.inherit.rdo_code ? null : v.rdo_code.trim(),
        };
        store.write("update_branch", current.tin, { name: v.name.trim() || current.name, fields });
        const view = yearFacts();
        if (view && e.yearFields) {
          const patch: { line_of_business?: string | null; business_start_date?: string | null } = {};
          const line = v.line_of_business.trim() || null;
          const start = v.business_start_date.trim() || null;
          if (line !== (view.facts.line_of_business || null)) patch.line_of_business = line;
          if (start !== (view.facts.business_start_date || null)) patch.business_start_date = start;
          if (Object.keys(patch).length) store.write("set_year_facts", current.tin, view.year, patch);
        }
      } else {
        const fields: Partial<Record<BranchFieldName, string | null>> = {};
        for (const [f] of ADDRESS_FIELDS) fields[f] = !b.is_head_office && e.inherit[f] ? null : v[f].trim();
        store.write("update_branch", b.tin, { fields });
      }
      edit.set(null);
      toast("Changes saved");
      if (e.fromForm && app.canBack()) back();
    } catch (err) {
      edit.set({ ...e, tinError: null });
      toast(errorText(err), "triangle-alert");
    }
  }

  // ------------------------------------------------------------ fact rows

  function startFactEdit(fact: FactName, opts: { fromForm?: boolean } = {}): void {
    const b = app.branch();
    const year = app.year();
    const view = yearFacts();
    if (!b || !year || !view) return;
    const current = (view.facts as Record<string, unknown>)[fact];
    batch(() => {
      setSectionOpen("facts", true);
      factEdit.set({
        fact,
        tin: b.tin,
        year,
        value: current === undefined ? null : current,
        atc: fact === "atc_codes" ? ((current as string[] | null) || []).join(", ") : "",
        error: null,
        fromForm: Boolean(opts.fromForm),
      });
    });
  }

  function pickFact(value: unknown): void {
    const e = factEdit();
    if (!e) return;
    // "Graduated rates" keeps a deduction method already chosen.
    const next = value === "GraduatedUnspecified" && /^Graduated/.test(String(e.value || "")) ? e.value : value;
    factEdit.set({ ...e, value: next, error: null });
  }

  function setAtc(text: string): void {
    const e = factEdit();
    if (e) factEdit.set({ ...e, atc: text, error: null });
  }

  function cancelFactEdit(): void {
    factEdit.set(null);
  }

  /** Saves the fact for the year; opened from a return, goes back to it. */
  function saveFactEdit(): void {
    const e = factEdit();
    if (!e) return;
    let value: unknown = e.value;
    if (e.fact === "atc_codes") {
      value = parseAtcCodes(e.atc);
      if (!value) return void factEdit.set({ ...e, error: "Type ATC codes like WC160 or PT010, with commas between them." });
    } else if (value === null || value === undefined) {
      return void factEdit.set({ ...e, error: "Choose one first." });
    }
    try {
      store.write("set_year_facts", e.tin, e.year, { [e.fact]: value });
    } catch (err) {
      return void factEdit.set({ ...e, error: errorText(err).replace(/^Other error:\s*/, "") });
    }
    factEdit.set(null);
    toast(`${FACT_LABELS[e.fact]} saved for ${e.year}.`);
    if (e.fromForm && app.canBack()) back();
  }

  // ------------------------------------------------------------ save as a setup

  function openSaveSetup(): void {
    saveSetup.set({ name: "", error: null });
  }

  function saveSetupNow(): void {
    const e = saveSetup();
    const b = app.branch();
    const year = app.year();
    if (!e || !b || !year) return;
    const name = e.name.trim();
    if (!name) return void saveSetup.set({ ...e, error: "Type a name for the setup." });
    try {
      store.write("save_setup", b.tin, year, name);
      saveSetup.set(null);
      toast(`Saved as the setup ${name}. Pick it when you add a taxpayer.`, "layout-template");
    } catch (err) {
      saveSetup.set({ ...e, error: errorText(err) });
    }
  }

  // ------------------------------------------------------------ inbox credentials

  function startInboxEdit(method?: InboxAuthMethod): void {
    const b = app.branch();
    if (!b) return;
    const s = store.inboxState(b.tin);
    inbox.set({
      method: method ?? (s?.auth_method === "GoogleOAuth" ? "GoogleOAuth" : "AppPassword"),
      email: s?.imap_login || "",
      host: s?.imap_host || "imap.gmail.com",
      password: "",
      error: null,
    });
  }

  /** Stages the method pick without saving; it applies on the next write. */
  function pickInboxMethod(method: InboxAuthMethod): void {
    const e = inbox();
    if (!e) startInboxEdit(method);
    else inbox.set({ ...e, method });
  }

  function setInbox(field: "email" | "host" | "password", value: string): void {
    const e = inbox();
    if (e) inbox.set({ ...e, [field]: value, error: null });
  }

  function saveInbox(): void {
    const e = inbox();
    const b = app.branch();
    // Google's "saving" is the Connect button's grant; only an app password has credentials to keep.
    if (!e || !b || e.method !== "AppPassword") return;
    const email = e.email.trim();
    if (!EMAIL_RE.test(email)) return void inbox.set({ ...e, error: "Type an email like name@example.com." });
    try {
      store.write("set_inbox_app_password", b.tin, { imap_email: email, imap_host: e.host.trim() || null, app_password: e.password.trim() || null });
      inbox.set(null);
      toast("Inbox credentials saved. Verify the connection to start tracking.");
    } catch (err) {
      inbox.set({ ...e, error: errorText(err) });
    }
  }

  // ------------------------------------------------------------ forms and years

  function editForms(): void {
    batch(() => {
      forms.set([...app.yearForms()]);
      query.set("");
      freq.set("All");
      addYear.set(false);
    });
  }

  function toggleForm(code: string, on: boolean): void {
    const list = forms();
    if (!list) return;
    forms.set(on ? (list.includes(code) ? list : [...list, code]) : list.filter((c) => c !== code));
  }

  function toggleAddYear(): void {
    batch(() => {
      addYear.set(!addYear());
      addYearSel.set(null);
      addYearCopy.set(true);
      forms.set(null);
    });
  }

  /** Taxpayer or branch switched: every edit in progress goes. */
  function reset(): void {
    batch(() => {
      edit.set(null);
      factEdit.set(null);
      saveSetup.set(null);
      deleteArmed.set(false);
      inbox.set(null);
      forms.set(null);
      addYear.set(false);
      resetShared();
    });
  }

  /** The year switched: the year's edits go (the fact row, Save as a setup, staged forms, the band, a section edit that holds the year's facts). */
  function leaveYear(): void {
    batch(() => {
      // The old page kept the section open and its Save wrote the old year's typing into the new year: it is closed instead.
      if (edit()?.yearFields) edit.set(null);
      factEdit.set(null);
      saveSetup.set(null);
      forms.set(null);
      addYear.set(false);
      leaveYearShared();
    });
  }

  return {
    edit, factEdit, saveSetup, deleteArmed, inbox, forms, query, freq, addYear, addYearSel, addYearCopy, yearFacts,
    startEdit, focusTaken, cancelEdit, setValue, touch, setType, setInherit, errors, saveSection,
    startFactEdit, pickFact, setAtc, cancelFactEdit, saveFactEdit, openSaveSetup, saveSetupNow,
    startInboxEdit, pickInboxMethod, setInbox, saveInbox, editForms, toggleForm, toggleAddYear, reset, leaveYear,
  };
}

/** A taxpayer page's state. */
export type PageState = ReturnType<typeof createPageState>;
