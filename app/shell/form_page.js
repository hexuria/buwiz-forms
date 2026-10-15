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

import { div } from "gpui-kit";
import { InputState, CalendarState, Popup } from "gpui-base";
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
  TextField,
  Dropdown,

  PageHeader,
  MetaLine,
} from "./ui.js";
import { formatTin } from "./store.js";
import { periodLabel } from "./periods.js";

const VALIDATE_DELAY = 300;

function message(e) {
  // Host errors arrive as "`bir.fn`: text"; the person needs only the text.
  return String(e && e.message ? e.message : e)
    .replace(/^Error:\s*/, "")
    .replace(/^`[^`]+`:\s*/, "");
}

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
  if (locked) return lockedBox(cx, (options.find((o) => o.value === shown) || { label: shown }).label, width);
  return Dropdown(cx, `fp-${k}`, {
    label,
    width,
    options,
    value: shown,
    open: s.openSelect === k,
    onOpenChange: (open, cx) => {
      s.openSelect = open ? k : null;
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
        .filter((it) => it.rows || (it.f && it.f.length)),
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
function itemKeys(it) {
  if (it.rows) return it.keys.flat();
  return (it.f || []).map((f) => f.k);
}

/** "1,234.50" from a number-ish string; empty stays empty. */
function formatMoney(text) {
  const n = Number(String(text).replace(/[₱,\s]/g, ""));
  if (String(text).trim() === "" || Number.isNaN(n)) return String(text);
  // QuickJS has no Intl, so group the digits by hand.
  const [whole, frac] = Math.abs(n).toFixed(2).split(".");
  return `${n < 0 ? "-" : ""}${whole.replace(/\B(?=(\d{3})+(?!\d))/g, ",")}.${frac}`;
}

export class FormPageActions {
  constructor(app) {
    this.app = app;
    this.state = null;
    this.token = 0;
  }

  /** Load the schema now and the return in the background. */
  open(opts, cx) {
    const app = this.app;
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
      errors: [],
      messages: {},
      formErrors: [],
      checking: false,
      openParts: new Set([0]),
      inputs: new Map(),
      pending: {},
      dirty: false,
      saved: false,
      openSelect: null,
      moreOpen: new Set(),
      guided: false,
    };
    if (!schema) return;
    cx.spawn(async (acx) => {
      try {
        const r = await app.store.call("screen_form_open", opts.tin, opts.code, opts.year, opts.period);
        if (token !== this.token) return;
        const s = this.state;
        s.loading = false;
        s.draft = r.draft;
        s.status = r.status;
        s.editable = r.editable;
        s.values = r.values;
        s.options = r.options || {};
        s.locks = r.locks;
        s.profile = new Set(r.profile || []);
        s.saved = Boolean(r.draft && r.draft.id);
        this.setErrors(r.errors);
        // Every part already open (one picked while the return was still
        // opening, too) gets its inputs now that the draft is here.
        for (const i of new Set([0, ...s.openParts])) this.ensureInputs(i);
        acx.notify();
      } catch (e) {
        if (token !== this.token) return;
        this.state.loading = false;
        this.state.error = message(e);
        acx.notify();
      }
    });
  }

  /** Create InputStates for one part's text-like controls (only once). */
  ensureInputs(partIndex) {
    const s = this.state;
    if (!s || !s.schema || !s.draft) return;
    const part = s.schema.parts[partIndex];
    if (!part) return;
    const make = (k, t) => {
      if (s.inputs.has(k)) return;
      let value = s.values[k] ?? "";
      if (t === "money" && value !== "") value = formatMoney(value);
      const input = InputState.new({ value: String(value) });
      input.on("change", (_e, cx) => this.edit(k, input.value(), cx));
      input.on("blur", (_e, cx) => {
        if (t === "money") input.set_value(formatMoney(input.value()));
        this.flush(cx);
      });
      s.inputs.set(k, input);
      (s.inputKinds ||= new Map()).set(k, t);
    };
    for (const it of part.items) {
      if (it.rows) {
        it.keys.forEach((row) => row.forEach((k, c) => make(k, it.cols[c].t)));
      } else {
        for (const f of it.f || []) if (["text", "money", "pct", "int", "date", "email", "phone"].includes(f.t)) make(f.k, f.t);
      }
    }
  }

  togglePart(i, cx) {
    const s = this.state;
    if (s.openParts.has(i)) s.openParts.delete(i);
    else {
      this.ensureInputs(i);
      s.openParts.add(i);
    }
    cx.notify();
  }

  /** Navigator: open the chosen part and fold the others, so it sits near the top. */
  jumpTo(i, cx) {
    const s = this.state;
    this.ensureInputs(i);
    s.openParts = new Set([i]);
    cx.notify();
  }

  /** Stage an edit; the draft takes it on the next flush (300 ms after the last edit). */
  edit(k, value, cx) {
    const s = this.state;
    if (!s || !s.editable) return;
    s.pending[k] = value;
    s.dirty = true;
    const token = this.token;
    s.editSeq = (s.editSeq || 0) + 1;
    const seq = s.editSeq;
    cx.timer.after(VALIDATE_DELAY, (acx) => {
      if (token !== this.token || seq !== this.state.editSeq) return;
      this.flush(acx);
    });
  }

  /** Fill an item's month, day and year boxes from a picked "YYYY-MM-DD". */
  pickDate(date, iso, cx) {
    const s = this.state;
    const [y, m, d] = iso.split("-");
    const edits = {};
    const put = (f, v) => {
      if (!f || f.c || s.locks[f.k] !== undefined) return;
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
    s.picker = null;
    this.choose(edits, cx);
  }

  /** Choice controls commit at once. */
  choose(edits, cx) {
    const s = this.state;
    if (!s || !s.editable) return;
    Object.assign(s.pending, edits);
    s.dirty = true;
    for (const [k, v] of Object.entries(edits)) s.values[k] = String(v);
    this.flush(cx);
  }

  /** Apply staged edits to the typed draft (fast, no database), then validate off-thread. */
  flush(cx) {
    const s = this.state;
    if (!s || !s.draft) return;
    const edits = s.pending;
    s.pending = {};
    if (Object.keys(edits).length) {
      try {
        const r = this.app.store.call("screen_form_fill", s.code, s.draft, edits);
        const before = s.values;
        s.draft = r.draft;
        s.values = r.values;
        s.options = r.options || {};
        this.syncComputed();
        this.syncChanged(before, edits);
        const rejected = Object.entries(r.rejected || {});
        if (rejected.length) {
          for (const [k, msg] of rejected) s.messages[this.itemIdForKey(k) || "form"] = msg;
        }
      } catch (e) {
        this.app.toast(message(e), cx, "triangle-alert");
      }
    }
    this.validate(cx);
  }

  /** Computed and locked boxes follow the draft; user boxes keep what was typed. */
  syncComputed() {
    const s = this.state;
    for (const [k, input] of s.inputs) {
      if (!(s.locks[k] !== undefined || this.isComputed(k))) continue;
      const v = s.values[k] ?? "";
      if (input.value() !== v) input.set_value(String(v));
    }
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
      const was = before[k] ?? "";
      const now = s.values[k] ?? "";
      if (was === now) continue;
      const shown = s.inputKinds?.get(k) === "money" && now !== "" ? formatMoney(now) : String(now);
      if (input.value() !== shown) input.set_value(shown);
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
        this.state.values = { ...this.state.values, ...r.values };
        this.state.options = r.options || {};
        this.syncComputed();
        this.setErrors(r.errors);
      } catch (e) {
        if (token !== this.token) return;
        this.state.formErrors = [message(e)];
      }
      if (token === this.token && seq === this.state.validateSeq) this.state.checking = false;
      acx.notify();
    });
    cx.notify();
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
    s.errors = errors || [];
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
    // Take whatever is staged before saving, without waiting for the debounce.
    const edits = s.pending;
    s.pending = {};
    try {
      if (Object.keys(edits).length) {
        const f = this.app.store.call("screen_form_fill", s.code, s.draft, edits);
        s.draft = f.draft;
        s.values = f.values;
      }
      const r = this.app.store.write("screen_form_save", s.code, s.draft);
      s.draft = r.draft;
      s.status = r.status;
      s.dirty = false;
      s.saved = true;
      this.app.toast("Draft saved.", cx, "save");
      this.validate(cx);
    } catch (e) {
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
    if (!s.dirty && s.saved) return true;
    this.save(cx);
    return s.saved && !s.dirty;
  }

  /** Reopen the return as stored (after queueing or cancelling). */
  reload(cx) {
    const s = this.state;
    this.open({ tin: s.tin, code: s.code, year: s.year, period: s.period }, cx);
  }

  queue(cx) {
    const s = this.state;
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
    try {
      if (this.usesPeriodPath()) this.app.store.write("cancel_queued", s.tin, s.code, s.year, s.period || null);
      else this.app.store.write("screen_form_cancel_queue", s.code, s.draft);
      this.app.toast("Queue cancelled. The return is an editable draft again.", cx);
      this.reload(cx);
    } catch (e) {
      this.app.toast(message(e), cx, "triangle-alert");
    }
  }

  /** Print preview of the return as saved: the official sheet, filled. */
  print(cx) {
    const s = this.state;
    if (s.editable && !this.ensureSaved(cx)) return;
    try {
      this.app.store.call("screen_form_print", s.code, s.draft);
      this.app.toast("Print preview opened in your browser.", cx, "printer");
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
    .child(Line(cx, "body", value || "", "fg2"));
}

function control(cx, ui, f, itemLabel, multi, available) {
  const s = ui.state;
  const t = tokens(cx);
  const width = controlWidth(f, available);
  const showSub = multi && f.l && f.l !== itemLabel && !/^\d+[A-Z]?$/.test(f.l);
  const locked = !s.editable || f.c || s.locks[f.k] !== undefined;
  const value = s.values[f.k] ?? "";
  let el;
  if (f.t === "sel") {
    el = selectBox(cx, ui, f.k, f.l, f.o, value, locked, width);
  } else if (f.t === "chk") {
    const on = value === "true" || value === "1" || value === "Y";
    return div()
      .id(`fp-${f.k}-row`)
      .flex()
      .flex_row()
      .items_center()
      .gap(SPACE[2])
      .child(Checkbox(cx, `fp-${f.k}`, on, (v, cx) => (locked ? null : ui.choose({ [f.k]: v ? "true" : "false" }, cx)), f.l || itemLabel))
      .child(Text(cx, "body", f.l || itemLabel, locked ? "fg2" : "fg").whitespace_normal());
  } else if (locked) {
    const shown = f.t === "money" ? formatMoney(value || (f.c ? "0" : "")) : value;
    el = lockedBox(cx, shown, width, { right: f.t === "money" || f.t === "pct" });
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
  const wrap = div().flex().flex_col().gap(SPACE[1]).min_w(0);
  if (showSub) wrap.child(Text(cx, "meta", f.l, "fg2").max_w(width).whitespace_normal());
  void t;
  return wrap.child(el);
}

function radioGroup(cx, ui, it, radios) {
  const s = ui.state;
  const t = tokens(cx);
  const row = div().flex().flex_row().flex_wrap().gap(6);
  for (const f of radios) {
    const on = s.values[f.k] === "true";
    const locked = !s.editable || f.c || s.locks[f.k] !== undefined;
    const pill = div()
      .id(`fp-${f.k}`)
      .flex()
      .items_center()
      .h(SIZE.controlSm)
      .px(SPACE[3])
      .rounded(RADIUS.full)
      .border(1)
      .border_color(on ? t.primary : t.borderStrong)
      .bg(on ? t.primary : t.surface)
      .accessibility_label(f.l)
      .child(Text(cx, "metaStrong", f.l.replace(/^.*?:\s*/, ""), on ? "primaryFg" : "fg2").whitespace_nowrap());
    if (!locked) {
      pill.cursor_pointer().hover((st) => (on ? st : st.border_color(t.fg3))).on_click((_e, cx) => {
        const edits = {};
        for (const r of radios) edits[r.k] = r.k === f.k ? "true" : "false";
        ui.choose(edits, cx);
      });
    } else pill.opacity(0.7);
    row.child(pill);
  }
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
    const locked = !s.editable || f.c || s.locks[f.k] !== undefined;
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
  const tableW = numW + shown.length * colW;
  const head = div().flex().flex_row().items_end().bg(t.bg).border_b(1).border_color(t.border);
  head.child(div().w(numW).flex_none().py(SPACE[2]).flex().justify_center().child(Text(cx, "meta", "#", "fg3")));
  for (const i of shown) head.child(div().w(colW).flex_none().py(SPACE[2]).px(SPACE[2]).child(Text(cx, "metaStrong", it.cols[i].l, "fg3").whitespace_normal()));
  const body = div().flex().flex_col();
  it.keys.forEach((row, r) => {
    const no = (it.start ?? 1) + r;
    const tr = div().flex().flex_row().items_center().when(r > 0, (el) => el.border_t(1).border_color(t.border));
    tr.child(div().w(numW).flex_none().flex().justify_center().child(Text(cx, "meta", String(no), "fg3")));
    row.forEach((k, c) => {
      const col = it.cols[c];
      if (col.t === "hidden") return;
      const locked = !s.editable || col.c || s.locks[k] !== undefined;
      const value = s.values[k] ?? "";
      let cell;
      if (col.t === "radio" || col.t === "chk") {
        const on = value === "true";
        cell = Checkbox(cx, `fp-${k}`, on, (v, cx) => (locked ? null : ui.choose({ [k]: v ? "true" : "false" }, cx)), `${col.l}, row ${no}`);
      } else if (col.t === "sel") {
        cell = selectBox(cx, ui, k, `${col.l}, row ${no}`, col.o, value, locked, colW - 16);
      } else if (locked || !s.inputs.get(k)) {
        cell = lockedBox(cx, col.t === "money" ? formatMoney(value) : value, colW - 16, { right: col.t === "money" });
      } else {
        cell = TextField(cx, s.inputs.get(k), { id: `fp-${k}`, label: `${col.l}, row ${no}`, width: colW - 16 }).h(30);
      }
      tr.child(div().w(colW).flex_none().py(6).px(SPACE[2]).child(cell));
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

const WEEKDAYS = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];
const MONTH_NAMES = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/** A calendar button beside an item's date boxes; picking a day fills them. Typing still works. */
function datePicker(cx, ui, id, date) {
  const s = ui.state;
  const t = tokens(cx);
  const open = s.picker && s.picker.id === id;
  const button = IconButton(cx, `fp-date-${id}`, "calendar", {
    tooltip: "Pick a date",
    pressed: open,
    onClick: (_e, cx) => {
      if (open) {
        s.picker = null;
        return cx.notify();
      }
      const cal = CalendarState.new();
      const m = Number(String(s.values[date.month.k] || "").slice(0, 2));
      const y = Number(s.values[date.year.k] || "");
      const d = date.day ? Number(s.values[date.day.k] || "") : 1;
      if (m >= 1 && m <= 12 && y > 1900) {
        cal.set_value(`${y}-${String(m).padStart(2, "0")}-${String(d >= 1 && d <= 31 ? d : 1).padStart(2, "0")}`);
      }
      cal.on("change", (value, cx) => {
        if (typeof value === "string") ui.pickDate(date, value, cx);
      });
      s.picker = { id, cal };
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
      s.picker = null;
      cx.notify();
    })
    .child(
      div()
        .flex()
        .flex_row()
        .items_center()
        .gap(SPACE[1])
        .mb(SPACE[1])
        .child(IconButton(cx, `fp-date-prev-${id}`, "chevron-left", { tooltip: "Previous month", onClick: (_e, cx) => { cal.prev_month(); cx.notify(); } }))
        .child(Text(cx, "control", `${MONTH_NAMES[cal.month() - 1]} ${cal.year()}`).font_weight(600).flex_1().text_center())
        .child(IconButton(cx, `fp-date-next-${id}`, "chevron-right", { tooltip: "Next month", onClick: (_e, cx) => { cal.next_month(); cx.notify(); } })),
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
  // only the extractor's guess).
  const fromProfile = tin || fs.some((x) => s.profile.has(x.k));
  // Markers are 16 px icons that explain themselves on hover, so a long
  // printed label is never covered; the label gets a pixel width.
  const markers = [];
  if (fromProfile) markers.push(["user", "From tax profile. Change it in the tax profile."]);
  if (allCalc) markers.push(["lock", "Computed from other items"]);
  const markerW = markers.length * 20 + (it.rows ? 64 : 0);
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

  let controls;
  if (it.rows) {
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
  } else if (tin) {
    controls = tinBoxes(cx, ui, it);
  } else {
    controls = div().flex().flex_row().flex_wrap().gap_x(SPACE[4]).gap_y(SPACE[3]).items_end();
    const radios = fs.filter((f) => f.t === "radio");
    if (radios.length) controls.child(radioGroup(cx, ui, it, radios));
    for (const f of fs) if (f.t !== "radio") controls.child(control(cx, ui, f, it.l, fs.length > 1, inner));
    const date = dateParts(fs);
    if (date && s.editable && ![date.month, date.day, date.year].filter(Boolean).every((f) => f.c || s.locks[f.k] !== undefined)) {
      controls.child(datePicker(cx, ui, id, date));
    }
  }

  const msg = s.messages[id] || "";
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
        .child(controls)
        // Reserved 18 px validation line, present with or without a message.
        .child(div().h(SIZE.validationLine).mt(SPACE[1]).overflow_hidden().child(Line(cx, "meta", msg, "danger").tooltip(msg || " "))),
    );
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

  const actions = [AppButton(cx, "fp-print", "Print preview", { size: "sm", icon: "printer", onClick: (_e, cx) => ui.print(cx) })];
  if (s.status === "Queued") {
    actions.push(AppButton(cx, "fp-cancel-queue", "Cancel queue", { size: "sm", onClick: (_e, cx) => ui.cancelQueue(cx) }));
  } else if (s.editable) {
    actions.push(AppButton(cx, "fp-check", "Check", { size: "sm", icon: "circle-check", onClick: (_e, cx) => ui.check(cx) }));
    actions.push(AppButton(cx, "fp-queue", "Queue", { size: "sm", icon: "send", disabled: s.loading, onClick: (_e, cx) => ui.queue(cx) }));
    actions.push(AppButton(cx, "fp-save", "Save draft", { variant: "primary", size: "sm", disabled: s.loading, onClick: (_e, cx) => ui.save(cx) }));
  }

  const chips = div().flex().flex_row().flex_wrap().items_center().gap(SPACE[2]).mt(SPACE[2]);
  chips.child(Chip(cx, f.freq)).child(Chip(cx, `${f.pages} page${f.pages > 1 ? "s" : ""}`)).child(Chip(cx, `${f.n} fields`));
  if (draftLayout) chips.child(Chip(cx, "Draft layout", "warning", "triangle-alert"));
  if (s.status !== "Draft") chips.child(Chip(cx, s.status, s.status === "Queued" ? "accent" : "success"));

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
  bar.child(Text(cx, "meta", `For ${periodLabel(f.freq, s.year, s.period)}`, "fg2"));
  if (s.checking || s.loading) bar.child(Chip(cx, s.loading ? "Opening…" : "Checking…", "accent"));
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
      p.items.forEach((it, ii) => {
        // Dev start route only (BIR_SHELL_START items=a-b): show a slice.
        if (ui.devItems && (ii < ui.devItems.from || ii > ui.devItems.to)) return;
        const row = itemRow(cx, ui, it, pi, ii, partsW);
        if (ii > 0) row.border_t(1).border_color(t.border);
        body.child(row);
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
