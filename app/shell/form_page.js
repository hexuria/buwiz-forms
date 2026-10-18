// The on-screen return (DESIGN_SYSTEM.md "Form page"): one renderer for all
// 43 forms, drawn from app/forms/forms-schema.json (served by the host as
// `screen_form_schema`). Every control binds to its rules-corpus field key
// `k`; values travel through bir-core's typed drafts via the screen_form_*
// host functions, so Save, Queue and Print keep using the same stored rows.
//
// Never block the UI on form work: opening and validating are async host
// calls; edits show at once and validation runs 300 ms after the last edit,
// with a "Checking…" chip while it runs. The html-frozen/ sheets stay the
// print output and are not touched here.

import { View, div } from "gpui-kit";
import { InputState, CalendarState, Popup } from "gpui-base";
import { Spinner } from "gpui-component";
import {
  tokens,
  SPACE,
  RADIUS,
  SIZE,
  BREAK,
  Text,
  Line,
  Code,
  Icon,
  AppButton,
  IconButton,
  Avatar,
  Chip,
  Switch,
  Checkbox,
  CheckboxLabel,
  TextField,
  Dropdown,

  PageHeader,
  MetaLine,
  Overlay,
} from "./ui.js";
import { errorText as message, formatTin } from "./store.js";
import { periodLabel } from "./periods.js";

const VALIDATE_DELAY = 300;
/** A sample's period is asked about this long after its last change. */
const PERIOD_ASK_DELAY = 300;
/** Debounce for live print-preview sync (ms after the last keystroke). */
const LIVE_PREVIEW_DELAY = 120;

/**
 * A listed dropdown option as `{ value, label }`. The schema writes many
 * as `"CODE - Label"` strings (`"03 - March"`): the host takes the code, so
 * that is the value and the whole string the label. Template text the
 * extractor left (`"\" + rdo.description + \""`) is dropped. Mirrored by
 * `page_option_value` in crates/bir-shell/src/screen_form_audit.rs.
 */
function optionOf(o) {
  if (typeof o !== "string") return o;
  if (/^"/.test(o)) return null;
  const m = /^(\S+) - (.+)$/.exec(o);
  return m ? { value: m[1], label: o } : { value: o, label: o };
}

/**
 * A dropdown for key `k` (a field or a schedule cell): the host's catalog
 * choices or the schema's listed ones. The host resolves catalog choices
 * into the typed draft and marks the selected one when two options share an
 * official code (2000).
 */
function selectBox(cx, ui, k, label, listedRaw, value, locked, width) {
  const s = ui.state;
  const listed = (s.options[k] || listedRaw || []).map(optionOf).filter(Boolean);
  const options = [{ value: "", label: "Choose…" }, ...listed];
  const selected = listed.find((o) => o.selected);
  const known = options.some((o) => o.value === value);
  // Unknown all-zero codes (0, 00, 000) and a lone "-" (2000's ATC writer)
  // are the official runtime's empty choices (excise locations, an empty
  // RDO). A listed 0 or "-" stays selectable.
  const shown = selected ? selected.value : known ? value : /^(0+|-)$/.test(value) ? "" : value;
  if (shown && !selected && !known) options.push({ value: shown, label: shown });
  if (locked) {
    const hint = !shown ? ui.reasonFor(k) : "";
    if (hint) return lockedBox(cx, hint, width, { hint: true });
    return lockedBox(cx, (options.find((o) => o.value === shown) || { label: shown }).label, width);
  }
  return Dropdown(cx, `fp-${k}`, {
    label,
    width,
    options,
    value: shown,
    open: s.openSelect === k,
    onOpenChange: (open, cx) => {
      s.openSelect = open ? k : null;
      ui.refreshRows();
      cx.notify();
    },
    onChange: (v, cx) => ui.choose({ [k]: v }, cx),
  });
}

/** Items whose four text boxes are the TIN (3-3-3-5), shown as locked boxes. */
function isTinItem(it) {
  const fs = it.f || [];
  const named = /\bTIN\b|Identification/i.test(it.l) || fs.some((x) => /\bTIN\d?\b|txtTIN/i.test(`${x.l} ${x.k}`));
  return named && fs.length >= 3 && fs.length <= 5 && fs.every((x) => x.t === "text" || x.t === "int");
}

/**
 * Drop fields the schema marks `hidden` (internal or login controls of the
 * official runtime), and items left with no field. Validation messages for
 * them fall back to their item number or the first item.
 */
function withoutHidden(schema) {
  const parts = schema.parts
    .map((p) => ({
      ...p,
      items: p.items
        .map((it) => (it.f ? { ...it, f: it.f.filter((f) => f.t !== "hidden") } : it))
        // A printed item with no box in the official program keeps its note.
        .filter((it) => it.rows || (it.f && it.f.length) || it.note),
    }))
    .filter((p) => p.items.length);
  return { ...schema, parts };
}

function partName(p, i) {
  return i === 0 && /^Page \d+$/.test(p.n) ? "Return details" : p.n;
}

/**
 * A part name for the compact navigator's one line: the filing instructions
 * in parentheses go ("(attach additional sheet/s, if necessary)") and
 * "Schedule" becomes "Sch.". The row's tooltip keeps the full name.
 */
function compactPartName(name) {
  const short = name
    .replace(/\s*\((?:attach|if graduated|except)[^)]*\)/gi, "")
    .replace(/^Schedule\b/, "Sch.")
    .trim();
  return short || name;
}

/** Every key an item binds: its fields, or every cell of its table. */
/**
 * A date the filer typed, read the way the old tax-profile date input read
 * it: `12012020`, `12/01/2020`, `12-01-2020`, `2020-12-01` or `Dec 01 2020`
 * all mean December 1, 2020. Returns null when it is not a real day.
 */
export function parseTypedDate(text) {
  const v = String(text || "").trim();
  if (!v) return null;
  const MON = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
  let y, m, d, hit;
  if ((hit = /^(\d{2})(\d{2})(\d{4})$/.exec(v))) [, m, d, y] = hit;
  else if ((hit = /^(\d{4})-(\d{1,2})-(\d{1,2})$/.exec(v))) [, y, m, d] = hit;
  else if ((hit = /^(\d{1,2})[/-](\d{1,2})[/-](\d{4})$/.exec(v))) [, m, d, y] = hit;
  else if ((hit = /^([a-z]{3})[a-z]*\.?\s+(\d{1,2}),?\s+(\d{4})$/i.exec(v))) {
    m = MON.indexOf(hit[1].toLowerCase()) + 1;
    [, , d, y] = hit;
  } else return null;
  [y, m, d] = [Number(y), Number(m), Number(d)];
  if (y < 1900 || y > 2100 || m < 1 || m > 12 || d < 1) return null;
  if (d > new Date(Date.UTC(y, m, 0)).getUTCDate()) return null;
  return { y, m, d };
}

/** The form's MM/DD/YYYY text for a parsed date. */
export function formatMDY({ y, m, d }) {
  return `${String(m).padStart(2, "0")}/${String(d).padStart(2, "0")}/${y}`;
}

const MONTH_SHORT = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/** How a date box shows a parsed date: "Dec 10, 2020" (rule 4). */
export function formatLongDate({ y, m, d }) {
  return `${MONTH_SHORT[m - 1]} ${d}, ${y}`;
}

/**
 * What a date box shows for the draft's text: the date as "Dec 10, 2020"
 * when it reads as one, else the text as it is (a blank, a part-typed date).
 * Display only: the draft keeps the form's own format.
 */
export function shownDate(text) {
  const date = parseTypedDate(text);
  return date ? formatLongDate(date) : String(text ?? "");
}

const DATE_ERROR = "Enter a valid date as MM/DD/YYYY (e.g. 12/01/2020 or 12012020)";

/**
 * A Form library sample is a preview and is never saved (provisional: the
 * findings rules page, question 1). A return starts with New return.
 */
const SAMPLE_NOTE = "A sample from the Form library is not saved. To start a return, choose Start a return, or New return (the + button or Cmd/Ctrl+N).";

/** How faded a box another answer turns off is (rule 6). */
const OFF_OPACITY = 0.45;

/**
 * The tag on a box another answer turns off (rule 6): the host's reviewed
 * tag ("Needs item 13 = Yes"), cut if it ever runs long.
 */
export function needsText(tag) {
  const text = String(tag || "").trim();
  return text.length > 90 ? `${text.slice(0, 89).trimEnd()}…` : text;
}

/** One box holding a whole date: the host types it from its binding. */
function isDateField(f) {
  return f.t === "date";
}

function itemKeys(it) {
  if (it.rows) return it.keys.flat();
  // A pooled address line is drawn by its pool's first line.
  return (it.f || []).flatMap((f) => (f.pool ? f.pool.keys : [f.k]));
}

/**
 * Lay `text` over stacked lines of `slots` character boxes (rule 5), as the
 * host splits a pooled address (combs::split_lines): each line but the last
 * ends at the last space run that starts within its boxes, the run kept at
 * the end of that line, and the last takes the rest, so the lines put end
 * to end are the text. A word longer than its line is cut at the line's
 * last box when `cutWords` (the host splits these lines into saved fields),
 * else it stays whole on the line (the boxes are only shown).
 */
export function splitLines(text, slots, cutWords = false) {
  const lines = [];
  let rest = [...String(text ?? "").trim()];
  const space = (c) => /\s/.test(c);
  slots.forEach((n, i) => {
    if (i === slots.length - 1 || rest.length <= n) {
      lines.push(rest.join(""));
      rest = [];
      return;
    }
    let cut = -1;
    for (let p = n; p >= 1; p--) {
      if (space(rest[p]) && !space(rest[p - 1])) {
        cut = p;
        break;
      }
    }
    if (cut < 0 && cutWords) cut = n;
    else {
      if (cut < 0) {
        cut = n;
        while (cut < rest.length && !space(rest[cut])) cut++;
      }
      while (cut < rest.length && space(rest[cut])) cut++;
    }
    lines.push(rest.slice(0, cut).join(""));
    rest = rest.slice(cut);
  });
  return lines;
}

/** Whether `text` fits the boxes of `slots`: boxes until it no longer fits. */
export function fitsBoxes(text, slots, cutWords = false) {
  return splitLines(text, slots, cutWords).every((line, i) => line.trimEnd().length <= slots[i]);
}

/**
 * "1,234.50" from a number-ish string; empty stays empty. `whole`: a box
 * over a whole-number field shows whole pesos ("1,234"); a typed fraction
 * stays as typed so the box's own message explains it.
 */
export function formatMoney(text, whole = false) {
  const n = Number(String(text).replace(/[₱,\s]/g, ""));
  if (String(text).trim() === "" || Number.isNaN(n)) return String(text);
  if (whole && !Number.isInteger(n)) return String(text);
  // QuickJS has no Intl, so group the digits by hand.
  const [int, frac] = Math.abs(n).toFixed(whole ? 0 : 2).split(".");
  const grouped = `${n < 0 ? "-" : ""}${int.replace(/\B(?=(\d{3})+(?!\d))/g, ",")}`;
  return whole ? grouped : `${grouped}.${frac}`;
}

export class FormPageActions {
  constructor(app) {
    this.app = app;
    this.state = null;
    this.token = 0;
  }

  /** Release a form's retained views, text states and focus handles. */
  release(state) {
    for (const row of state?.rowViews?.values() || []) row.entity.release();
    for (const input of state?.inputs?.values() || []) input.release();
    for (const handle of state?.editFocus?.values() || []) handle.release();
    state?.checkButton?.entity.release();
  }

  /**
   * Load the schema now and the return in the background. `keep`: the form
   * on screen is kept alive (not released) for an Undo to bring back.
   * `then(error, cx)` runs once the host answered (error null on success).
   * `opts.returnKey`: the stored return's exact key (a transaction's MMDD, 2553's
   * quarter); `opts.filedOriginal`: an amended return's filed version.
   */
  open(opts, cx, keep = false, then = null) {
    const app = this.app;
    // Leaving the sample: an offer of a stored return for its period goes.
    this.closeOffer();
    // The last form's retained views and text states go with it, unless an
    // Undo may still bring it back.
    if (this.kept && this.kept !== this.state) this.release(this.kept);
    this.kept = null;
    if (keep) this.kept = this.state;
    else this.release(this.state);
    const token = ++this.token;
    let schema = null;
    let error = null;
    try {
      schema = app.store.call("screen_form_schema", opts.code);
    } catch (e) {
      error = message(e);
    }
    if (schema) schema = withoutHidden(schema);
    this.state = {
      ...opts,
      schema,
      loading: true,
      error,
      draft: null,
      status: "Draft",
      editable: true,
      values: {},
      options: {},
      locks: {},
      profile: new Set(),
      // Filled profile boxes the filer may open with Edit, and the ones open.
      profileEdit: new Set(),
      editing: new Set(),
      // Profile boxes the filer opened with Edit on this visit (sent as
      // `unlocked`), the item under the pointer, and the box that has focus.
      opened: new Set(),
      hoverItem: null,
      focusKey: null,
      editFocus: new Map(),
      // Parts whose "From tax profile" group is unfolded.
      profileOpen: new Set(),
      // A Form library sample: its period and date boxes stay blank until typed.
      sample: Boolean(opts.sample),
      blank: new Set(),
      needs: {},
      // Boxes another answer turns off, each with what turns it on (rule 6).
      off: {},
      // Each party's penalty boxes (rule 8), and the ones Edit penalties opened.
      penalties: [],
      penaltyOpen: new Set(),
      typed: new Set(),
      errors: [],
      messages: {},
      formErrors: [],
      checking: false,
      openParts: new Set([0]),
      inputs: new Map(),
      rowViews: new Map(),
      pending: {},
      dirty: false,
      saved: false,
      openSelect: null,
      moreOpen: new Set(),
      guided: false,
    };
    if (!schema) return;
    this.state.checkButton = {
      entity: cx.new(FormCheckButton, { ui: this, state: this.state }),
      checking: false,
    };
    cx.spawn(async (acx) => {
      try {
        const key = opts.returnKey ?? null;
        const r = opts.sample
          ? await app.store.call("screen_form_sample", opts.tin, opts.code)
          : opts.filedOriginal
            ? await app.store.call("screen_form_filed_original", opts.code, opts.filedOriginal)
            : opts.amend
              ? await app.store.call("screen_form_amend", opts.tin, opts.code, opts.year, opts.period, key)
              : await app.store.call("screen_form_open", opts.tin, opts.code, opts.year, opts.period, key, opts.newKey ?? null);
        if (token !== this.token) return;
        const s = this.state;
        s.loading = false;
        this.takeOpened(r);
        // Every part already open (one picked while the return was still
        // opening, too) gets its inputs now that the draft is here.
        for (const i of new Set([0, ...s.openParts])) this.ensureInputs(i, acx);
        s.chromeSnapshot = this.chromeSnapshot();
        acx.notify();
        if (then) then(null, acx);
      } catch (e) {
        if (token !== this.token) return;
        this.state.loading = false;
        this.state.error = message(e);
        acx.notify();
        if (then) then(this.state.error, acx);
      }
    });
  }

  /** Take what the host opened (a return, a sample, or a sample rebuilt for its period). */
  takeOpened(r) {
    const s = this.state;
    s.draft = r.draft;
    s.status = r.status;
    s.editable = r.editable;
    s.blank = new Set(r.blank || []);
    // A sample's period parts: page inputs the filer sets the period with,
    // never written into the draft (screen_form_sample_period rebuilds it).
    s.periodParts = new Map((r.period_parts || []).map((p) => [p.key, p]));
    s.periodSet = Boolean(r.period_set);
    // A read-only return says when it was sent and confirmed; an amendment
    // says what it amends.
    s.submittedAt = r.submitted_at || null;
    s.confirmedAt = r.confirmed_at || null;
    s.amends = r.amends || null;
    // An amended return's filed version, kept in its history; and, on that
    // version opened read-only, what it is.
    s.filedOriginal = r.filed_original || null;
    s.filedOriginalView = r.filed_original_view || null;
    s.values = this.shown(r.values);
    s.options = r.options || {};
    s.locks = r.locks;
    s.lockNotes = r.lock_notes || {};
    s.profile = new Set(r.profile || []);
    s.profileEdit = new Set(r.profile_edit || []);
    // A sample's held box that cannot be typed says what it waits for.
    s.needs = r.needs || {};
    s.waits = r.waits || {};
    s.off = r.off || {};
    s.penalties = r.penalties || [];
    s.poolTexts = r.pools || {};
    s.saved = Boolean(r.draft && r.draft.id);
    // An untouched date whose message waits (the host's `quiet`: 1800's and
    // 1801's item 1 on a new return) says nothing until the filer edits it
    // or presses Check or Save.
    s.quiet = r.quiet || null;
    this.setErrors(r.errors);
  }

  /** The filer edited a quiet date, or asked for every message: it speaks. */
  unquiet(keys) {
    const s = this.state;
    if (s?.quiet && (!keys || keys.some((k) => s.quiet.keys.includes(k)))) s.quiet = null;
  }

  /** Create InputStates for one part's text-like controls (only once). */
  ensureInputs(partIndex, cx) {
    const s = this.state;
    if (!s || !s.schema || !s.draft) return;
    const part = s.schema.parts[partIndex];
    if (!part) return;
    const make = (k, t) => {
      if (s.inputs.has(k)) return;
      let value = s.values[k] ?? "";
      if (t === "money" && value !== "") value = formatMoney(value, this.isWhole(k));
      if (t === "date") value = shownDate(value);
      const input = InputState.new({ value: String(value) });
      if (t === "date") {
        // Typing stays as typed; the draft gets MM/DD/YYYY as soon as it
        // reads as a date (12102020, 12-10-2020 and 12/10/2020 alike), and
        // on blur the box shows it as "Dec 10, 2020".
        input.on("change", (_e, cx) => {
          const raw = input.value();
          const date = parseTypedDate(raw);
          if (date || !raw.trim()) this.setDateError(k, "");
          this.edit(k, date ? formatMDY(date) : raw, cx);
        });
        input.on("blur", (_e, cx) => {
          const raw = input.value();
          const date = parseTypedDate(raw);
          if (date) {
            const text = formatLongDate(date);
            if (text !== raw) input.set_value(text);
            this.setDateError(k, "");
          } else this.setDateError(k, raw.trim() ? DATE_ERROR : "");
          this.leaveBox(k, cx);
          this.flush(cx);
        });
        input.on("focus", () => this.enterBox(k));
        s.inputs.set(k, input);
        (s.inputKinds ||= new Map()).set(k, t);
        return;
      }
      const pool = this.poolOf(k);
      if (pool) input.set_value(this.poolText({ pool }));
      input.on("change", (_e, cx) => (pool ? this.editPool(pool, input.value(), cx) : this.edit(k, input.value(), cx)));
      input.on("blur", (_e, cx) => {
        if (t === "money") input.set_value(formatMoney(input.value(), this.isWhole(k)));
        // Leaving an edited profile item keeps the value on this return and
        // gives its boxes their profile look (and Edit) back.
        this.leaveBox(k, cx);
        this.flush(cx);
      });
      input.on("focus", () => this.enterBox(k));
      s.inputs.set(k, input);
      (s.inputKinds ||= new Map()).set(k, t);
    };
    for (const it of part.items) {
      const date = oneDate(it);
      if (date) {
        this.makeOneDate(date);
        continue;
      }
      if (it.rows) {
        it.keys.forEach((row) => row.forEach((k, c) => make(k, it.cols[c].t)));
      } else {
        for (const f of it.f || []) if (["text", "money", "pct", "int", "date", "email", "phone"].includes(f.t)) make(f.k, f.t);
      }
    }
    if (cx) {
      // So is the part's "From tax profile" fold (Enter or Space unfolds it).
      const fold = `fold-${partIndex}`;
      if (cx.focus_handle && !s.editFocus.has(fold) && this.tuckedItems(partIndex).length) {
        s.editFocus.set(fold, cx.focus_handle());
      }
      part.items.forEach((it, ii) => {
        const id = `${partIndex}:${ii}`;
        // The Edit button of a profile item is a tab stop (Tab, then Enter).
        if (cx.focus_handle && !s.editFocus.has(id) && (it.f || []).some((f) => s.profileEdit.has(f.k))) {
          s.editFocus.set(id, cx.focus_handle());
        }
        if (!s.rowViews.has(id)) {
          const props = { ui: this, state: s, it, pi: partIndex, ii, width: this.rowWidth() };
          s.rowViews.set(id, {
            props,
            snapshot: this.rowSnapshot(props),
            entity: cx.new(FormItemView, props),
          });
        }
      });
      this.refreshRows();
    }
  }

  /**
   * The one date box of a `one_date` item: what it reads goes to the month,
   * day and year boxes together as soon as it reads as a date (12102020,
   * 12-10-2020, 12/10/2020 or "Dec 10, 2020"), and a blank box clears all
   * three; on blur it shows "Dec 10, 2020", or says what it needs.
   */
  makeOneDate(date) {
    const s = this.state;
    const k = date.oneKey;
    if (s.inputs.has(k)) return;
    const keys = [date.month.k, date.day.k, date.year.k];
    const input = InputState.new({ value: shownDate(oneDateText(s.values, date)) });
    input.on("change", (_e, cx) => {
      const raw = input.value();
      const typed = parseTypedDate(raw);
      if (typed || !raw.trim()) this.setDateError(keys[0], "");
      if (!typed && raw.trim()) return;
      const [m, d, y] = typed ? formatMDY(typed).split("/") : ["", "", ""];
      [m, d, y].forEach((v, i) => this.edit(keys[i], v, cx));
    });
    input.on("blur", (_e, cx) => {
      const raw = input.value();
      const typed = parseTypedDate(raw);
      if (typed) {
        const text = formatLongDate(typed);
        if (text !== raw) input.set_value(text);
        this.setDateError(keys[0], "");
      } else this.setDateError(keys[0], raw.trim() ? DATE_ERROR : "");
      this.leaveBox(keys[0], cx);
      this.flush(cx);
    });
    input.on("focus", () => this.enterBox(keys[0]));
    s.inputs.set(k, input);
    (s.inputKinds ||= new Map()).set(k, "date");
    (s.oneDates ||= new Map()).set(k, date);
  }

  rowWidth() {
    const width = this.app.layout().column;
    return width >= BREAK.formNav ? width - 208 - 28 : width;
  }

  /** Values used by one row; fresh host objects alone do not invalidate it. */
  rowSnapshot({ state: s, it, pi, ii, width }) {
    const id = `${pi}:${ii}`;
    const keys = itemKeys(it);
    const picker = s.picker?.row === id ? s.picker.cal : null;
    return JSON.stringify([
      width, s.editable, s.messages[id] || "", s.dateErrors?.[id] || "", s.moreOpen.has(id),
      keys.includes(s.openSelect) ? s.openSelect : null,
      picker ? [picker.year(), picker.month(), picker.value(), s.picker.select || null] : null,
      s.hoverItem === id,
      keys.map(k => [s.values[k] ?? "", s.poolTexts?.[k] ?? "", s.locks[k], s.off?.[k] || "", this.penaltySignature(k), s.profile.has(k), s.editing.has(k), s.profileEdit.has(k), this.optionsSignature(k)]),
    ]);
  }

  /**
   * One box's choices as text, computed once per host reply rather than once
   * per row per refresh: `s.options` is replaced whole on every fill, so the
   * memo lives as long as that object.
   */
  optionsSignature(k) {
    const s = this.state;
    if (s.optionSigsFor !== s.options) {
      s.optionSigsFor = s.options;
      s.optionSigs = new Map();
    }
    let sig = s.optionSigs.get(k);
    if (sig === undefined) {
      sig = s.options[k] === undefined ? "" : JSON.stringify(s.options[k]);
      s.optionSigs.set(k, sig);
    }
    return sig;
  }

  /** Retained rows keep their snapshots until something they display changes. */
  refreshRows() {
    const s = this.state;
    if (!s?.rowViews) return;
    const width = this.rowWidth();
    s.rowLayoutWidth = width;
    for (const row of s.rowViews.values()) {
      if (!s.openParts.has(row.props.pi)) continue;
      const props = { ...row.props, width };
      const snapshot = this.rowSnapshot(props);
      if (snapshot === row.snapshot) continue;
      row.props = props;
      row.snapshot = snapshot;
      row.entity.set_props(props);
    }
  }

  refreshLayout() {
    if (this.state && this.state.rowLayoutWidth !== this.rowWidth()) this.refreshRows();
  }

  /** Header and navigation may change too; row callbacks have their own cx. */
  notify(cx) {
    this.refreshRows();
    const check = this.state.checkButton;
    if (check && check.checking !== this.state.checking) {
      check.checking = this.state.checking;
      check.entity.set_props({ ui: this, state: this.state });
    }
    const snapshot = this.chromeSnapshot();
    if (snapshot === this.state.chromeSnapshot) return;
    this.state.chromeSnapshot = snapshot;
    if (this.app.invalidate) this.app.invalidate();
    else cx.notify();
  }

  chromeSnapshot() {
    const s = this.state;
    return JSON.stringify([
      s.status, s.editable, s.loading, s.error,
      Object.keys(s.messages).sort(), s.formErrors,
    ]);
  }

  togglePart(i, cx) {
    const s = this.state;
    if (s.openParts.has(i)) s.openParts.delete(i);
    else {
      this.ensureInputs(i, cx);
      s.openParts.add(i);
    }
    this.refreshRows();
    cx.notify();
  }

  /** Navigator: open the chosen part and fold the others, so it sits near the top. */
  jumpTo(i, cx) {
    const s = this.state;
    this.ensureInputs(i, cx);
    s.openParts = new Set([i]);
    this.refreshRows();
    cx.notify();
  }

  /** Stage an edit; the draft takes it on the next flush (300 ms after the last edit). */
  edit(k, value, cx) {
    const s = this.state;
    if (!s || !s.editable) return;
    if (s.periodParts?.has(k)) return this.setPeriodPart({ [k]: value }, cx);
    // Only what is typed is sent: in a penalty group opened with Edit the
    // boxes left alone stay computed (rule 8). The page refilling a box
    // (InputState.set_value) emits no "change" (gpui turns events off for
    // it), so focusing or reformatting a box never sends it.
    s.pending[k] = value;
    s.dirty = true;
    this.unquiet([k]);
    const token = this.token;
    s.editSeq = (s.editSeq || 0) + 1;
    const seq = s.editSeq;
    cx.timer.after(VALIDATE_DELAY, (acx) => {
      if (token !== this.token || seq !== this.state.editSeq) return;
      this.flush(acx);
    });
    // Live sync to an open print preview, sooner than the draft flush.
    cx.timer.after(LIVE_PREVIEW_DELAY, () => {
      if (token !== this.token || seq !== this.state.editSeq) return;
      this.livePreview();
    });
  }

  /** Push draft + staged edits to the open print preview (no-op if none is open). */
  livePreview() {
    const s = this.state;
    if (!s || !s.previewOpen) return;
    try {
      const pending = s.pending || {};
      this.app.store.call("screen_form_preview_sync", s.code, s.draft, pending, this.unlockedFor(pending));
    } catch (_e) {
      /* preview sync is best effort */
    }
  }

  /** A typed date's own message; only its row's snapshot changes. */
  setDateError(k, msg) {
    const s = this.state;
    const id = this.itemIdForKey(k);
    if (!s || !id) return;
    s.dateErrors ||= {};
    if ((s.dateErrors[id] || "") === msg) return;
    if (msg) s.dateErrors[id] = msg;
    else delete s.dateErrors[id];
  }

  /** Fill an item's month, day and year boxes (or its one date box) from a picked "YYYY-MM-DD". */
  pickDate(date, iso, cx) {
    const s = this.state;
    const [y, m, d] = iso.split("-");
    if (date.single) {
      const f = date.single;
      if (f.c || this.isLocked(f.k)) return;
      const text = formatMDY({ y: Number(y), m: Number(m), d: Number(d) });
      s.inputs.get(f.k)?.set_value(shownDate(text));
      this.setDateError(f.k, "");
      s.picker = null;
      this.choose({ [f.k]: text }, cx);
      return;
    }
    const edits = {};
    const put = (f, v) => {
      if (!f || f.c || this.isLocked(f.k)) return;
      let value = v;
      if (f.t === "sel" && f.o) {
        const hit = f.o.map(optionOf).find((o) => o && (o.value === v || o.value === String(Number(v))));
        value = hit ? hit.value : v;
      }
      edits[f.k] = value;
      const input = s.inputs.get(f.k);
      if (input) input.set_value(String(value));
    };
    put(date.month, m);
    put(date.day, d);
    put(date.year, y);
    if (date.oneKey) {
      s.inputs.get(date.oneKey)?.set_value(formatLongDate({ y: Number(y), m: Number(m), d: Number(d) }));
      this.setDateError(date.month.k, "");
    }
    s.picker = null;
    this.choose(edits, cx);
  }

  /** Choice controls commit at once. */
  choose(edits, cx) {
    const s = this.state;
    if (!s || !s.editable) return;
    const period = Object.fromEntries(Object.entries(edits).filter(([k]) => s.periodParts?.has(k)));
    if (Object.keys(period).length) {
      this.setPeriodPart(period, cx);
      edits = Object.fromEntries(Object.entries(edits).filter(([k]) => !(k in period)));
      if (!Object.keys(edits).length) return;
    }
    Object.assign(s.pending, edits);
    s.dirty = true;
    this.unquiet(Object.keys(edits));
    // What the boxes showed, for an answer the filer then cancels.
    s.chosenFrom = Object.fromEntries(Object.keys(edits).map((k) => [k, s.values[k]]));
    for (const [k, v] of Object.entries(edits)) s.values[k] = String(v);
    // flush() syncs the preview from the host-applied draft.
    this.flush(cx);
  }

  /** Take what the host applied for `edits`. */
  applyFill(r, edits) {
    const s = this.state;
    this.accepted(edits, r.rejected);
    // The host says which of a sample's boxes are still blank.
    if (s.sample && Array.isArray(r.blank)) s.blank = new Set(r.blank);
    const before = s.values;
    s.draft = r.draft;
    s.values = this.shown(r.values);
    s.options = r.options || {};
    if (Array.isArray(r.penalties)) s.penalties = r.penalties;
    if (r.pools) s.poolTexts = r.pools;
    this.syncComputed();
    this.syncChanged(before, edits);
    // The preview prints the draft the host just applied, never the
    // page's optimistic values.
    this.livePreview();
    const rejected = Object.entries(r.rejected || {});
    if (rejected.length) {
      for (const [k, msg] of rejected) s.messages[this.itemIdForKey(k) || "form"] = msg;
    }
  }

  /**
   * An answer that removes rows the filer typed (the host's `removes`) is
   * asked first: the page keeps its draft and shows the old answer until
   * Continue (`confirmRemoval`); Cancel (`cancelRemoval`) leaves both.
   */
  holdRemoval(r, edits, cx) {
    const s = this.state;
    const chosen = s.chosenFrom || {};
    for (const k of Object.keys(edits)) if (k in chosen) s.values[k] = chosen[k] ?? "";
    // What was typed in the same batch is not the answer: Cancel keeps it.
    const typed = Object.fromEntries(Object.entries(edits).filter(([k]) => !(k in chosen)));
    s.removal = { r, edits, typed, draft: s.draft };
    this.app.overlay = "removal";
    this.refreshRows();
    cx.notify();
  }

  /** Continue: the answer and its removal apply; Undo brings both back. */
  confirmRemoval(cx) {
    const s = this.state;
    const held = s.removal;
    if (!held) return;
    s.removal = null;
    this.app.overlay = null;
    this.applyFill(held.r, held.edits);
    s.dirty = true;
    this.refreshRows();
    this.validate(cx);
    // Undo belongs to this return's page as it is now: another return, or
    // any later edit here, retires it (it would bring back a stale draft).
    const undo = (ucx) => this.undoRemoval(held.draft, s, undo, ucx);
    this.removalUndo = { state: s, undo };
    this.offerUndo(held.r.removes.done, undo, cx);
    cx.notify();
  }

  /** Cancel: the old answer and the rows stay, and so does what was typed with it. */
  cancelRemoval(cx) {
    const s = this.state;
    const held = s?.removal;
    if (s) s.removal = null;
    this.app.overlay = null;
    if (held && Object.keys(held.typed).length) {
      s.pending = { ...held.typed, ...s.pending };
      this.flush(cx);
    }
    this.refreshRows();
    cx.notify();
  }

  /** The removal question is on screen (main.js blocks Cmd+Z and cancels it before Cmd+K or New return). */
  askingRemoval() {
    return Boolean(this.state?.removal);
  }

  /** The app leaves this page (main.js `go`): a removal's Undo stays behind. */
  leavingPage() {
    this.retireRemovalUndo();
  }

  /** The removal's Undo, if still offered, goes (a later edit, another page). */
  retireRemovalUndo() {
    const held = this.removalUndo;
    if (!held) return;
    this.removalUndo = null;
    if (this.app.toastState?.action?.run === held.undo) {
      this.app.toastState = null;
      this.app.invalidate?.();
    }
  }

  /** Undo a removal: the draft from before the answer comes back, on the same page only. */
  undoRemoval(draft, state, undo, cx) {
    const s = this.state;
    if (!s || s !== state || this.removalUndo?.undo !== undo) return;
    this.removalUndo = null;
    try {
      const r = this.app.store.call("screen_form_fill", s.code, draft, {}, []);
      this.applyFill(r, {});
    } catch (e) {
      return this.app.toast(message(e), cx, "triangle-alert");
    }
    s.dirty = true;
    this.refreshRows();
    this.validate(cx);
    cx.notify();
  }

  /** Apply staged edits to the typed draft (fast, no database), then validate off-thread. */
  flush(cx) {
    const s = this.state;
    if (!s || !s.draft) return;
    const edits = s.pending;
    s.pending = {};
    if (Object.keys(edits).length) {
      this.retireRemovalUndo();
      try {
        const r = this.app.store.call("screen_form_fill", s.code, s.draft, edits, this.unlockedFor(edits));
        if (r.removes) return this.holdRemoval(r, edits, cx);
        this.applyFill(r, edits);
      } catch (e) {
        // The draft never took these edits: keep them staged (under anything
        // typed since) so the next flush or save sends them again.
        s.pending = { ...edits, ...s.pending };
        this.app.toast(message(e), cx, "triangle-alert");
      }
    }
    this.validate(cx);
  }

  /**
   * A sample's period part changed: the page keeps it (it is never a draft
   * write) and asks the host about the period PERIOD_ASK_DELAY after the
   * last change, so a year typed digit by digit is asked about once. Any
   * change closes an offer still on screen and forgets the last answer: an
   * offer, Start a return and the fast lane only ever act on the answer for
   * the period the boxes show now. A new complete period rebuilds the
   * sample for it, with what the filer typed elsewhere applied again; a
   * period that already has a stored return is offered (offerExisting),
   * never opened by itself.
   */
  setPeriodPart(parts, cx) {
    const s = this.state;
    s.periodValues = { ...(s.periodValues || {}), ...parts };
    for (const [k, v] of Object.entries(parts)) {
      const text = String(v);
      s.values[k] = text;
      // A part set by a click or a picker shows in its box too.
      const input = s.inputs.get(k);
      if (input && input.value() !== text) input.set_value(text);
    }
    const asked = JSON.stringify(s.periodValues);
    if (asked === s.periodAsked) return this.notify(cx);
    s.periodAsked = asked;
    this.closeOffer();
    s.opens = null;
    s.existing = null;
    const token = this.token;
    const seq = (s.periodSeq = (s.periodSeq || 0) + 1);
    cx.timer.after(PERIOD_ASK_DELAY, (tcx) => {
      if (token !== this.token || seq !== this.state.periodSeq) return;
      this.askPeriod(asked, tcx);
    });
    this.notify(cx);
  }

  /** Ask the host about the period the boxes show (`asked`, the parts as JSON). */
  askPeriod(asked, cx) {
    const s = this.state;
    const token = this.token;
    s.periodInFlight = asked;
    cx.spawn(async (acx) => {
      let r;
      try {
        r = await this.app.store.call("screen_form_sample_period", s.tin, s.code, JSON.parse(asked));
      } catch (e) {
        if (s.periodInFlight === asked) s.periodInFlight = null;
        s.startAfterAnswer = false;
        if (token === this.token) this.app.toast(message(e), acx, "triangle-alert");
        return;
      }
      if (s.periodInFlight === asked) s.periodInFlight = null;
      if (token !== this.token || asked !== this.state.periodAsked) return;
      s.periodAnswered = asked;
      this.takePeriod(r, acx);
      // Start a return pressed while this answer was pending acts on it now
      // (an existing return was offered by the answer itself).
      if (s.startAfterAnswer) {
        s.startAfterAnswer = false;
        if (this.state === s && !s.existing) this.startReturn(acx);
      }
    });
  }

  /**
   * The host's answer for a sample's typed period. A fresh sample is built
   * for a new complete period. A sample built for an earlier period goes
   * back to the placeholder sample once its period is incomplete or has a
   * stored return, so nothing on screen belongs to a period the boxes no
   * longer show; a stored return is then offered.
   */
  takePeriod(r, cx) {
    const s = this.state;
    s.opens = r.complete ? r.opens || null : null;
    s.existing = r.existing || null;
    const period = (s.periodToken = (s.periodToken || 0) + 1);
    this.closeOffer();
    const offer = (acx) => {
      if (r.existing) this.offerExisting(r.existing, acx);
    };
    if (r.sample) {
      this.rebuild(r.sample, cx);
      offer(cx);
    } else if (s.periodSet) {
      this.toPlaceholder(period, cx, offer);
    } else {
      offer(cx);
    }
    if (this.state === s) this.notify(cx);
  }

  /** The placeholder sample again (the period not set), then `then`. */
  toPlaceholder(period, cx, then) {
    const s = this.state;
    const token = this.token;
    cx.spawn(async (acx) => {
      let r;
      try {
        r = await this.app.store.call("screen_form_sample", s.tin, s.code);
      } catch (e) {
        if (token === this.token) this.app.toast(message(e), acx, "triangle-alert");
        return;
      }
      if (token !== this.token || period !== this.state.periodToken) return;
      this.rebuild(r, acx);
      if (then) then(acx);
      if (this.state === s) this.notify(acx);
    });
  }

  /** Take a sample the host built, with what the filer typed outside the period applied again. */
  rebuild(sample, cx) {
    const s = this.state;
    const before = s.values;
    this.takeOpened(sample);
    const typed = s.typedEdits || {};
    if (Object.keys(typed).length) {
      try {
        const f = this.app.store.call("screen_form_fill", s.code, s.draft, typed, this.unlockedFor(typed));
        s.draft = f.draft;
        s.values = this.shown(f.values);
        s.options = f.options || {};
      } catch (e) {
        this.app.toast(message(e), cx, "triangle-alert");
      }
    }
    this.syncComputed();
    this.syncChanged(before, { ...typed, ...(s.periodValues || {}) });
    this.validate(cx);
  }

  /** The stored-return offer leaves the screen (a new answer, a choice, or leaving the sample). */
  closeOffer() {
    const app = this.app;
    if (app.overlay === "existing") app.overlay = null;
    app.existingOffer = null;
  }

  /**
   * A stored return already has the sample's period: the app's modal
   * offers what its status allows. `existing` carries the period, its
   * label, status, last save and total payable.
   */
  offerExisting(existing, cx) {
    const app = this.app;
    this.state.existing = existing;
    // "Don't ask again for drafts" (until the app quits or the session
    // ends): a draft loads straight away. Queued and filed returns always ask.
    if (existing.status === "Draft" && app.fastLane?.drafts) return this.takeExisting("load", cx, true);
    app.existingOffer = existing;
    app.overlay = "existing";
    cx.notify();
  }

  /**
   * The filer's choice for a stored return of the sample's period: `load`
   * (a draft), `view` (read-only), `amend` (a new amended return), `keep`
   * (stay in the sample, its period cleared). It acts only on the answer
   * for the period the boxes show now, and opens exactly the return that
   * answer matched (`opens`: its year, period and stored key). Each can be
   * undone once; the Undo toast shows only once the return opened.
   */
  takeExisting(choice, cx, auto = false) {
    const app = this.app;
    const s = this.state;
    const offered = auto ? null : app.existingOffer;
    const existing = auto ? s?.existing : offered;
    this.closeOffer();
    if (!s || !s.sample || !existing || existing !== s.existing || !s.opens) return cx.notify();
    if (choice === "keep") {
      const before = this.periodSnapshot();
      this.keepForm(cx);
      return this.offerUndo("Kept this form; its period is cleared", (ucx) => this.restorePeriod(before, existing, ucx), cx);
    }
    const sample = s;
    const { year, period, key } = s.opens;
    const opts = { tin: s.tin, code: s.code, year, period, ...(key != null ? { returnKey: key } : {}) };
    const what = {
      load: auto ? `Loaded your saved ${existing.label} draft (you chose not to be asked again)` : `Loaded your saved ${existing.label} draft`,
      view: `Opened ${existing.label} read-only`,
      amend: `Started an amended ${existing.label} return`,
    }[choice];
    const then = (error, acx) => {
      if (error) {
        // Nothing opened: back to the sample, and say why.
        this.restoreForm(sample, acx);
        return this.app.toast(`Could not open ${existing.label}: ${error}`, acx, "triangle-alert");
      }
      this.offerUndo(what, (ucx) => this.restoreForm(sample, ucx, offered), acx);
    };
    if (choice === "amend") this.open({ ...opts, amend: true }, cx, true, then);
    else app.openForm(opts, cx, true, then);
  }

  /** The Undo window closed: the form kept for it goes. */
  dropKept() {
    if (this.kept && this.kept !== this.state) this.release(this.kept);
    this.kept = null;
  }

  /** "<what happened>. Undo" for a few seconds; Undo runs `undo` once. */
  offerUndo(what, undo, cx) {
    if (this.app.toastUndo) this.app.toastUndo(`${what}.`, undo, cx);
  }

  /**
   * Undo a Load, View or Amend: the sample comes back exactly as it was,
   * with the offer it was answered from (`offer`) open again. An amended
   * return that was never saved is simply dropped; one already saved stays
   * saved (nothing is deleted) and the toast says so. Edits not saved yet
   * are never dropped: Undo refuses and says why.
   */
  restoreForm(sample, cx, offer = null) {
    if (!sample || this.state === sample) return;
    const s = this.state;
    if (s && (s.dirty || Object.keys(s.pending || {}).length)) {
      return this.app.toast("Undo would lose edits you have not saved. Save them first, or leave this return.", cx, "triangle-alert");
    }
    const savedAmendment = Boolean(s?.amends && s.saved);
    this.release(s);
    this.kept = null;
    this.state = sample;
    this.token++;
    if (offer && sample.existing === offer) {
      this.app.existingOffer = offer;
      this.app.overlay = "existing";
    }
    this.app.go?.("form", cx);
    this.refreshRows();
    if (savedAmendment) this.app.toast("Back to the sample. The amended return was saved and stays saved.", cx, "undo-2");
    if (this.app.invalidate) this.app.invalidate();
    else cx.notify();
  }

  /** What Keep clears, to put back on Undo: the fields and every box's text. */
  periodSnapshot() {
    const s = this.state;
    const inputs = {};
    for (const [k, input] of s.inputs) inputs[k] = input.value();
    return {
      state: s,
      fields: {
        draft: s.draft,
        values: { ...s.values },
        options: s.options,
        locks: s.locks,
        lockNotes: s.lockNotes,
        blank: s.blank,
        needs: s.needs,
        errors: s.errors,
        messages: s.messages,
        formErrors: s.formErrors,
        periodValues: { ...(s.periodValues || {}) },
        periodAsked: s.periodAsked,
        periodSet: s.periodSet,
        opens: s.opens,
        existing: s.existing,
      },
      inputs,
    };
  }

  /**
   * Undo a Keep: the period and everything built from it come back, every
   * box showing exactly what it showed, and the offer it was kept from.
   */
  restorePeriod(snapshot, offer, cx) {
    const s = this.state;
    if (s !== snapshot.state) return;
    s.periodToken = (s.periodToken || 0) + 1;
    s.periodSeq = (s.periodSeq || 0) + 1;
    Object.assign(s, snapshot.fields);
    for (const [k, input] of s.inputs) {
      let text = snapshot.inputs[k];
      if (text === undefined) {
        // A box made after the Keep shows the restored value.
        const v = s.values[k] ?? "";
        text = s.inputKinds?.get(k) === "money" && v !== "" ? formatMoney(v) : String(v);
      }
      if (input.value() !== text) input.set_value(text);
    }
    if (offer && s.existing === offer) {
      this.app.existingOffer = offer;
      this.app.overlay = "existing";
    }
    this.refreshRows();
    if (this.app.invalidate) this.app.invalidate();
    else cx?.notify();
  }

  /**
   * Keep this form: the same sample with every value the filer typed
   * outside the period, and every period part blank again (the period
   * "not set"), rebuilt on the placeholder period the host holds.
   */
  keepForm(cx) {
    const s = this.state;
    s.periodValues = {};
    s.periodAsked = JSON.stringify({});
    s.periodSeq = (s.periodSeq || 0) + 1;
    s.opens = null;
    s.existing = null;
    for (const k of s.periodParts?.keys() || []) {
      s.values[k] = "";
      s.inputs.get(k)?.set_value("");
    }
    if (s.periodSet) {
      // The sample was built for an earlier period: back to the placeholder.
      this.toPlaceholder((s.periodToken = (s.periodToken || 0) + 1), cx, null);
    }
    this.notify(cx);
  }

  /**
   * Start a return from a sample: the return its period matched (exactly
   * that one, by its stored key), offered first when one is stored; New
   * return while the period is not set, and always for a form whose
   * returns are numbered entries rather than periods (no `opens`).
   */
  startReturn(cx) {
    const s = this.state;
    // A period change still waiting for its answer (the pause before the
    // ask): ask now, and act once the answer is in.
    if (s.periodAsked && s.periodAnswered !== s.periodAsked && Object.keys(s.periodValues || {}).length) {
      s.startAfterAnswer = true;
      if (s.periodInFlight !== s.periodAsked) {
        s.periodSeq = (s.periodSeq || 0) + 1;
        this.askPeriod(s.periodAsked, cx);
      }
      return;
    }
    if (!s.opens) return this.app.startNewReturn(cx, s.code);
    if (s.existing) return this.offerExisting(s.existing, cx);
    const { year, period, key, new_key: newKey } = s.opens;
    this.app.openForm(
      { tin: s.tin, code: s.code, year, period, ...(key != null ? { returnKey: key } : newKey != null ? { newKey } : {}) },
      cx,
    );
  }

  /** Computed and locked boxes follow the draft; user boxes keep what was typed. */
  syncComputed() {
    const s = this.state;
    for (const [k, input] of s.inputs) {
      if (!(this.isLocked(k) || this.isComputed(k))) continue;
      const pool = this.poolOf(k);
      const kind = s.inputKinds?.get(k);
      const raw = String(s.values[k] ?? "");
      const v = pool ? this.poolText({ pool }) : kind === "date" ? shownDate(raw) : kind === "money" && raw !== "" ? formatMoney(raw, this.isWhole(k)) : raw;
      if (input.value() !== v) input.set_value(v);
    }
  }

  /** The pool (rule 5) whose first line is `k`, if any. */
  poolOf(k) {
    const s = this.state;
    if (!s.pools) {
      s.pools = new Map();
      for (const p of s.schema.parts) for (const it of p.items) for (const f of it.f || []) if (f.pool) s.pools.set(f.k, f.pool);
    }
    return s.pools.get(k) || null;
  }

  /**
   * A pooled address as one text, as the host reads it from the draft
   * (`pools`): never the field map's lines, which are cut at a fixed width.
   */
  poolText(f) {
    return String(this.state.poolTexts?.[f.pool.keys[0]] ?? "");
  }

  /**
   * Stage a pooled address: the whole text goes to its first line, and the
   * host writes it into the draft's own fields (combs::write_pool), split
   * over the lines only where each has a field of its own.
   */
  editPool(pool, text, cx) {
    this.edit(pool.keys[0], text, cx);
  }

  /**
   * A box the filer can type in but the host also fills (a 2550M schedule's
   * input tax refilled at 12% from its amount, rows moving up when one is
   * cleared) shows the host's new value. Boxes in this batch keep the typing.
   */
  syncChanged(before, edits) {
    const s = this.state;
    for (const [k, input] of s.inputs) {
      if (k in edits) continue;
      const pool = this.poolOf(k);
      if (pool) {
        const now = this.poolText({ pool });
        if (input.value() !== now) input.set_value(now);
        continue;
      }
      const was = before[k] ?? "";
      const now = s.values[k] ?? "";
      if (was === now) continue;
      const kind = s.inputKinds?.get(k);
      const shown = kind === "money" && now !== "" ? formatMoney(now, this.isWhole(k)) : kind === "date" ? shownDate(now) : String(now);
      if (input.value() !== shown) input.set_value(shown);
    }
    for (const [k, date] of s.oneDates || []) {
      const keys = [date.month.k, date.day.k, date.year.k];
      if (keys.some((part) => part in edits)) continue;
      const was = oneDateText(before, date);
      const now = oneDateText(s.values, date);
      const input = s.inputs.get(k);
      if (input && was !== now && input.value() !== shownDate(now)) input.set_value(shownDate(now));
    }
  }

  isComputed(k) {
    const s = this.state;
    if (!s.computedKeys) {
      s.computedKeys = new Set();
      for (const p of s.schema.parts) {
        for (const it of p.items) {
          if (it.rows) it.cols.forEach((c, ci) => c.c && it.keys.forEach((row) => s.computedKeys.add(row[ci])));
          else for (const f of it.f || []) if (f.c) s.computedKeys.add(f.k);
        }
      }
    }
    return s.computedKeys.has(k);
  }

  /**
   * Why an item's shut boxes show blank, in the filer's words (the host's
   * `lock_notes`; a box the form computes: "Computed by the form."), once
   * each. A box saying what it waits for (`needs`, or the tag of a box
   * another answer turns off) says it there instead; a computed amount shows
   * 0.00, never blank.
   */
  lockNotesFor(it) {
    const s = this.state;
    if (!s.editable || !s.draft) return [];
    const notes = [];
    const note = (k) => {
      const text = s.lockNotes?.[k] || (this.isComputed(k) ? "Computed by the form." : "");
      if (text && !notes.includes(text)) notes.push(text);
    };
    const shut = (k, c) => c || this.isLocked(k);
    const waits = (k) => Boolean(this.reasonFor(k) || this.isOff(k));
    const ticked = (k) => ["true", "1", "Y"].includes(String(s.values[k] ?? ""));
    const blankText = (k, t) => {
      const v = String(s.values[k] ?? "").trim();
      if (t === "money" && this.isComputedBox(k)) return false;
      if (t === "sel") return v === "" || /^(0+|-)$/.test(v);
      return v === "";
    };
    if (it.rows) {
      it.keys.forEach((row) =>
        row.forEach((k, c) => {
          const col = it.cols[c];
          if (col.t === "hidden" || !shut(k, col.c) || waits(k)) return;
          if (col.t === "radio" || col.t === "chk" ? !ticked(k) : blankText(k, col.t)) note(k);
        }),
      );
      return notes;
    }
    const fs = it.f || [];
    const radios = fs.filter((f) => f.t === "radio");
    if (radios.length && radios.every((f) => shut(f.k, f.c)) && !radios.some((f) => ticked(f.k)) && !radios.some((f) => waits(f.k))) {
      note(radios[0].k);
    }
    for (const f of fs) {
      if (f.t === "radio" || f.t === "hidden" || !shut(f.k, f.c) || waits(f.k)) continue;
      if (f.t === "chk" ? !ticked(f.k) : blankText(f.k, f.t)) note(f.k);
    }
    return notes;
  }

  /**
   * A locked blank box: what it waits for, shown in its place. A sample's
   * held box says so (`needs`); a box the form fills from another box of
   * its row says which (the host's `waits`: a rate waits for its ATC).
   */
  reasonFor(k) {
    const s = this.state;
    return (s.blank.has(k) && s.needs?.[k]) || s.waits?.[k] || "";
  }

  /** Computed by the form (the schema's flag, or the host's lock): an amount shows 0.00 for zero. */
  isComputedBox(k) {
    return this.isComputed(k) || /^computed/.test(this.state.locks[k] || "");
  }

  /** A peso box over a whole-number field (the host's `w`): no centavos. */
  isWhole(k) {
    const s = this.state;
    if (!s.wholeKeys) {
      s.wholeKeys = new Set();
      for (const p of s.schema.parts) {
        for (const it of p.items) {
          if (it.rows) it.cols.forEach((c, ci) => c.w && it.keys.forEach((row) => s.wholeKeys.add(row[ci])));
          else for (const f of it.f || []) if (f.w) s.wholeKeys.add(f.k);
        }
      }
    }
    return s.wholeKeys.has(k);
  }

  /**
   * Locked on the page: a host lock, unless it is a profile box opened with
   * Edit; or a box another answer turns off (rule 6), until it holds.
   */
  isLocked(k) {
    const s = this.state;
    if (this.isOff(k)) return true;
    const penalty = this.penaltyGroupOf(k);
    if (penalty) return !penalty.applies || !(penalty.edited || s.penaltyOpen.has(k));
    return s.locks[k] !== undefined && !(s.profileEdit.has(k) && s.editing.has(k));
  }

  /** A penalty box's state for row snapshots: edited, part of the return, opened. */
  penaltySignature(k) {
    const g = this.penaltyGroupOf(k);
    return g ? `${g.edited}:${g.applies}:${this.state.penaltyOpen.has(k)}` : "";
  }

  /** The penalty group (rule 8) `k` belongs to, if any. */
  penaltyGroupOf(k) {
    return (this.state.penalties || []).find((g) => g.keys.includes(k)) || null;
  }

  /** Edit penalties: the group's three boxes open together. */
  openPenalties(group, cx) {
    const s = this.state;
    if (!s.editable || !group.applies) return;
    for (const k of group.keys) s.penaltyOpen.add(k);
    this.refreshRows();
    cx.notify();
  }

  /** Use the computed amounts: the group's overrides are cleared. */
  usePenaltyComputed(group, cx) {
    const s = this.state;
    for (const k of group.keys) s.penaltyOpen.delete(k);
    if (group.edited) this.choose(Object.fromEntries(group.keys.map((k) => [k, ""])), cx);
    this.refreshRows();
    cx.notify();
  }

  /** What turns `k` on, while another answer has it off (the host's `off`). */
  isOff(k) {
    return this.state.off?.[k] || "";
  }

  /**
   * The values a page shows: on a Form library sample its period and date
   * boxes stay blank until the filer types one (rule 1).
   */
  shown(values) {
    const s = this.state;
    if (!s.sample || !s.blank.size) return values;
    const out = { ...values };
    for (const k of s.blank) {
      // A period part shows what the filer typed into it.
      if (s.periodParts?.has(k)) out[k] = s.periodValues?.[k] ?? "";
      else if (k in out && !s.typed.has(k)) out[k] = "";
    }
    return out;
  }

  /**
   * The boxes of an edit batch the page has open (screen_form_fill's
   * `unlocked`): a box with no lock, and a profile box the filer opened
   * with Edit. A box locked for any other reason is never sent.
   */
  unlockedFor(edits) {
    const s = this.state;
    return Object.keys(edits).filter((k) => s.locks[k] === undefined || s.opened.has(k) || s.penaltyOpen.has(k));
  }

  /** A sample shows a box it blanked once the host has taken an edit for it. */
  accepted(edits, rejected) {
    const s = this.state;
    for (const [k, v] of Object.entries(edits)) {
      if (rejected && k in rejected) continue;
      s.typed.add(k);
      // A sample keeps what the filer typed, for a rebuild for its period.
      if (s.sample) (s.typedEdits ||= {})[k] = v;
    }
  }

  /** Edit on a profile item: its profile boxes take typing; the edit stays on this return. */
  startProfileEdit(keys, cx) {
    const s = this.state;
    if (!s.editable) return;
    const id = keys.length ? this.itemIdForKey(keys[0]) : null;
    this.endProfileEdit([...s.editing].filter((k) => this.itemIdForKey(k) !== id));
    if (id) {
      const [pi, ii] = id.split(":").map(Number);
      if (s.schema.parts[pi].items[ii].l === "Additional details") s.moreOpen.add(id);
    }
    for (const k of keys) {
      if (!s.profileEdit.has(k)) continue;
      s.editing.add(k);
      s.opened.add(k);
    }
    this.refreshRows();
    cx.notify();
  }

  /** The profile boxes of the item that holds `k`. */
  itemEditKeys(k) {
    const s = this.state;
    const id = this.itemIdForKey(k);
    if (!id) return [k];
    const [pi, ii] = id.split(":").map(Number);
    return (s.schema.parts[pi].items[ii].f || []).map((f) => f.k).filter((key) => s.profileEdit.has(key));
  }

  /**
   * A box took focus. An item still open with Edit elsewhere (a select or
   * radio item has no box to blur) goes back to its profile look.
   */
  enterBox(k) {
    const s = this.state;
    s.focusKey = k;
    const here = this.itemIdForKey(k);
    this.endProfileEdit([...s.editing].filter((key) => this.itemIdForKey(key) !== here));
  }

  /**
   * A box lost focus: once focus has not landed on another box of the same
   * item, the item's profile boxes go back to their profile look.
   */
  leaveBox(k, cx) {
    const s = this.state;
    if (s.focusKey === k) s.focusKey = null;
    if (!this.itemEditKeys(k).some((key) => s.editing.has(key))) return;
    const token = this.token;
    cx.timer.after(50, () => {
      if (token !== this.token) return;
      const now = this.state.focusKey;
      if (now && this.itemIdForKey(now) === this.itemIdForKey(k)) return;
      this.endProfileEdit(this.itemEditKeys(k));
    });
  }

  /** Back to the profile look; the value stays (the draft has it, unsaved). */
  endProfileEdit(keys) {
    const s = this.state;
    let changed = false;
    for (const k of keys) changed = s.editing.delete(k) || changed;
    if (changed) this.refreshRows();
    return changed;
  }

  /**
   * A part's items the profile filled whole (every box it fills has a value
   * and the item has nothing else to type): they fold into one "From tax
   * profile" group. A box the profile left blank stays in paper order.
   */
  tuckedItems(pi) {
    const s = this.state;
    const part = s.schema.parts[pi];
    if (!part || !s.profile.size) return [];
    const out = [];
    part.items.forEach((it, ii) => {
      if (it.rows || it.l === "Additional details") return;
      const typed = (it.f || []).filter((f) => !f.c);
      if (typed.length && typed.every((f) => s.profile.has(f.k))) out.push(ii);
    });
    return out;
  }

  validate(cx) {
    const s = this.state;
    if (!s || !s.draft) return;
    const token = this.token;
    s.checking = true;
    s.validateSeq = (s.validateSeq || 0) + 1;
    const seq = s.validateSeq;
    const draft = s.draft;
    cx.spawn(async (acx) => {
      try {
        const r = await this.app.store.call("screen_form_validate", s.code, draft);
        if (token !== this.token || seq !== this.state.validateSeq) return;
        // Keep a newer local draft if the user typed meanwhile; take values/errors.
        if (this.state.draft === draft) this.state.draft = r.draft;
        if (this.state.sample && Array.isArray(r.blank)) this.state.blank = new Set(r.blank);
        this.state.values = this.shown({ ...this.state.values, ...r.values });
        this.state.options = r.options || {};
        this.state.off = r.off || {};
        if (Array.isArray(r.penalties)) this.state.penalties = r.penalties;
        if (r.pools) this.state.poolTexts = r.pools;
        this.syncComputed();
        this.setErrors(r.errors);
      } catch (e) {
        if (token !== this.token) return;
        this.state.formErrors = [message(e)];
      }
      if (token === this.token && seq === this.state.validateSeq) {
        this.state.checking = false;
      }
      this.notify(acx);
    });
    this.notify(cx);
  }

  /** Item id for a key ("p:i") from the schema. */
  itemIdForKey(k) {
    const s = this.state;
    if (!s.keyIndex) {
      s.keyIndex = new Map();
      s.schema.parts.forEach((p, pi) => p.items.forEach((it, ii) => itemKeys(it).forEach((key) => s.keyIndex.set(key, `${pi}:${ii}`))));
    }
    return s.keyIndex.get(k);
  }

  /**
   * Bind each validation message to an item: by field key, then by the item
   * number in the message ("Item 06" matches item 6), then by label words;
   * one that matches nothing goes on the first item.
   */
  setErrors(errors) {
    const s = this.state;
    // The host holds back what a sample's blank period cannot judge, and
    // the page a quiet date's message until it speaks.
    s.errors = (errors || []).filter((e) => !(s.quiet && e.field === s.quiet.field));
    s.messages = {};
    const items = [];
    s.schema.parts.forEach((p, pi) => p.items.forEach((it, ii) => items.push({ id: `${pi}:${ii}`, it })));
    const words = (t) => String(t).toLowerCase().match(/[a-z]{4,}/g) || [];
    for (const e of s.errors) {
      let id = null;
      for (const k of e.keys || []) {
        id = this.itemIdForKey(k);
        if (id) break;
      }
      if (!id) {
        const m = /\bItem\s+0*(\d+[A-Z]?)\b/i.exec(e.message);
        if (m) {
          const hit = items.find((x) => String(x.it.no || "").replace(/^0+/, "").toUpperCase() === m[1].toUpperCase());
          if (hit) id = hit.id;
        }
      }
      if (!id && /\bTIN\b/.test(e.message)) {
        const hit = items.find((x) => isTinItem(x.it));
        if (hit) id = hit.id;
      }
      if (!id) {
        const w = new Set(words(`${e.field} ${e.message}`));
        let best = 0;
        for (const x of items) {
          const n = words(x.it.l).filter((v) => w.has(v)).length;
          if (n > best) {
            best = n;
            id = x.id;
          }
        }
      }
      id = id || (items[0] && items[0].id);
      if (id) s.messages[id] = s.messages[id] ? `${s.messages[id]} · ${e.message}` : e.message;
    }
  }

  check(cx) {
    const s = this.state;
    this.unquiet();
    this.flush(cx);
    s.announce = true;
    const token = this.token;
    cx.timer.after(50, (acx) => {
      if (token !== this.token) return;
      const wait = () => {
        if (this.state.checking) return acx.timer.after(100, () => wait());
        const n = this.state.errors.length;
        this.app.toast(n ? `${n} item${n > 1 ? "s need" : " needs"} attention.` : "No problems found.", acx, n ? "triangle-alert" : "circle-check");
      };
      wait();
    });
  }

  save(cx) {
    const s = this.state;
    if (!s || !s.draft) return;
    if (s.sample) return this.app.toast(SAMPLE_NOTE, cx);
    this.unquiet();
    // Take whatever is staged before saving, without waiting for the debounce.
    let edits = s.pending;
    s.pending = {};
    try {
      if (Object.keys(edits).length) {
        this.retireRemovalUndo();
        const f = this.app.store.call("screen_form_fill", s.code, s.draft, edits, this.unlockedFor(edits));
        // Rows this would remove are never saved without the question:
        // ask, and save nothing until the filer answers.
        if (f.removes) return this.holdRemoval(f, edits, cx);
        this.accepted(edits, f.rejected);
        s.draft = f.draft;
        s.values = this.shown(f.values);
        if (f.pools) s.poolTexts = f.pools;
        if (Array.isArray(f.penalties)) s.penalties = f.penalties;
        edits = {};
      }
      const r = this.app.store.write("screen_form_save", s.code, s.draft);
      s.draft = r.draft;
      s.status = r.status;
      s.dirty = false;
      s.saved = true;
      this.app.toast("Draft saved.", cx, "save");
      this.validate(cx);
    } catch (e) {
      // Edits the draft never took stay staged for the next try.
      s.pending = { ...edits, ...s.pending };
      this.app.toast(message(e), cx, "triangle-alert");
    }
  }

  /** 1601C and 2551Q keep their own queue path, keyed by month or quarter. */
  usesPeriodPath() {
    return this.state.code === "1601C" || this.state.code === "2551Q";
  }

  /** Save if anything changed; false when the save failed (its toast says why). */
  ensureSaved(cx) {
    const s = this.state;
    if (s.sample) {
      this.app.toast(SAMPLE_NOTE, cx);
      return false;
    }
    if (!s.dirty && s.saved) return true;
    this.save(cx);
    return s.saved && !s.dirty;
  }

  /**
   * "View the filed original": an amended return's filed version, from its
   * history, read-only. Back to the amendment reopens the return.
   */
  openFiledOriginal(cx) {
    const s = this.state;
    if (!s.filedOriginal) return;
    if (s.dirty || Object.keys(s.pending || {}).length) {
      return this.app.toast("Save your edits first: viewing the filed original leaves this return.", cx, "triangle-alert");
    }
    const back = { tin: s.tin, code: s.code, year: s.year, period: s.period, ...(s.returnKey != null ? { returnKey: s.returnKey } : {}) };
    this.open({ tin: s.tin, code: s.code, year: s.year, period: s.period, filedOriginal: s.filedOriginal.id, back }, cx);
    cx.notify();
  }

  backToAmendment(cx) {
    const back = this.state.back;
    if (!back) return;
    this.open(back, cx);
    cx.notify();
  }

  /** Reopen the return as stored (after queueing or cancelling). */
  reload(cx) {
    const s = this.state;
    this.open({ tin: s.tin, code: s.code, year: s.year, period: s.period, sample: s.sample }, cx);
  }

  queue(cx) {
    const s = this.state;
    if (s.sample) return this.app.toast(SAMPLE_NOTE, cx);
    if (!this.ensureSaved(cx)) return;
    try {
      if (this.usesPeriodPath()) this.app.store.write("queue_form_draft", s.tin, s.code, s.year, s.period || null);
      else this.app.store.write("screen_form_queue", s.code, s.draft);
      this.app.toast("Queued for background submission.", cx, "send");
      this.reload(cx);
    } catch (e) {
      this.app.toast(message(e), cx, "triangle-alert");
    }
  }

  cancelQueue(cx) {
    const s = this.state;
    if (s.sample) return this.app.toast(SAMPLE_NOTE, cx);
    try {
      if (this.usesPeriodPath()) this.app.store.write("cancel_queued", s.tin, s.code, s.year, s.period || null);
      else this.app.store.write("screen_form_cancel_queue", s.code, s.draft);
      this.app.toast("Queue cancelled. The return is an editable draft again.", cx);
      this.reload(cx);
    } catch (e) {
      this.app.toast(message(e), cx, "triangle-alert");
    }
  }

  /**
   * Print preview of the return as saved: the official sheet, filled. An
   * amendment not saved yet prints from the page and is not saved: its
   * first save is the filer's own choice, as it reverts the filed return.
   */
  print(cx) {
    const s = this.state;
    if (s.sample) return this.app.toast(SAMPLE_NOTE, cx);
    if (s.amends && !s.saved) this.flush(cx);
    else if (s.editable && !this.ensureSaved(cx)) return;
    try {
      this.app.store.call("screen_form_print", s.code, s.draft);
      s.previewOpen = true;
      this.app.toast("Print preview opened. It follows your edits live.", cx, "printer");
    } catch (e) {
      this.app.toast(message(e), cx, "triangle-alert");
    }
  }
}

// ---------------------------------------------------------------- controls

function controlWidth(f, available) {
  if (f.t === "money") return Math.min(230, available);
  if (f.t === "pct") return 88;
  if (f.t === "date") return 140;
  if (f.t === "int") return f.m && f.m <= 4 ? 88 : 140;
  if (f.t === "sel") return Math.min(260, available);
  if (f.t === "text" && f.m && f.m <= 4) return 88;
  return Math.min(320, available);
}

function lockedBox(cx, value, width, opts = {}) {
  const t = tokens(cx);
  return div()
    .flex()
    .flex_row()
    .items_center()
    .h(SIZE.control)
    .w(width)
    .px(10)
    .rounded(RADIUS.row)
    .border(1)
    .border_color(t.border)
    .bg(t.hover)
    .when(opts.right, (el) => el.justify_end())
    .when(opts.center, (el) => el.justify_center())
    .child(Line(cx, "body", value || "", opts.hint ? "fg3" : "fg2"));
}

function control(cx, ui, f, itemLabel, multi, available) {
  const s = ui.state;
  const t = tokens(cx);
  const width = controlWidth(f, available);
  const showSub = multi && f.l && f.l !== itemLabel && !/^\d+[A-Z]?$/.test(f.l);
  const locked = !s.editable || f.c || ui.isLocked(f.k);
  const value = f.pool ? ui.poolText(f) : s.values[f.k] ?? "";
  let el;
  if (f.t === "sel") {
    el = selectBox(cx, ui, f.k, f.l, f.o, value, locked, width);
  } else if (f.t === "chk") {
    const on = value === "true" || value === "1" || value === "Y";
    const hint = locked && !on ? ui.reasonFor(f.k) : "";
    return div()
      .id(`fp-${f.k}-row`)
      .flex()
      .flex_row()
      .items_center()
      .gap(SPACE[2])
      .when(Boolean(ui.isOff(f.k)), (el) => el.opacity(OFF_OPACITY))
      .child(Checkbox(cx, `fp-${f.k}`, on, (v, cx) => (locked ? null : ui.choose({ [f.k]: v ? "true" : "false" }, cx)), f.l || itemLabel))
      .child(Text(cx, "body", f.l || itemLabel, locked ? "fg2" : "fg").whitespace_normal())
      .when(Boolean(hint), (el) => el.child(Text(cx, "meta", hint, "fg3")));
  } else if (locked) {
    // A computed amount (a penalty too, until Edit penalties) shows 0.00 for zero, never an empty box.
    const shown = f.t === "money" ? formatMoney(value || (ui.isComputedBox(f.k) || ui.penaltyGroupOf(f.k) ? "0" : ""), f.w) : f.t === "date" ? shownDate(value) : value;
    // Locked and blank only with a reason: what it waits for.
    const hint = !shown ? ui.reasonFor(f.k) : "";
    el = lockedBox(cx, shown || hint, width, { right: !hint && (f.t === "money" || f.t === "pct"), hint: Boolean(hint) });
  } else {
    const input = s.inputs.get(f.k);
    if (!input) el = lockedBox(cx, value, width);
    else {
      const box = TextField(cx, input, { id: `fp-${f.k}`, label: f.l, width, error: false }).when(
        f.t === "money" || f.t === "pct",
        (b) => b.text_right(),
      );
      if (f.t === "money" || f.t === "pct") {
        el = div()
          .relative()
          .w(width)
          .child(box.when(f.t === "money", (b) => b.pl(24)).when(f.t === "pct", (b) => b.pr(24)))
          .child(
            div()
              .absolute()
              .top(0)
              .bottom(0)
              .when(f.t === "money", (d) => d.left(10))
              .when(f.t === "pct", (d) => d.right(10))
              .flex()
              .items_center()
              .child(Text(cx, "meta", f.t === "money" ? "₱" : "%", "fg3")),
          );
      } else el = box;
    }
  }
  // Rule 6: a box another answer turns off looks off.
  const wrap = div().flex().flex_col().gap(SPACE[1]).min_w(0).when(Boolean(ui.isOff(f.k)), (el) => el.opacity(OFF_OPACITY));
  if (showSub) wrap.child(Text(cx, "meta", f.l, "fg2").max_w(width).whitespace_normal());
  void t;
  wrap.child(el);
  // Rule 5: the printed boxes, while the text fits them.
  const slots = f.pool ? f.pool.slots : f.slots ? [f.slots] : null;
  const text = f.pool ? ui.poolText(f) : value;
  const cutWords = Boolean(f.pool && f.pool.write.length > 1);
  if (slots && fitsBoxes(text, slots, cutWords)) wrap.child(charBoxes(cx, f.k, splitLines(text, slots, cutWords).map((line) => line.trimEnd()), slots));
  return wrap;
}

/** Rows of character boxes, one character each, as the paper prints them. */
function charBoxes(cx, k, lines, slots) {
  const t = tokens(cx);
  const rows = div().id(`fp-boxes-${k}`).flex().flex_col().gap(2).accessibility_label(`${slots.join(" + ")} character boxes`);
  lines.forEach((line, i) => {
    const row = div().flex().flex_row().flex_wrap();
    for (let c = 0; c < slots[i]; c++) {
      row.child(
        div()
          .flex()
          .items_center()
          .justify_center()
          .w(14)
          .h(18)
          .border_l(c ? 0 : 1)
          .border_r(1)
          .border_y(1)
          .border_color(t.border)
          .child(Text(cx, "cap", line[c] || "", "fg2")),
      );
    }
    rows.child(row);
  });
  return rows;
}

/** BIR's own words for the income-tax ATCs the forms list as bare codes. */
const ATC_TITLES = {
  II011: "Compensation Income",
  II012: "Business Income – Graduated IT Rates",
  II013: "Mixed Income – Graduated IT Rates",
  II014: "Income from Profession – Graduated IT Rates",
  II015: "Business Income – 8% IT Rate",
  II016: "Mixed Income – 8% IT Rate",
  II017: "Income from Profession – 8% IT Rate",
};

function choiceLabel(f) {
  const text = f.l.replace(/^.*?:\s*/, "");
  const code = /^A?TC\s+([A-Z]{2}\d{3})$/.exec(text)?.[1];
  return code ? (ATC_TITLES[code] ? `${code}  ${ATC_TITLES[code]}` : code) : text;
}

/**
 * A one-answer group, drawn as the paper form's tick boxes. Ticking one
 * clears the others; the ticked box of a group of three or more unticks
 * (clears the answer), a yes/no pair keeps one answer (picking it again
 * sends it again).
 */
function radioGroup(cx, ui, it, radios) {
  const s = ui.state;
  const t = tokens(cx);
  const wide = radios.some((f) => choiceLabel(f).length > 24);
  const row = div().flex().gap_x(SPACE[4]).gap_y(SPACE[2]).when(wide, (el) => el.flex_col()).when(!wide, (el) => el.flex_row().flex_wrap());
  // A locked group with nothing ticked says what it waits for.
  const shut = radios.every((f) => !s.editable || f.c || ui.isLocked(f.k)) && !radios.some((f) => s.values[f.k] === "true");
  const hint = shut ? radios.map((f) => ui.reasonFor(f.k)).find(Boolean) || "" : "";
  for (const f of radios) {
    const on = s.values[f.k] === "true";
    const locked = !s.editable || f.c || ui.isLocked(f.k);
    const pick = (cx) => {
      if (locked) return;
      // A pair's ticked answer given again runs again (1602Q item 13 "No"
      // removes the Schedule 2 rows a legacy draft kept, after asking).
      if (on && radios.length >= 3) {
        ui.choose({ [f.k]: "false" }, cx);
        return;
      }
      const edits = {};
      for (const r of radios) edits[r.k] = r.k === f.k ? "true" : "false";
      ui.choose(edits, cx);
    };
    const label = choiceLabel(f);
    const box = div()
      .flex()
      .flex_none()
      .items_center()
      .justify_center()
      .size(16)
      .rounded(4)
      .border(1)
      .border_color(on ? t.primary : t.borderStrong)
      .bg(on ? t.primary : t.surface);
    if (on) box.child(Icon(cx, "check", { size: 12, color: "primaryFg" }));
    const item = div()
      .id(`fp-${f.k}`)
      .flex()
      .flex_row()
      .items_center()
      .gap(SPACE[2])
      .accessibility_label(label)
      .child(box)
      .child(Text(cx, "body", label, locked ? "fg2" : "fg").whitespace_normal());
    if (!locked) item.cursor_pointer().on_click((_e, cx) => pick(cx));
    else item.opacity(ui.isOff(f.k) ? OFF_OPACITY : 0.7);
    row.child(item);
  }
  if (hint) row.child(Text(cx, "meta", hint, "fg3"));
  void it;
  return row;
}

function tinBoxes(cx, ui, it) {
  const s = ui.state;
  const row = div().flex().flex_row().flex_wrap().items_center().gap(6);
  (it.f || []).forEach((f, j) => {
    if (j) row.child(Text(cx, "body", "-", "fg3"));
    const width = j === 3 ? 76 : 58;
    // A TIN the filer types (1700's spouse) is editable; the profile's is locked.
    const locked = !s.editable || f.c || ui.isLocked(f.k);
    const input = s.inputs.get(f.k);
    if (locked || !input) row.child(lockedBox(cx, s.values[f.k] ?? "", width, { center: true }));
    else row.child(TextField(cx, input, { id: `fp-${f.k}`, label: f.l, width, error: false }));
  });
  return row;
}

function scheduleTable(cx, ui, it, width) {
  const s = ui.state;
  const t = tokens(cx);
  const colW = 140;
  const numW = 36;
  // A column an override hides (a row template another table now holds).
  const shown = it.cols.map((c, i) => (c.t === "hidden" ? -1 : i)).filter((i) => i >= 0);
  // The sheet's preprinted row captions (1603Q Schedule I's recipients).
  const labels = it.row_labels || null;
  const labelW = labels ? 220 : 0;
  const tableW = numW + labelW + shown.length * colW;
  const head = div().flex().flex_row().items_end().bg(t.bg).border_b(1).border_color(t.border);
  head.child(div().w(numW).flex_none().py(SPACE[2]).flex().justify_center().child(Text(cx, "meta", "#", "fg3")));
  if (labels) head.child(div().w(labelW).flex_none().py(SPACE[2]).px(SPACE[2]).child(Text(cx, "metaStrong", labels.caption, "fg3").whitespace_normal()));
  for (const i of shown) head.child(div().w(colW).flex_none().py(SPACE[2]).px(SPACE[2]).child(Text(cx, "metaStrong", it.cols[i].l, "fg3").whitespace_normal()));
  const body = div().flex().flex_col();
  it.keys.forEach((row, r) => {
    const no = (it.start ?? 1) + r;
    const tr = div().flex().flex_row().items_center().when(r > 0, (el) => el.border_t(1).border_color(t.border));
    tr.child(div().w(numW).flex_none().flex().justify_center().child(Text(cx, "meta", String(no), "fg3")));
    if (labels) tr.child(div().w(labelW).flex_none().py(6).px(SPACE[2]).child(Text(cx, "body", labels.rows[r] || "", "fg2").whitespace_normal()));
    row.forEach((k, c) => {
      const col = it.cols[c];
      if (col.t === "hidden") return;
      const locked = !s.editable || col.c || ui.isLocked(k);
      const value = s.values[k] ?? "";
      let cell;
      if (col.t === "radio" || col.t === "chk") {
        const on = value === "true";
        const hint = locked && !on ? ui.reasonFor(k) : "";
        cell = Checkbox(cx, `fp-${k}`, on, (v, cx) => (locked ? null : ui.choose({ [k]: v ? "true" : "false" }, cx)), `${col.l}, row ${no}`);
        if (hint) cell = div().flex().flex_row().items_center().gap(SPACE[1]).child(cell).child(Text(cx, "meta", hint, "fg3"));
      } else if (col.t === "sel") {
        cell = selectBox(cx, ui, k, `${col.l}, row ${no}`, col.o, value, locked, colW - 16);
      } else if (locked || !s.inputs.get(k)) {
        const shown = col.t === "money" ? formatMoney(value || (ui.isComputedBox(k) ? "0" : ""), col.w) : col.t === "date" ? shownDate(value) : value;
        const hint = !shown ? ui.reasonFor(k) : "";
        cell = lockedBox(cx, shown || hint, colW - 16, { right: !hint && col.t === "money", hint: Boolean(hint) });
      } else {
        cell = TextField(cx, s.inputs.get(k), { id: `fp-${k}`, label: `${col.l}, row ${no}`, width: colW - 16 }).h(30);
      }
      tr.child(div().w(colW).flex_none().py(6).px(SPACE[2]).when(Boolean(ui.isOff(k)), (el) => el.opacity(OFF_OPACITY)).child(cell));
    });
    body.child(tr);
  });
  // The one sideways scroller in the app: a schedule wider than its box.
  return div()
    .id(`fp-table-${it.keys[0][0]}`)
    .w(width)
    .rounded(RADIUS.row)
    .border(1)
    .border_color(t.border)
    .overflow_x_scroll()
    .overflow_y_hidden()
    .child(div().flex().flex_col().w(tableW).child(head).child(body));
}

/**
 * The month, day and year boxes of an item, when it has them: a printed date
 * split into separate boxes. Needs at least a month and a year.
 */
function dateParts(fs) {
  const find = (re, not) =>
    fs.find((f) => f.t !== "radio" && f.t !== "chk" && re.test(`${f.l} ${String(f.k).split(":").pop()}`) && !(not && not.test(`${f.l} ${f.k}`)));
  const month = find(/month|\bmm\b/i, /months|no\.? of/i);
  const year = find(/year|yyyy|\byr\b/i, /month|years ended|no\.? of/i);
  const day = find(/\bday\b|\bdd\b|[a-z]day\b|day\d*$/i, /days|month|year/i);
  if (!month || !year || month === year) return null;
  return { month, day: day && day !== month && day !== year ? day : null, year };
}

/**
 * An item the host marks `one_date` (1701Q item 11): its month, day and year
 * boxes are one date on the page, typed or picked as "May 17, 1980", while
 * the draft keeps the three boxes. The page's box for it is keyed
 * `<month key>#date`; it is never sent to the host.
 */
function oneDate(it) {
  if (!it.one_date) return null;
  const date = dateParts(it.f || []);
  return date ? { ...date, oneKey: `${date.month.k}#date` } : null;
}

/** The three boxes' values as the MM/DD/YYYY text a date box reads ("" when all are blank). */
export function oneDateText(values, date) {
  const parts = [date.month, date.day, date.year].map((f) => String((f && values[f.k]) ?? "").trim());
  return parts.every((v) => !v) ? "" : parts.join("/");
}

const WEEKDAYS = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];
const MONTH_NAMES = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/**
 * Move the calendar's grid to `year` and `month` (1-12) in one jump: select
 * the same day there (the last one when the month is shorter), as the rules
 * page's calendar does. The box is filled only when a day is picked;
 * `picker.jumpedTo` lets the calendar's change event tell this jump from a
 * pick. Steps the rest one month at a time if the grid did not follow.
 */
export function moveCalendar(cal, year, month, picker = null) {
  const selected = typeof cal.value() === "string" ? cal.value() : "";
  const days = new Date(Date.UTC(year, month, 0)).getUTCDate();
  const day = Math.min(Number(selected.slice(8, 10)) || 1, days);
  const target = `${year}-${String(month).padStart(2, "0")}-${String(day).padStart(2, "0")}`;
  if (picker) picker.jumpedTo = target;
  cal.set_value(target);
  let steps = (year - cal.year()) * 12 + (month - cal.month());
  for (; steps > 0; steps--) cal.next_month();
  for (; steps < 0; steps++) cal.prev_month();
}

/**
 * The calendar's month or year list (rule 4: the picker changes the month
 * and the year, not just the day). Years run from 1900, for dates of birth,
 * to ten years ahead.
 */
function calendarSelect(cx, ui, id, cal, which) {
  const s = ui.state;
  const month = which === "month";
  const last = Number(String(cal.today()).slice(0, 4)) + 10;
  const options = month
    ? MONTH_NAMES.map((name, i) => ({ value: String(i + 1), label: name }))
    : Array.from({ length: last - 1899 }, (_x, i) => String(last - i)).map((y) => ({ value: y, label: y }));
  return Dropdown(cx, `fp-date-${which}-${id}`, {
    label: month ? "Month" : "Year",
    width: month ? 128 : 88,
    options,
    value: String(month ? cal.month() : cal.year()),
    open: s.picker?.select === which,
    onOpenChange: (open, cx) => {
      if (s.picker) s.picker.select = open ? which : null;
      ui.refreshRows();
      cx.notify();
    },
    onChange: (v, cx) => {
      if (month) moveCalendar(cal, cal.year(), Number(v), s.picker);
      else moveCalendar(cal, Number(v), cal.month(), s.picker);
      if (s.picker) s.picker.select = null;
      ui.refreshRows();
      cx.notify();
    },
  });
}

/** A calendar button beside an item's date boxes; picking a day fills them. Typing still works. */
function datePicker(cx, ui, id, date, row = id) {
  const s = ui.state;
  const t = tokens(cx);
  const open = s.picker && s.picker.id === id;
  const button = IconButton(cx, `fp-date-${id}`, "calendar", {
    tooltip: "Pick a date",
    pressed: open,
    onClick: (_e, cx) => {
      if (open) {
        s.picker = null;
        ui.refreshRows();
        return cx.notify();
      }
      const cal = CalendarState.new();
      const typed = date.single ? parseTypedDate(s.values[date.single.k]) : null;
      const m = typed ? typed.m : date.single ? 0 : Number(String(s.values[date.month.k] || "").slice(0, 2));
      const y = typed ? typed.y : date.single ? 0 : Number(s.values[date.year.k] || "");
      const d = typed ? typed.d : date.day ? Number(s.values[date.day.k] || "") : 1;
      if (m >= 1 && m <= 12 && y > 1900) {
        cal.set_value(`${y}-${String(m).padStart(2, "0")}-${String(d >= 1 && d <= 31 ? d : 1).padStart(2, "0")}`);
      }
      cal.on("change", (value, cx) => {
        // A month or year jump selects a day too; only a pick fills the box.
        if (typeof value === "string" && s.picker && value !== s.picker.jumpedTo) ui.pickDate(date, value, cx);
      });
      s.picker = { id, row, cal };
      ui.refreshRows();
      cx.notify();
    },
  });
  const popup = Popup.new(`fp-date-popup-${id}`, button).anchor("top_left");
  if (!open) return popup;
  const cal = s.picker.cal;
  const grid = cal.month_days()[0] || [];
  const selected = cal.value();
  const card = div()
    .id(`fp-date-card-${id}`)
    .flex()
    .flex_col()
    .gap(SPACE[1])
    .mt(SPACE[1])
    .p(SPACE[3])
    .rounded(RADIUS.card)
    .border(1)
    .border_color(t.border)
    .bg(t.raised)
    .shadow_lg()
    .on_mouse_down_out((_e, cx) => {
      // A click away closes the calendar. While a month or year list is
      // open over the card, wait for the list: a click in it is the card's
      // own, and one outside it closes the list and the calendar.
      // The deferred check uses the timer's own context: this handler's
      // is gone by then.
      const close = (c) => {
        s.picker = null;
        ui.refreshRows();
        c.notify();
      };
      if (!s.picker?.select) return close(cx);
      cx.timer.after(0, (acx) => {
        if (s.picker && !s.picker.select) close(acx);
      });
    })
    .child(
      div()
        .flex()
        .flex_row()
        .items_center()
        .gap(SPACE[1])
        .mb(SPACE[1])
        .child(IconButton(cx, `fp-date-prev-${id}`, "chevron-left", { tooltip: "Previous month", onClick: (_e, cx) => { cal.prev_month(); ui.refreshRows(); cx.notify(); } }))
        .child(calendarSelect(cx, ui, id, cal, "month"))
        .child(calendarSelect(cx, ui, id, cal, "year"))
        .child(IconButton(cx, `fp-date-next-${id}`, "chevron-right", { tooltip: "Next month", onClick: (_e, cx) => { cal.next_month(); ui.refreshRows(); cx.notify(); } })),
    )
    .child(div().flex().flex_row().children(WEEKDAYS.map((w) => div().w(32).flex().justify_center().child(Text(cx, "cap", w, "fg3")))));
  for (const week of grid) {
    const row = div().flex().flex_row();
    for (const day of week) {
      const inMonth = Number(day.slice(5, 7)) === cal.month();
      const on = day === selected;
      row.child(
        div()
          .id(`fp-date-${id}-${day}`)
          .flex()
          .items_center()
          .justify_center()
          .size(32)
          .rounded(RADIUS.ctl)
          .cursor_pointer()
          .when(on, (el) => el.bg(t.primary))
          .when(!on, (el) => el.hover((st) => st.bg(t.hover)))
          .on_click((_e, cx) => {
            // gpui's set_value emits no "change" (CalendarState::set_date only
            // notifies); marked anyway so a pick can never run twice.
            if (s.picker) s.picker.jumpedTo = day;
            cal.set_value(day);
            ui.pickDate(date, day, cx);
          })
          .child(Text(cx, "body", String(Number(day.slice(8))), on ? "primaryFg" : inMonth ? "fg" : "fg3")),
      );
    }
    card.child(row);
  }
  return popup.content(card);
}

class FormCheckButton extends View {
  init(props) { this.props = props; }
  update(props) { this.props = props; }
  render(cx) {
    const { ui, state: s } = this.props;
    if (s !== ui.state) return div();
    const leading = div().size(16).flex_none().flex().items_center().justify_center()
      .child(s.checking ? Spinner.new().size("xsmall").color(tokens(cx).fg) : Icon(cx, "circle-check"));
    return AppButton(cx, "fp-check", "Check", {
      size: "sm", leading, ariaLabel: s.checking ? "Check — validation in progress" : "Check",
      onClick: (_e, cx) => ui.check(cx),
    });
  }
}

class FormItemView extends View {
  init(props) { this.props = props; }
  update(props) { this.props = props; }
  render(cx) {
    const { ui, state, it, pi, ii, width } = this.props;
    if (state !== ui.state) return div();
    const row = itemRow(cx, ui, it, pi, ii, width);
    if (ii > 0) row.border_t(1).border_color(tokens(cx).border);
    return row;
  }
}

function itemRow(cx, ui, it, pi, ii, width) {
  const s = ui.state;
  const t = tokens(cx);
  const id = `${pi}:${ii}`;
  const fs = it.f || [];
  const inner = width - 44 - 12 - 20 - 12;
  const allCalc = !it.rows && fs.length > 0 && fs.every((x) => x.c);
  const req = fs.some((x) => x.r);
  const tin = isTinItem(it);
  // The host says which boxes the profile fills (the schema's auto flag is
  // only the extractor's guess). A TIN the host marks `typed_tin` (1801's
  // executor) is the filer's to type, not the profile's.
  const fromProfile = (tin && !it.typed_tin) || fs.some((x) => s.profile.has(x.k));
  // Profile boxes keep the profile mark, which turns into Edit while the
  // pointer is over the item or the keyboard is on the button (Tab, then
  // Enter): the edit stays on this return, never in the profile (rule 2).
  const editKeys = fs.filter((x) => s.profileEdit.has(x.k)).map((x) => x.k);
  const editingHere = editKeys.some((k) => s.editing.has(k));
  const hasEdit = editKeys.length > 0 && s.editable;
  // Markers are 16 px icons that explain themselves on hover, so a long
  // printed label is never covered; the label gets a pixel width.
  const markers = [];
  if (fromProfile) {
    const tip = editKeys.length
      ? "From tax profile. Edit changes this return only; the profile stays as it is."
      : tin
        ? "From tax profile. Change it in the tax profile."
        : "From tax profile. It repeats a box above; use Edit there.";
    markers.push(["user", tip]);
  }
  if (allCalc) markers.push(["lock", "Computed from other items"]);
  // Rule 8: the penalties show what is computed; one Edit opens all three.
  const penalty = s.editable ? fs.map((f) => ui.penaltyGroupOf(f.k)).find((g) => g && g.applies) : null;
  const penaltyOpen = Boolean(penalty) && (penalty.edited || penalty.keys.some((k) => s.penaltyOpen.has(k)));
  const markerW = markers.length * 20 + (it.rows ? 64 : 0) + (hasEdit ? 56 : 0) + (penalty ? (penaltyOpen ? 210 : 130) : 0);
  const label = div()
    .flex()
    .flex_row()
    .items_start()
    .gap(SPACE[2])
    .mb(10)
    .min_h(22)
    .child(
      div()
        .w(Math.max(120, inner - markerW - 8))
        .child(Text(cx, "control", req ? `${it.l} *` : it.l).whitespace_normal()),
    );
  if (it.rows) label.child(Text(cx, "meta", `${it.rows} rows`, "fg3").flex_none().pt(2));
  for (const [icon, tip] of markers) {
    label.child(div().id(`fp-mark-${icon}-${id}`).flex_none().pt(3).tooltip(tip).accessibility_label(tip).child(Icon(cx, icon, { size: 14, color: "fg3" })));
  }
  if (penalty) {
    label.child(
      penaltyOpen
        ? AppButton(cx, `fp-penalty-computed-${id}`, "Use the computed amounts", { size: "sm", variant: "ghost", icon: "history", onClick: (_e, cx) => ui.usePenaltyComputed(penalty, cx) })
        : AppButton(cx, `fp-penalty-edit-${id}`, "Edit penalties", { size: "sm", variant: "ghost", icon: "pencil", onClick: (_e, cx) => ui.openPenalties(penalty, cx) }),
    );
  }
  if (hasEdit) {
    // The button stays in place (and keeps the keyboard) while the boxes
    // are open, only hidden, so Tab from it goes on into the box.
    const shown = !editingHere && s.hoverItem === id;
    const edit = div()
      .id(`fp-edit-${id}`)
      .flex()
      .flex_none()
      .flex_row()
      .items_center()
      .gap(4)
      .h(22)
      .px(SPACE[2])
      .rounded(RADIUS.ctl)
      .opacity(shown ? 1 : 0)
      .border(1)
      .border_color("#00000000")
      .focus((st) => st.opacity(1).border_color(t.primary))
      .accessibility_label(editingHere ? `${it.l}: editing on this return` : `Edit ${it.l} on this return`)
      .child(Icon(cx, "pencil", { size: 12, color: "fg2" }))
      .child(Text(cx, "meta", "Edit", "fg2"));
    if (!editingHere) {
      edit
        .cursor_pointer()
        .hover((st) => st.bg(t.hover))
        .tooltip("Change it on this return only")
        .on_click((_e, cx) => ui.startProfileEdit(editKeys, cx));
    }
    const focus = s.editFocus.get(id);
    if (focus) {
      edit.track_focus(focus).tab_index(0).on_key_down((e, cx) => {
        if (editingHere || (e.key !== "enter" && e.key !== "space")) return;
        ui.startProfileEdit(editKeys, cx);
        cx.stop_propagation();
      });
    }
    label.child(edit);
  }

  // Rule 6: what turns the item's faded boxes on.
  const needs = [...new Set(itemKeys(it).map((k) => ui.isOff(k)).filter(Boolean))].map((why, n) =>
    div().id(`fp-needs-${id}-${n}`).mb(6).tooltip(why).child(Text(cx, "meta", needsText(why), "fg3").whitespace_normal()),
  );

  let controls;
  if (it.note) {
    controls = Text(cx, "meta", it.note, "fg3").whitespace_normal();
  } else if (it.rows) {
    controls = scheduleTable(cx, ui, it, inner);
  } else if (it.l === "Additional details") {
    const open = s.moreOpen.has(id);
    controls = div()
      .flex()
      .flex_col()
      .child(
        div()
          .id(`fp-more-${id}`)
          .flex()
          .flex_row()
          .items_center()
          .gap(6)
          .h(SIZE.controlSm)
          .cursor_pointer()
          .on_click((_e, cx) => {
            if (open) s.moreOpen.delete(id);
            else s.moreOpen.add(id);
            cx.notify();
          })
          .child(Icon(cx, open ? "chevron-down" : "chevron-right", { color: "fg3" }))
          .child(Text(cx, "metaStrong", `${fs.length} more field${fs.length > 1 ? "s" : ""} not yet placed on the paper layout`, "fg2")),
      );
    if (open) {
      const wrap = div().flex().flex_row().flex_wrap().gap_x(SPACE[4]).gap_y(SPACE[3]).items_end().mt(10);
      for (const f of fs) wrap.child(control(cx, ui, f, f.l, true, inner));
      controls.child(wrap);
    }
  } else if (oneDate(it)) {
    const date = oneDate(it);
    const parts = [date.month, date.day, date.year];
    const locked = !s.editable || parts.every((f) => f.c || ui.isLocked(f.k));
    const input = s.inputs.get(date.oneKey);
    const box =
      locked || !input
        ? lockedBox(cx, shownDate(oneDateText(s.values, date)), controlWidth({ t: "date" }, inner))
        : TextField(cx, input, { id: `fp-${date.oneKey}`, label: it.l, width: controlWidth({ t: "date" }, inner), error: false });
    controls = div().flex().flex_row().items_end().gap(SPACE[1]).child(box);
    if (!locked) controls.child(datePicker(cx, ui, id, date));
  } else if (tin) {
    controls = tinBoxes(cx, ui, it);
  } else {
    controls = div().flex().flex_row().flex_wrap().gap_x(SPACE[4]).gap_y(SPACE[3]).items_end();
    const radios = fs.filter((f) => f.t === "radio");
    if (radios.length) controls.child(radioGroup(cx, ui, it, radios));
    // Each whole date box has its own calendar beside it (2200A's four
    // payment dates, 1702EX's Effectivity From and To).
    const single = fs.filter((f) => isDateField(f));
    for (const f of fs) {
      if (f.t === "radio") continue;
      const box = control(cx, ui, f, it.l, fs.length > 1, inner);
      if (!isDateField(f) || !s.editable || f.c || ui.isLocked(f.k)) {
        controls.child(box);
        continue;
      }
      const pickerId = single.length === 1 ? id : `${id}-${single.indexOf(f)}`;
      controls.child(div().flex().flex_row().items_end().gap(SPACE[1]).child(box).child(datePicker(cx, ui, pickerId, { single: f }, id)));
    }
    const date = single.length ? null : dateParts(fs);
    if (date && s.editable && ![date.month, date.day, date.year].filter(Boolean).every((f) => f.c || ui.isLocked(f.k))) {
      controls.child(datePicker(cx, ui, id, date));
    }
  }

  const msg = s.dateErrors?.[id] || s.messages[id] || "";
  return div()
    .id(`fp-item-${id}`)
    .flex()
    .flex_row()
    .gap(SPACE[3])
    .pt(14)
    .pr(SPACE[5])
    .pb(SPACE[2])
    .pl(SPACE[3])
    .when(allCalc, (el) => el.bg(t.bg))
    // The pointer over a profile item turns its mark into Edit.
    .when(hasEdit, (el) =>
      el.on_hover((hovered, cx) => {
        const next = hovered ? id : s.hoverItem === id ? null : s.hoverItem;
        if (next === s.hoverItem) return;
        s.hoverItem = next;
        ui.refreshRows();
        cx.notify();
      }),
    )
    // Clicking away from an item opened with Edit gives it the profile look
    // back; what was typed stays on this return. A list still open is the
    // item's own and does not count as away.
    .when(editingHere, (el) =>
      el.on_mouse_down_out((_e, cx) => {
        if (editKeys.includes(s.openSelect)) return;
        if (ui.endProfileEdit(editKeys)) ui.flush(cx);
      }),
    )
    .child(
      div()
        .w(44)
        .flex_none()
        .child(
          it.no
            ? div().flex().items_center().justify_center().h(22).min_w(30).px(6).rounded(RADIUS.ctl).bg(t.hover).child(Code(cx, it.no, "fg2"))
            : div(),
        ),
    )
    .child(
      div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w(0)
        .child(label)
        .children(needs)
        .child(controls)
        // A shut box that shows blank says why, in the filer's words.
        .children(ui.lockNotesFor(it).map((note) => Text(cx, "meta", note, "fg3").mt(SPACE[1]).whitespace_normal()))
        // Reserved 18 px validation line, present with or without a message.
        .child(div().h(SIZE.validationLine).mt(SPACE[1]).overflow_hidden().child(Line(cx, "meta", msg, "danger").tooltip(msg || " "))),
    );
}

/**
 * The fold for a part's profile-filled items: "From tax profile · N filled
 * boxes", collapsed so the filer sees what is left to type. A message on a
 * folded item is counted on the fold.
 */
function profileGroup(cx, ui, pi, tucked, border) {
  const s = ui.state;
  const t = tokens(cx);
  const open = s.profileOpen.has(pi);
  const items = tucked.map((ii) => s.schema.parts[pi].items[ii]);
  // A ticked radio item is one box, however many radios it has.
  const boxes = (it) => {
    const fs = (it.f || []).filter((f) => s.profile.has(f.k));
    return fs.filter((f) => f.t !== "radio").length + (fs.some((f) => f.t === "radio") ? 1 : 0);
  };
  const filled = items.reduce((n, it) => n + boxes(it), 0);
  const flagged = tucked.filter((ii) => s.messages[`${pi}:${ii}`] || s.dateErrors?.[`${pi}:${ii}`]).length;
  const label = `From tax profile · ${filled} filled box${filled === 1 ? "" : "es"}`;
  const toggle = (cx) => {
    if (s.profileOpen.has(pi)) s.profileOpen.delete(pi);
    else s.profileOpen.add(pi);
    ui.refreshRows();
    cx.notify();
  };
  const header = div()
    .id(`fp-profile-${pi}`)
    .flex()
    .flex_row()
    .items_center()
    .gap(SPACE[2])
    .min_h(40)
    .pl(SPACE[3])
    .pr(SPACE[5])
    .cursor_pointer()
    .when(border, (el) => el.border_t(1).border_color(t.border))
    .bg(t.bg)
    .hover((st) => st.bg(t.hover))
    .focus((st) => st.bg(t.selected))
    .accessibility_label(open ? `${label}, shown` : `${label}, folded`)
    .on_click((_e, cx) => toggle(cx))
    .child(Icon(cx, open ? "chevron-down" : "chevron-right", { color: "fg3" }))
    .child(Icon(cx, "user", { size: 14, color: "fg3" }))
    .child(Text(cx, "metaStrong", label, "fg2"))
    .when(flagged > 0, (el) => el.child(Text(cx, "meta", `· ${flagged} need${flagged === 1 ? "s" : ""} attention`, "danger")));
  const focus = s.editFocus.get(`fold-${pi}`);
  if (focus) {
    header.track_focus(focus).tab_index(0).on_key_down((e, cx) => {
      if (e.key !== "enter" && e.key !== "space") return;
      toggle(cx);
      cx.stop_propagation();
    });
  }
  return header;
}

// ---------------------------------------------------------------- existing return

/** The strip on an amended return's filed version, opened from its history. */
export function filedOriginalStrip(view) {
  const when = view.filed_on ? ` on ${shortDate(view.filed_on)}` : "";
  return `This is the version filed${when}, kept in this return's history; the current return is its amendment. Nothing here can be changed.`;
}

/** Where an amended return's filed version goes (the history view). */
export const FILED_KEPT = "The filed version is kept in this return's history.";

/** `2026-10-01T…` → `Oct 1, 2026`; anything else as given. */
function shortDate(text) {
  const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(String(text || ""));
  if (!m) return String(text || "");
  const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
  return `${months[Number(m[2]) - 1]} ${Number(m[3])}, ${m[1]}`;
}

/**
 * What the offer says and offers for a stored return's status: a draft is
 * loaded, a queued return viewed, one that left the app viewed read-only
 * or amended (only where the host offers `amend`).
 */
export function existingOfferFor(existing) {
  const label = existing.label;
  const has = (action) => (existing.actions || []).includes(action);
  const keep = { id: "keep", label: "Keep this form", hint: "Stay in the sample; the period clears." };
  if (existing.status === "Draft") {
    return {
      title: `You already have a ${label} draft`,
      subtitle: existing.updated_at ? `edited ${shortDate(existing.updated_at)}.` : "",
      choices: [{ id: "load", label: "Load saved draft", hint: "Continue where you left off.", variant: "primary" }, keep],
      fastLane: true,
    };
  }
  if (existing.status === "Queued") {
    return {
      title: `${label} is queued to send to BIR`,
      subtitle: "",
      choices: [{ id: "view", label: "View it", hint: "Read-only while it waits to send.", variant: "primary" }, keep],
      fastLane: false,
    };
  }
  const filed = existing.status === "Submitted" ? "submitted" : "filed";
  const choices = [{ id: "view", label: "View read-only", hint: `See exactly what was ${filed}.`, variant: "primary" }];
  if (has("amend")) choices.push({ id: "amend", label: "Amend this return", hint: `New amended return. ${FILED_KEPT}`, variant: "danger" });
  choices.push(keep);
  return { title: `${label} was ${filed}`, subtitle: "", choices, fastLane: false };
}

/** The app's modal for a stored return of a sample's period (Esc or outside: Keep this form). */
export function renderExistingOffer(cx, app) {
  const ui = app.formUi;
  const existing = app.existingOffer;
  if (!existing) return div();
  const offer = existingOfferFor(existing);
  // One left edge and equal padding on every side: the title, the
  // full-width choices (each with what it does inside it) and the checkbox.
  const card = div().flex().flex_col().items_stretch().w_full().gap(SPACE[3]).p(SPACE[5]);
  const head = div().flex().flex_col().gap(2).child(Text(cx, "title", offer.title).whitespace_normal());
  if (offer.subtitle) head.child(Text(cx, "meta", offer.subtitle, "fg2"));
  card.child(head);
  for (const choice of offer.choices) {
    card.child(
      AppButton(cx, `existing-${choice.id}`, choice.label, {
        variant: choice.variant || "secondary",
        block: true,
        hint: choice.hint,
        onClick: (_e, cx) => ui.takeExisting(choice.id, cx),
      }),
    );
  }
  if (offer.fastLane) {
    card.child(
      CheckboxLabel(
        cx,
        "existing-fast-lane",
        Boolean(app.fastLane?.drafts),
        (v, cx) => {
          app.fastLane = { ...(app.fastLane || {}), drafts: v };
          cx.notify();
        },
        "Don't ask again for drafts (this session)",
      ),
    );
  }
  return Overlay(cx, "existing-offer", card, (cx) => ui.takeExisting("keep", cx), { width: 420 });
}

/** The question an answer that removes typed rows asks first. */
export function renderRemovalConfirm(cx, app) {
  const ui = app.formUi;
  const held = ui.state?.removal;
  if (!held) return div();
  const card = div()
    .flex()
    .flex_col()
    .items_stretch()
    .w_full()
    .gap(SPACE[3])
    .p(SPACE[5])
    .child(Text(cx, "title", held.r.removes.message).whitespace_normal())
    .child(AppButton(cx, "removal-continue", "Continue", { variant: "primary", block: true, onClick: (_e, cx) => ui.confirmRemoval(cx) }))
    .child(AppButton(cx, "removal-cancel", "Cancel", { variant: "secondary", block: true, onClick: (_e, cx) => ui.cancelRemoval(cx) }));
  return Overlay(cx, "removal-confirm", card, (cx) => ui.cancelRemoval(cx), { width: 420 });
}

// ---------------------------------------------------------------- page

/**
 * The form page's pieces: the header (title, actions, taxpayer bar), the part
 * navigator and the parts. `renderFormPage` stacks them in the main scroll
 * area; `renderFormSplit` pins the navigator beside a form that scrolls on
 * its own.
 */
function formContent(cx, app, width) {
  const ui = app.formUi;
  const s = ui.state;
  const t = tokens(cx);
  if (!s) return { early: div() };
  const f = s.schema;
  if (!f) {
    return { early: PageHeader(cx, { title: `Form ${s.code}`, lede: s.error || "This form has no on-screen layout." }) };
  }
  // The return's own TIN and branch decide whose it is; never a name match
  // or whichever taxpayer happens to be selected.
  const branch = app.branchFor(s.tin);
  const tp = branch ? app.taxpayers().find((x) => x.tin_root === branch.tin_root) : null;
  const branchCode = String(s.tin).slice(9) || "00000";
  const draftLayout = f.q < 0.8;

  // A library sample has only Check: Print, Queue and Save need a return.
  const actions = s.sample ? [] : [AppButton(cx, "fp-print", "Print preview", { size: "sm", icon: "printer", onClick: (_e, cx) => ui.print(cx) })];
  if (s.filedOriginal && !s.filedOriginalView) {
    actions.push(AppButton(cx, "fp-filed-original", "View the filed original", { size: "sm", icon: "history", onClick: (_e, cx) => ui.openFiledOriginal(cx) }));
  }
  if (s.filedOriginalView) {
    actions.push(AppButton(cx, "fp-back-to-amendment", "Back to the amendment", { size: "sm", onClick: (_e, cx) => ui.backToAmendment(cx) }));
  } else if (s.sample) {
    if (s.editable) actions.push(s.checkButton.entity);
    actions.push(AppButton(cx, "fp-start-return", "Start a return", { variant: "primary", size: "sm", icon: "plus", onClick: (_e, cx) => ui.startReturn(cx) }));
  } else if (s.status === "Queued") {
    actions.push(AppButton(cx, "fp-cancel-queue", "Cancel queue", { size: "sm", onClick: (_e, cx) => ui.cancelQueue(cx) }));
  } else if (s.editable) {
    actions.push(s.checkButton.entity);
    actions.push(AppButton(cx, "fp-queue", "Queue", { size: "sm", icon: "send", disabled: s.loading, onClick: (_e, cx) => ui.queue(cx) }));
    actions.push(AppButton(cx, "fp-save", "Save draft", { variant: "primary", size: "sm", disabled: s.loading, onClick: (_e, cx) => ui.save(cx) }));
  }

  const chips = div().flex().flex_row().flex_wrap().items_center().gap(SPACE[2]).mt(SPACE[2]);
  chips.child(Chip(cx, f.freq)).child(Chip(cx, `${f.pages} page${f.pages > 1 ? "s" : ""}`)).child(Chip(cx, `${f.n} fields`));
  if (draftLayout) chips.child(Chip(cx, "Draft layout", "warning", "triangle-alert"));
  if (s.status !== "Draft") chips.child(Chip(cx, s.editable ? s.status : `${s.status} · read-only`, s.status === "Queued" ? "accent" : "success"));
  if (s.amends || s.filedOriginal) chips.child(Chip(cx, "Amended return", "warning"));
  if (s.filedOriginalView) chips.child(Chip(cx, "Filed original · read-only", "success"));

  const head = div()
    .flex()
    .flex_row()
    .flex_wrap()
    .items_start()
    .gap(14)
    .child(
      div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w(Math.min(width, 420))
        .child(MetaLine(cx, [Code(cx, `BIR Form ${f.c}`), `Revision ${f.rev}`]))
        .child(Text(cx, "page", f.t).mt(SPACE[1]).whitespace_normal())
        .child(chips),
    )
    .child(
      div()
        .flex()
        .flex_row()
        .flex_wrap()
        .gap(SPACE[2])
        .children(actions),
    );

  const bar = div()
    .flex()
    .flex_row()
    .flex_wrap()
    .items_center()
    .gap_x(SPACE[3])
    .gap_y(SPACE[2])
    .my(SPACE[5])
    .py(10)
    .px(SPACE[3])
    .rounded(RADIUS.card)
    .border(1)
    .border_color(t.border)
    .bg(t.bg);
  if (tp) {
    bar
      .child(Avatar(cx, tp.name, 20, tp.tin_root))
      .child(Text(cx, "control", tp.name).font_weight(600))
      .child(Code(cx, formatTin(tp.tin_root, branchCode), "fg2").font_weight(400))
      .child(Text(cx, "meta", branch ? branch.name : "", "fg2"));
  } else {
    bar.child(Code(cx, formatTin(String(s.tin).slice(0, 9), branchCode), "fg2").font_weight(400));
  }
  bar.child(div().size(3).rounded(RADIUS.full).bg(t.fg3));
  bar.child(Text(cx, "meta", s.sample ? "Sample from the Form library" : `For ${periodLabel(f.freq, s.year, s.period)}`, "fg2"));
  if (s.loading) bar.child(Chip(cx, "Opening…", "accent"));
  bar.child(div().flex_1());
  // Guided mode walks the fields a template marks, so it shows only when this
  // branch's template for the form marks some.
  const template = app.store.templates(s.tin).find((x) => x.form_code === s.code);
  if (template && template.marked > 0) {
    bar.child(
      div()
        .flex()
        .flex_row()
        .items_center()
        .gap(SPACE[2])
        .child(Text(cx, "meta", "Guided mode", "fg2"))
        .child(
          Switch(cx, "fp-guided", s.guided, (v, cx) => {
            s.guided = v;
            cx.notify();
          }, "Guided mode"),
        ),
    );
  }

  const page = div().flex().flex_col().child(head).child(bar);
  if (s.sample) page.child(Text(cx, "meta", SAMPLE_NOTE, "fg2").mb(SPACE[4]).whitespace_normal());
  // An amended return says where its filed version is; that version, opened
  // from its history, says what it is.
  if (s.amends || s.filedOriginal) page.child(Text(cx, "meta", `This return amends a filed one. ${FILED_KEPT}`, "fg2").mb(SPACE[4]).whitespace_normal());
  if (s.filedOriginalView) page.child(Text(cx, "meta", filedOriginalStrip(s.filedOriginalView), "fg2").mb(SPACE[4]).whitespace_normal());
  // A return that left the app is read-only, and says when it went.
  if (!s.editable && !s.filedOriginalView && (s.submittedAt || s.confirmedAt)) {
    const parts = [];
    if (s.submittedAt) parts.push(`Sent to BIR ${shortDate(s.submittedAt)}`);
    if (s.confirmedAt) parts.push(`confirmed ${shortDate(s.confirmedAt)}`);
    page.child(Text(cx, "meta", `${parts.join(", ")}. Nothing here can be changed.`, "fg2").mb(SPACE[4]).whitespace_normal());
  }
  if (s.error) {
    return { page: page.child(div().p(SPACE[4]).rounded(RADIUS.card).bg(t.dangerSoft).child(Text(cx, "body", s.error, "danger").whitespace_normal())) };
  }
  if (s.loading) {
    return { page: page.child(div().py(40).flex().justify_center().child(Text(cx, "body", `Opening ${f.c}…`, "fg3"))) };
  }

  // Part navigator: 208 px beside the parts, a chip row below 760 px. Labels
  // wrap instead of truncating; each part shows its item count and, in red,
  // how many of its items have a validation message.
  const sideNav = width >= BREAK.formNav;
  const partsW = sideNav ? width - 208 - 28 : width;
  const errorsIn = (pi) => Object.keys(s.messages).filter((id) => id.startsWith(`${pi}:`)).length;
  const badge = (text, danger) =>
    div()
      .flex()
      .flex_none()
      .items_center()
      .justify_center()
      .h(18)
      .min_w(18)
      .px(5)
      .rounded(RADIUS.full)
      .bg(danger ? t.danger : t.hover)
      .child(Text(cx, "cap", text, danger ? "primaryFg" : "fg2").when(danger, (el) => el.text_color(t.surface)));
  const nav = sideNav ? div().flex().flex_col().gap(2).w(208).flex_none() : div().flex().flex_row().flex_wrap().gap(SPACE[1]).mb(SPACE[4]);
  // Heading: where you are, and a link to the next part with something to fix.
  const openOne = s.openParts.size === 1 ? [...s.openParts][0] : -1;
  const navHead = div()
    .flex()
    .flex_row()
    .items_center()
    .px(SPACE[2])
    .pb(6)
    .child(Text(cx, "metaStrong", "On this form", "fg3"))
    .child(div().flex_1())
    .child(Text(cx, "meta", openOne >= 0 ? `${openOne + 1} of ${f.parts.length}` : `${f.parts.length} parts`, "fg3"));
  const needing = f.parts.map((_p, i) => i).filter((i) => errorsIn(i) > 0);
  const attn = needing.length
    ? div()
        .id("fp-nav-attention")
        .px(SPACE[2])
        .pb(6)
        .cursor_pointer()
        .accessibility_label("Go to the next part that needs attention")
        .on_click((_e, cx) => ui.jumpTo(needing.find((i) => i > openOne) ?? needing[0], cx))
        .child(Text(cx, "meta", `${needing.length} part${needing.length > 1 ? "s" : ""} need${needing.length > 1 ? "" : "s"} attention · next ↓`, "danger"))
    : null;
  const navRows = [];
  f.parts.forEach((p, i) => {
    const current = s.openParts.has(i) && s.openParts.size === 1;
    const errors = errorsIn(i);
    if (sideNav) {
      // Compact (Settings > Compact form sections): one line per part, the
      // label shortened and cut with an ellipsis, the full name on hover.
      const compact = app.prefs.compactFormNav;
      const labelW = errors > 0 ? 208 - 16 - 8 - 26 - 26 - 4 : 208 - 16 - 8 - 26;
      const row = div()
        .id(`fp-nav-${i}`)
        .flex()
        .flex_row()
        .when(!compact, (el) => el.items_start().min_h(32).py(6))
        .when(compact, (el) => el.items_center().h(28).tooltip(partName(p, i)))
        .gap(SPACE[2])
        .w(208)
        .px(SPACE[2])
        .rounded(RADIUS.row)
        .cursor_pointer()
        .accessibility_label(partName(p, i))
        .when(current, (el) => el.bg(t.selected))
        .when(!current, (el) => el.hover((st) => st.bg(t.hover)))
        .on_click((_e, cx) => ui.jumpTo(i, cx))
        // Pixel widths, not flex-grow: a grown text cell has no definite width
        // and GPUI lays the wrapped label out one word per line.
        .child(
          compact
            ? Line(cx, "control", compactPartName(partName(p, i)), current ? "fg" : "fg2").w(labelW)
            : Text(cx, "control", partName(p, i), current ? "fg" : "fg2").w(labelW).line_height("18px").whitespace_normal(),
        )
        .child(div().flex().flex_row().flex_none().gap(SPACE[1]).when(errors > 0, (el) => el.child(badge(String(errors), true))).child(badge(String(p.items.length), false)));
      navRows.push(row);
    } else {
      nav.child(
        div()
          .id(`fp-nav-${i}`)
          .flex()
          .flex_row()
          .items_center()
          .gap(SPACE[1])
          .cursor_pointer()
          .on_click((_e, cx) => ui.jumpTo(i, cx))
          .child(Chip(cx, `${partName(p, i)} · ${p.items.length}`, s.openParts.has(i) ? "accent" : "neutral"))
          .when(errors > 0, (el) => el.child(badge(String(errors), true))),
      );
    }
  });

  const parts = div().flex().flex_col().gap(SPACE[4]).w(partsW);
  f.parts.forEach((p, pi) => {
    // Dev start route only (BIR_SHELL_START solo=<part>): draw that part alone.
    if (ui.devSolo != null && pi !== ui.devSolo) return;
    const open = s.openParts.has(pi);
    const card = div()
      .id(`fp-part-${pi}`)
      .flex()
      .flex_col()
      .rounded(RADIUS.card)
      .border(1)
      .border_color(t.border)
      .bg(t.surface)
      .overflow_hidden()
      .child(
        div()
          .id(`fp-part-${pi}-toggle`)
          .flex()
          .flex_row()
          .items_center()
          .gap(SPACE[2])
          .min_h(48)
          .pl(SPACE[3])
          .pr(SPACE[4])
          .cursor_pointer()
          .accessibility_label(partName(p, pi))
          .on_click((_e, cx) => ui.togglePart(pi, cx))
          .child(Icon(cx, open ? "chevron-down" : "chevron-right", { color: "fg3" }))
          // min_w(0) lets a long part name wrap instead of running under the count.
          .child(Text(cx, "control", partName(p, pi)).font_weight(600).flex_1().min_w(0).whitespace_normal())
          .child(Text(cx, "meta", `${p.items.length} item${p.items.length > 1 ? "s" : ""}`, "fg3").flex_none()),
      );
    if (open) {
      const body = div().flex().flex_col().border_t(1).border_color(t.border);
      const rowOf = (ii) => {
        const retained = s.rowViews.get(`${pi}:${ii}`);
        const row = retained ? retained.entity : itemRow(cx, ui, p.items[ii], pi, ii, partsW);
        if (!retained && ii > 0) row.border_t(1).border_color(t.border);
        return row;
      };
      // Items the profile filled fold into one group where the first sits.
      const tucked = ui.tuckedItems(pi);
      p.items.forEach((it, ii) => {
        // Dev start route only (BIR_SHELL_START items=a-b): show a slice.
        if (ui.devItems && (ii < ui.devItems.from || ii > ui.devItems.to)) return;
        if (tucked.includes(ii)) {
          if (ii !== tucked[0]) return;
          body.child(profileGroup(cx, ui, pi, tucked, ii > 0));
          if (s.profileOpen.has(pi)) {
            for (const tj of tucked) body.child(tj === 0 ? div().border_t(1).border_color(t.border).child(rowOf(tj)) : rowOf(tj));
          }
          return;
        }
        body.child(rowOf(ii));
      });
      card.child(body);
    }
    parts.child(card);
  });

  return { page, nav, navHead, attn, navRows, parts, sideNav };
}

export function renderFormPage(cx, app, width) {
  const c = formContent(cx, app, width);
  if (c.early) return c.early;
  if (!c.parts) return c.page;
  if (c.sideNav) {
    c.nav.child(c.navHead).when(Boolean(c.attn), (el) => el.child(c.attn));
    for (const row of c.navRows) c.nav.child(row);
    return c.page.child(div().flex().flex_row().items_start().gap(28).child(c.nav).child(c.parts));
  }
  return c.page.child(c.nav).child(c.parts);
}

/**
 * Wide windows: the part navigator is pinned beside the form instead of
 * scrolling away with it. The form scrolls in its own area; the navigator's
 * heading stays put and its list scrolls on its own (with a scrollbar) when
 * the parts need more height than the window has, so the wheel over it never
 * moves the form. `null` below the side-navigator width, where the page
 * keeps one scroll area and a chip row. `L` is the shell's layout().
 */
export function renderFormSplit(cx, app, L) {
  if (L.column < BREAK.formNav) return null;
  const c = formContent(cx, app, L.column);
  if (c.early) return null;
  // The column's left edge, matching the centred column of the main scroll area.
  const left = Math.max(L.padX, Math.floor((L.mainWidth - L.column) / 2));
  const navW = 208 + 6; // the rows plus room for the list's scrollbar
  const formW = L.column - 208 - 28;
  const navCol = div().flex().flex_col().flex_none().w(left + navW).pl(left).h_full().min_h(0).pt(28).pb(16);
  if (c.parts) {
    navCol
      .child(c.navHead)
      .when(Boolean(c.attn), (el) => el.child(c.attn))
      .child(div().id("fp-nav-list").flex().flex_col().gap(2).flex_1().min_h(0).w(navW).overflow_y_scrollbar().children(c.navRows));
  }
  const body = div().flex().flex_col().w(formW + 22).pl(22).pt(28).pb(56).child(c.page);
  if (c.parts) body.child(c.parts);
  return div()
    .id("fp-split")
    .flex()
    .flex_row()
    .flex_1()
    .min_h(0)
    .w_full()
    .child(navCol)
    .child(div().id("fp-scroll").flex_1().min_w(0).h_full().overflow_y_scroll().overflow_x_hidden().child(body));
}
