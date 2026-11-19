// The form page's session: the old FormPageActions state machine (app/shell/form_page.js), ported to signals.
// Pure logic plus door calls, no JSX. One session per open page; dispose() it when the page goes.
//
// Reactivity (docs/ui-performance-rules.md): the session keeps the old page's plain state (st) and PUBLISHES it into
// small signals after every change: one per box (value, locked, hint, off, options, invalid), one per row (RowInfo,
// compared by its JSON as the old rowSnapshot was), one per part, and a few page-level ones. A signal written with an
// equal value notifies nobody, so one edit re-runs only the bindings of the rows it changed; typing re-runs only its
// own box (the value signal) until the 300 ms flush.
//
// New behaviour (docs/plans/nsh-tsx-migration.md "Form page"):
// - Autosave: after a successful fill, AUTOSAVE_DELAY of quiet saves the draft (screen_form_save); saveState says
//   "saving" then "saved", or "failed" with the host's message (the next successful edit tries again). Not autosaved:
//   a read-only return; an amendment never saved (its first save is the filer's choice: saveNow()); a Form library
//   sample until its period is set (saveNow() asks for the period; once the host has built the sample for a complete
//   period that has no stored return, autosave saves it and the session reopens as that period's return).
// - Errors show only after the user has left a box of the row (touched), or after Check (all); a stored return's
//   messages at its opening show at once. Counts never wait for the text: the to-do list, the part badges, "n parts
//   need attention" and the pill's red count take every message, shown or not (the old page's todo() and partErrors).
import { batch, signal, untracked, type Accessor, type Signal } from "nsh";
import { isAvailable as hasSecondWindow } from "nsh/desktop";
import { Print } from "../print/native";
import {
  bir,
  type IncomeTaxElectionReadiness,
  type ScreenFormAsk,
  type ScreenFormError,
  type ScreenFormExisting,
  type ScreenFormFact,
  type ScreenFormFilled,
  type ScreenFormOpened,
  type ScreenFormPenalties,
  type ScreenFormPeriodPart,
  type ScreenFormSamplePeriod,
  type ScreenFormStatus,
} from "../../bir";
import { errorText } from "../../lib/errors";
import { cancelQueueMessage } from "../../lib/queue_message";
import { effectiveStops, nextRow, stopCall, todoRows, type TodoPart, type TodoRow } from "../../lib/todo";
import { formatMoney, wholePesosOf } from "../../lib/money";
import { DATE_ERROR, formatLongDate, formatMDY, oneDateText, parseTypedDate, shortIsoDate, shownDate } from "../../lib/dates";
import { sameAmount } from "../../ui/mask";
import type { BoxState, ChoiceOption, FormSchema, SchemaField, SchemaItem } from "./contract";
import { dateParts, isPageNotice, isTinItem, keysOf, listedFields, oneDate, optionOf, plainRefusal, sentWords, withoutHidden, type DateParts, type OneDate } from "./schema";

/** The boxes that hold a number: emptied, the host holds zero in them where it holds no blank (see FormSession.fill). */
const NUMBER_KINDS = new Set(["money", "pct", "int"]);
/** What an emptied number box is staged as when the host refuses a blank: the host's cleared value. */
const CLEARED = "0";

/** What a fill answers, with the boxes an emptied number box was cleared in. */
type Filled = ScreenFormFilled & { cleared?: string[] };

/** What a device with no second window says for Print preview (screens/print/host.tsx and src/print.rs say the same). */
const NO_PREVIEW = "Print preview is not available on this device.";

/** What Check, and a Queue the host refused, say of the items that need attention. */
const attentionText = (n: number) => `${n} item${n > 1 ? "s need" : " needs"} attention.`;

/** Why a month other than December is not taken for a calendar year's end (a calendar pick says it; so does a month chosen from the list that the fill put back). */
const CALENDAR_YEAR_ENDS = "A calendar year ends in December. Choose Fiscal to pick another month.";

/** A flush runs this long after the last edit (ms). */
export const VALIDATE_DELAY = 300;
/** Autosave runs after this much quiet following a successful fill (ms). */
export const AUTOSAVE_DELAY = 700;
/** A sample's period is asked about this long after its last change (ms). */
export const PERIOD_ASK_DELAY = 300;
/** Live print-preview sync, sooner than the flush (ms). */
export const LIVE_PREVIEW_DELAY = 120;
/** A removal's Undo is offered this long (the kit's toast with an action); autosave waits it out (ms). */
export const UNDO_WINDOW = 5000;
/** How long "Saving…" shows before the (synchronous) save runs, so it is painted (ms). */
const SAVING_PAINT = 40;

/** 2551Q's message for a later-quarter return whose year has no income-tax election on the profile. */
const ELECTION_UNRECORDED = /needs the taxpayer profile's recorded annual income-tax election/;
/** The Item 13 radio for each answer of the election, per form. */
const ELECTION_RADIOS: Record<string, Record<string, string>> = {
  "2551Q": { graduated: "frm2551Qv2018:taxRate1", eight_percent: "frm2551Qv2018:taxRate2" },
};

// ---------------------------------------------------------------- public types

/** What a page opens. mode: a stored or new return, a Form library sample, a new amendment, an amendment's filed original. */
export interface FormTarget {
  tin: string;
  formCode: string;
  year: number;
  period: number;
  /** The stored return's exact key (a transaction's MMDD, 2553's quarter). */
  key?: number | null;
  mode: "return" | "sample" | "amend" | "filed_original";
  /** filed_original: the history snapshot id. */
  snapshotId?: number;
  /** A new return started under this key (the typed date), offered for the sample newKeyFrom. */
  newKey?: number | null;
  newKeyFrom?: string | null;
}

export interface ToastAction {
  label: string;
  onPress: () => void;
}

/** What the session asks of the page around it. All optional: tests pass none. */
export interface SessionHooks {
  /** A toast (icon: an nsh/ui IconName); it may answer the toast's id (a string), so the page can close it again. */
  toast?(message: string, options?: { icon?: string; action?: ToastAction }): unknown;
  /** Close a toast the session showed (a removal's Undo that no longer applies). */
  dismissToast?(id: string): void;
  /** The session wrote through the door (name: the host function): the frame drops its cached reads. */
  afterWrite?(name: string): void;
  /** A changed return was saved, or one was stored for the first time (an amendment saved with nothing typed): the frame bumps it in Recent and settles the history. */
  onSaved?(target: FormTarget): void;
  /** The session now shows another return (a sample became its return, an offer loaded one, Undo came back). */
  onRetarget?(target: FormTarget): void;
  /** The return was queued or taken out of the queue. */
  onQueueChanged?(target: FormTarget): void;
}

export type SaveState =
  | { state: "idle" }
  | { state: "saving" }
  | { state: "saved" }
  /** Something is staged or changed and not saved yet. */
  | { state: "unsaved" }
  | { state: "failed"; message: string }
  /** Not autosaved: why ("Sample · set its period to save", "Amendment · not saved yet"). */
  | { state: "manual"; message: string };

export interface AskCard {
  fact: ScreenFormFact;
  label: string;
  note: string;
  control: ScreenFormAsk["control"];
  choices: { value: unknown; label: string }[];
  /** The picked choice's value as JSON, or null. */
  pick: string | null;
}

export type ElectionCard =
  | { kind: "ask"; question: string; pick: string | null }
  | { kind: "blocked"; title: string; body: string; action: { id: "fp-election-retry" | "fp-election-follow"; label: string } | null };

/** Everything a row draws besides its boxes. Compared by JSON: an equal RowInfo notifies nobody. */
export interface RowInfo {
  /** The message the row shows now ("" until touched or Check). */
  message: string;
  /** Why the row's shut blank boxes show blank. */
  notes: string[];
  /** What turns the row's faded boxes on ("Needs item 13 = Yes"). */
  needs: string[];
  /** A box this new return copied: from the template, the last return, or "". */
  kept: "template" | "last" | "";
  /** The row a step went to (tinted). */
  stepHere: boolean;
  /** "Stop here" offered, and whether it is on. */
  stoppable: boolean;
  isStop: boolean;
  /** The tooltip of the "From tax profile" mark, or "" when the row is not from the profile. */
  fromProfile: string;
  /** Edit is offered (profile boxes the filer may open), and they are open now. */
  canEdit: boolean;
  editing: boolean;
  /** "Save to profile too": offered, on, and its note. */
  toProfile: { on: boolean; note: string } | null;
  /** Edit penalties / Use the computed amounts. */
  penalty: "edit" | "computed" | null;
  /** Folded into its part's "From tax profile" group right now. */
  tucked: boolean;
  /** Fact cards on this row. */
  asks: AskCard[];
  askSaved: string;
  /** 2551Q Item 13's card, and its saved line. */
  election: ElectionCard | null;
  electionSaved: string;
  /** Notices the filer may mark checked. */
  checkable: { token: string; label: string }[];
  /** "Use it" from another item: its line, or "". */
  from: string;
}

export interface PartInfo {
  /** Messages shown on the part's rows, each host message counted once (two on one row count two). */
  errors: number;
  /** Rows of the part on the to-do list. */
  todo: number;
  open: boolean;
  /** The part's "From tax profile" fold: its first item, its items, its label, how many need attention, open. */
  fold: { first: number; items: number[]; label: string; flagged: number; open: boolean } | null;
}

export interface HeaderInfo {
  loading: boolean;
  error: string;
  status: ScreenFormStatus;
  editable: boolean;
  sample: boolean;
  periodSet: boolean;
  /** An amendment (or a return with a filed original). */
  amends: boolean;
  /** An amendment not saved yet: its first save is explicit. */
  amendUnsaved: boolean;
  /** "View the filed original" is offered. */
  filedOriginal: boolean;
  /** On the filed original: its strip, or "". */
  filedOriginalView: { status: string; filed_on: string | null } | null;
  /** "Sent to BIR … Nothing here can be changed." ("Dry run …, not sent to BIR" for a dry-run send), or "". */
  submittedLine: string;
  profileUpdates: string[];
  profileDiffers: string[];
  profileChanged: string[];
  notes: string[];
  formErrors: string[];
}

export type Overlay =
  | null
  | { kind: "removal"; message: string }
  | { kind: "existing"; existing: ScreenFormExisting }
  | { kind: "queue-confirm"; warns: string[] }
  | { kind: "sample-period"; parts: { key: string; part: ScreenFormPeriodPart["part"]; quarter: number | null; value: string }[]; error: string };

// ---------------------------------------------------------------- internal state (the old page's state object)

interface RemovalHeld {
  r: ScreenFormFilled;
  edits: Record<string, string>;
  typed: Record<string, string>;
  draft: object;
}

interface St {
  target: FormTarget;
  /** The amendment a filed original goes back to. */
  back: FormTarget | null;
  loading: boolean;
  error: string;
  draft: object | null;
  status: ScreenFormStatus;
  editable: boolean;
  storedPeriod: number | null;
  values: Record<string, string>;
  options: Record<string, unknown[]>;
  locks: Record<string, string>;
  lockNotes: Record<string, string>;
  notesAlways: Set<string>;
  profile: Set<string>;
  profileEdit: Set<string>;
  editing: Set<string>;
  opened: Set<string>;
  focusKey: string | null;
  profileOpen: Set<number>;
  sample: boolean;
  blank: Set<string>;
  needs: Record<string, string>;
  whys: Record<string, string>;
  waits: Record<string, string>;
  off: Record<string, string>;
  penalties: ScreenFormPenalties[];
  penaltyOpen: Set<string>;
  poolTexts: Record<string, string>;
  typed: Set<string>;
  typedEdits: Record<string, string>;
  periodParts: Map<string, ScreenFormPeriodPart>;
  periodSet: boolean;
  periodValues: Record<string, string>;
  periodAsked: string | null;
  periodAnswered: string | null;
  periodComplete: boolean;
  periodSeq: number;
  periodToken: number;
  periodInFlight: string | null;
  opens: ScreenFormSamplePeriod["opens"] | null;
  existing: ScreenFormExisting | null;
  saveAfterAnswer: { then: (() => void) | null } | null;
  submittedAt: string | null;
  /** The send `submittedAt` names was a dry run (the host's `submitted_dry_run`): nothing reached BIR. */
  submittedDryRun: boolean;
  /** That send went to the local mock relay, as the worker recorded it (the host's `submitted_via_relay`), not to BIR. */
  submittedViaRelay: boolean;
  confirmedAt: string | null;
  amends: ScreenFormStatus | null;
  filedOriginal: { id: number; status: string; filed_on: string | null } | null;
  filedOriginalView: { status: string; filed_on: string | null } | null;
  profileUpdates: string[];
  profileDiffers: string[];
  notes: { key: string; text: string; recorded: boolean }[];
  saved: boolean;
  quiet: { field: string; keys: string[] } | null;
  kept: Record<string, "last" | "template">;
  autoStops: string[];
  stopEdits: Record<string, boolean>;
  stopKeys: Set<string>;
  stoppable: Set<string>;
  passed: Set<string>;
  stepAt: string | null;
  stepFromTop: boolean;
  asks: ScreenFormAsk[];
  askAt: Map<string, ScreenFormAsk[]>;
  askPick: Record<string, unknown>;
  askText: Record<string, string>;
  askSavedAt: Map<string, string>;
  factErrors: ScreenFormError[];
  profileSave: NonNullable<ScreenFormOpened["profile_save"]>;
  profileEdited: Set<string>;
  toProfile: Set<string>;
  profileChanged: string[];
  errors: ScreenFormError[];
  messages: Record<string, string>;
  /** Keys each row's messages name (a red border on those boxes). */
  messageKeys: Record<string, string[]>;
  /** How many messages each row carries (host errors bound to it and refused edits): what the counts add up. */
  messageCounts: Record<string, number>;
  /** Boxes the host refused at the last fill, with its message (kept until the box is edited again). */
  rejected: Record<string, string>;
  formErrors: string[];
  checkable: Record<string, { token: string; label: string }[]>;
  electionAt: string | null;
  electionPick: string | null;
  electionSavedAt: string | null;
  profileRefreshed: boolean;
  dateErrors: Record<string, string>;
  datesEdited: Set<string>;
  pending: Record<string, string>;
  /** What the boxes showed before a choice, for a removal the filer cancels. */
  chosenFrom: Record<string, string>;
  dirty: boolean;
  editSeq: number;
  validateSeq: number;
  checking: boolean;
  removal: RemovalHeld | null;
  queueWarn: { warns: string[] } | null;
  openParts: Set<number>;
  current: number;
  touched: Set<string>;
  showAll: boolean;
  previewOpen: boolean;
  overlay: Overlay;
  saveState: SaveState;
  electionReady: IncomeTaxElectionReadiness | null | undefined;
  moreOpen: Set<string>;
}

function freshState(target: FormTarget): St {
  return {
    target, back: null, loading: true, error: "", draft: null, status: "Draft", editable: true, storedPeriod: null,
    values: {}, options: {}, locks: {}, lockNotes: {}, notesAlways: new Set(), profile: new Set(), profileEdit: new Set(),
    editing: new Set(), opened: new Set(), focusKey: null, profileOpen: new Set(), sample: target.mode === "sample",
    blank: new Set(), needs: {}, whys: {}, waits: {}, off: {}, penalties: [], penaltyOpen: new Set(), poolTexts: {},
    typed: new Set(), typedEdits: {}, periodParts: new Map(), periodSet: false, periodValues: {}, periodAsked: null,
    periodAnswered: null, periodComplete: false, periodSeq: 0, periodToken: 0, periodInFlight: null, opens: null,
    existing: null, saveAfterAnswer: null, submittedAt: null, submittedDryRun: false, submittedViaRelay: false, confirmedAt: null, amends: null, filedOriginal: null,
    filedOriginalView: null, profileUpdates: [], profileDiffers: [], notes: [], saved: false, quiet: null, kept: {},
    autoStops: [], stopEdits: {}, stopKeys: new Set(), stoppable: new Set(), passed: new Set(), stepAt: null,
    stepFromTop: false, asks: [], askAt: new Map(), askPick: {}, askText: {}, askSavedAt: new Map(), factErrors: [],
    profileSave: {}, profileEdited: new Set(), toProfile: new Set(), profileChanged: [], errors: [], messages: {},
    messageKeys: {}, messageCounts: {}, rejected: {}, formErrors: [], checkable: {}, electionAt: null, electionPick: null, electionSavedAt: null,
    profileRefreshed: false, dateErrors: {}, datesEdited: new Set(), pending: {}, chosenFrom: {}, dirty: false,
    editSeq: 0, validateSeq: 0, checking: false, removal: null, queueWarn: null, openParts: new Set([0]), current: 0,
    touched: new Set(), showAll: false, previewOpen: false, overlay: null, saveState: { state: "idle" },
    electionReady: undefined, moreOpen: new Set(),
  };
}

const sameJson = <T,>(a: T, b: T) => a === b || JSON.stringify(a) === JSON.stringify(b);

/** Two amounts are the same number; a blank one is zero. */
function sameNumber(a: string, b: string): boolean {
  const plain = (t: string) => {
    const v = t.replace(/[,\s₱%]/g, "");
    return v === "" || /^[-+]?0*\.?0*$/.test(v) ? "0" : v;
  };
  const x = plain(a);
  const y = plain(b);
  return x === y || sameAmount(x, y);
}

/** The pages open now (flushNow saves them all). */
const live = new Set<FormSession>();

/** Every open form page: typing flushed and saved now where autosave may (the frame awaits it before quit or navigation). False when a save was refused (its message was toasted). */
export async function flushNow(): Promise<boolean> {
  const done = await Promise.all([...live].map((s) => s.flushNow()));
  return done.every(Boolean);
}
const ticked = (v: unknown) => v === "true" || v === "1" || v === "Y";

/** Lazily made signals by key. */
class SignalMap<T> {
  private map = new Map<string, Signal<T>>();
  constructor(
    private initial: T,
    private equals?: (a: T, b: T) => boolean,
  ) {}
  get(k: string): Signal<T> {
    let s = this.map.get(k);
    if (!s) {
      s = this.equals ? signal<T>(this.initial, { equals: this.equals }) : signal<T>(this.initial);
      this.map.set(k, s);
    }
    return s;
  }
}

// ---------------------------------------------------------------- the session

export class FormSession {
  readonly schema: FormSchema | null;
  private st: St;
  private token = 0;
  private hooks: SessionHooks;
  private removalUndo: (() => void) | null = null;
  private undoToast: string | null = null;
  /** A sample became its return and the return is being opened: the page stays up, typing waits in pending. */
  private reopening = false;
  private timers = new Set<ReturnType<typeof setTimeout>>();
  private flushTimer: ReturnType<typeof setTimeout> | null = null;
  private saveTimer: ReturnType<typeof setTimeout> | null = null;
  private disposed = false;
  private readyWaiters: (() => void)[] = [];
  private focusers = new Map<string, () => void>();
  private fastLane = { drafts: false };
  private periodThen: (() => void) | null = null;

  // Schema indexes (static per session).
  private keyIndex = new Map<string, string>();
  private fieldOf = new Map<string, SchemaField>();
  private kindOf = new Map<string, string>();
  private computedKeys = new Set<string>();
  private wholeKeys = new Set<string>();
  private pools = new Map<string, { keys: string[] }>();
  private allKeys: string[] = [];
  private rowIds: string[] = [];
  private items = new Map<string, SchemaItem>();
  private oneDates = new Map<string, { date: OneDate; row: string }>();

  // Published signals.
  private sValue = new SignalMap<string>("");
  private sLocked = new SignalMap<boolean>(true);
  private sHint = new SignalMap<string>("");
  private sOff = new SignalMap<string>("");
  private sOptions = new SignalMap<ChoiceOption[]>([], sameJson);
  private sInvalid = new SignalMap<boolean>(false);
  private sRow = new SignalMap<RowInfo>(null as unknown as RowInfo, sameJson);
  private sPart = new SignalMap<PartInfo>({ errors: 0, todo: 0, open: false, fold: null }, sameJson);
  private sHeader = signal<HeaderInfo>(null as unknown as HeaderInfo, { equals: sameJson });
  private sTodo = signal<TodoRow[]>([], { equals: sameJson });
  private sSave = signal<SaveState>({ state: "idle" }, { equals: sameJson });
  private sChecking = signal(false);
  private sOverlay = signal<Overlay>(null, { equals: sameJson });
  private sCurrent = signal(0);
  private sStepAt = signal<string | null>(null);
  private sAllOpen = signal(false);
  private sReveal = signal<{ row: string | null; part: number; seq: number } | null>(null);
  private sAttention = signal(0);
  private revealSeq = 0;
  private boxes = new Map<string, BoxState>();

  constructor(target: FormTarget, hooks: SessionHooks = {}) {
    this.hooks = hooks;
    this.st = freshState(target);
    live.add(this);
    let schema: FormSchema | null = null;
    try {
      schema = withoutHidden(bir.screen_form_schema(target.formCode) as FormSchema);
    } catch (e) {
      this.st.error = errorText(e);
    }
    this.schema = schema;
    if (schema) this.indexSchema(schema);
    else {
      this.st.loading = false;
      this.st.error ||= "This form has no on-screen layout.";
    }
    this.publish();
  }

  private indexSchema(schema: FormSchema) {
    schema.parts.forEach((p, pi) =>
      p.items.forEach((it, ii) => {
        const id = `${pi}:${ii}`;
        this.rowIds.push(id);
        this.items.set(id, it);
        for (const k of keysOf(it)) this.keyIndex.set(k, id);
        if (it.rows) {
          (it.keys || []).forEach((row) =>
            row.forEach((k, c) => {
              const col = (it.cols || [])[c];
              if (!col || col.t === "hidden") return;
              this.kindOf.set(k, col.t);
              this.fieldOf.set(k, { k, l: col.l, t: col.t, c: col.c, w: col.w, o: col.o });
              if (col.c) this.computedKeys.add(k);
              if (col.w) this.wholeKeys.add(k);
              this.allKeys.push(k);
            }),
          );
          return;
        }
        for (const f of it.f || []) {
          this.kindOf.set(f.k, f.t);
          this.fieldOf.set(f.k, f);
          if (f.c) this.computedKeys.add(f.k);
          if (f.w) this.wholeKeys.add(f.k);
          if (f.pool) this.pools.set(f.k, f.pool);
          this.allKeys.push(f.k);
        }
        const date = oneDate(it);
        if (date) this.oneDates.set(date.oneKey, { date, row: id });
      }),
    );
  }

  // ---------------------------------------------------------------- reading

  /** What the session shows now (it changes when a sample becomes its return). */
  target(): FormTarget {
    return this.st.target;
  }

  /** One box's state (also "<month key>#date" for a one_date item). The same object for the life of the session. */
  box(k: string): BoxState {
    let b = this.boxes.get(k);
    if (!b) {
      const value = this.sValue.get(k);
      const locked = this.sLocked.get(k);
      const hint = this.sHint.get(k);
      const off = this.sOff.get(k);
      const options = this.sOptions.get(k);
      const invalid = this.sInvalid.get(k);
      b = {
        value: () => value(),
        locked: () => locked(),
        hint: () => hint(),
        off: () => off(),
        options: () => options(),
        invalid: () => invalid(),
        zeroWhenBlank: this.kindOf.get(k) === "money" && this.computedKeys.has(k),
      };
      this.boxes.set(k, b);
    }
    return b;
  }

  /** A radio group's ticked key, or null (its own binding over the radios' value signals). */
  radioSelected(keys: string[]): Accessor<string | null> {
    const sigs = keys.map((k) => [k, this.sValue.get(k)] as const);
    return () => sigs.find(([, s]) => s() === "true")?.[0] ?? null;
  }

  row(id: string): Accessor<RowInfo> {
    const s = this.sRow.get(id);
    return () => s();
  }
  part(pi: number): Accessor<PartInfo> {
    const s = this.sPart.get(String(pi));
    return () => s();
  }
  readonly header: Accessor<HeaderInfo> = () => this.sHeader();
  readonly todo: Accessor<TodoRow[]> = () => this.sTodo();
  readonly saveState: Accessor<SaveState> = () => this.sSave();
  readonly checking: Accessor<boolean> = () => this.sChecking();
  readonly overlay: Accessor<Overlay> = () => this.sOverlay();
  readonly current: Accessor<number> = () => this.sCurrent();
  readonly stepAt: Accessor<string | null> = () => this.sStepAt();
  readonly allPartsOpen: Accessor<boolean> = () => this.sAllOpen();
  /** A step's row to reveal (scroll to, then focus); seq changes on every step. */
  /** What the page brings into view: a step's row (then its keyboard focus), or a part the navigator or a part step went to. */
  readonly reveal: Accessor<{ row: string | null; part: number; seq: number } | null> = () => this.sReveal();
  /** Messages shown now, each counted once: the pill's red count, the part badges' sum and Check's toast agree on it. */
  readonly attention: Accessor<number> = () => this.sAttention();

  /** Plain reads (untracked), for tests and one-off decisions. */
  valueOf(k: string): string {
    return this.st.values[k] ?? "";
  }
  shownOf(k: string): string {
    return untracked(() => this.sValue.get(k)());
  }
  messageOf(id: string): string {
    return this.st.messages[id] || "";
  }
  messages(): Readonly<Record<string, string>> {
    return this.st.messages;
  }
  errors(): readonly ScreenFormError[] {
    return this.st.errors;
  }
  itemIdForKey(k: string): string | undefined {
    return this.keyIndex.get(k);
  }
  item(id: string): SchemaItem | undefined {
    return this.items.get(id);
  }
  isDirty(): boolean {
    return this.st.dirty || Object.keys(this.st.pending).length > 0;
  }
  isSaved(): boolean {
    return this.st.saved;
  }
  draft(): object | null {
    return this.st.draft;
  }
  status(): ScreenFormStatus {
    return this.st.status;
  }
  /** How many boxes the form binds (every key of the schema, schedule cells included). */
  fieldCount(): number {
    return this.allKeys.length;
  }
  rowCount(): number {
    return this.rowIds.length;
  }
  /** Resolves once the open finished (or failed). */
  ready(): Promise<void> {
    if (!this.st.loading) return Promise.resolve();
    return new Promise((resolve) => this.readyWaiters.push(resolve));
  }
  /** Resolves once no flush, validation, period question or save is pending (tests). */
  async idle(): Promise<void> {
    for (let i = 0; i < 600; i++) {
      const s = this.st;
      if (!s.loading && !s.checking && !this.flushTimer && !this.saveTimer && s.saveState.state !== "saving" && !s.periodInFlight && !this.periodWaiting) return;
      await new Promise((r) => setTimeout(r, 25));
    }
  }
  private periodWaiting = false;

  /** A control registers how to put the keyboard in its box (a pill step uses it). Radio groups: "radio:<row id>". */
  registerFocus(k: string, focus: () => void) {
    this.focusers.set(k, focus);
  }

  // ---------------------------------------------------------------- open

  /** Load the return (the host's async open). The error text, or null. */
  async open(target: FormTarget = this.st.target, keep = false): Promise<string | null> {
    if (!this.schema) return this.st.error;
    const back = this.st.back;
    const token = ++this.token;
    // keep: the page stays as it is until the host answers (a sample reopening as its return), typing is kept.
    this.reopening = keep;
    if (!keep) {
      this.st = freshState(target);
      if (target.mode === "filed_original") this.st.back = back;
      this.publish();
    }
    let r: ScreenFormOpened;
    try {
      const key = target.key ?? null;
      if (target.mode === "sample") r = await bir.screen_form_sample(target.tin, target.formCode);
      else if (target.mode === "filed_original") r = await bir.screen_form_filed_original(target.formCode, target.snapshotId ?? 0);
      else if (target.mode === "amend") r = await bir.screen_form_amend(target.tin, target.formCode, target.year, target.period, key);
      else if (target.newKeyFrom != null) r = await bir.screen_form_open(target.tin, target.formCode, target.year, target.period, key, target.newKey ?? null, target.newKeyFrom);
      else r = await bir.screen_form_open(target.tin, target.formCode, target.year, target.period, key, target.newKey ?? null);
    } catch (e) {
      if (token !== this.token || this.disposed) return null;
      if (keep) this.st = this.carried(target);
      this.reopening = false;
      this.st.loading = false;
      this.st.error = errorText(e);
      this.publish();
      this.wake();
      return this.st.error;
    }
    if (token !== this.token || this.disposed) return null;
    if (keep) this.st = this.carried(target);
    this.reopening = false;
    this.st.loading = false;
    this.takeOpened(r);
    // A stored return was checked before: the messages the host returns at its opening show at once (as the old page
    // shows them). A message that comes later shows once its row is left, or after Check.
    if (!keep && this.st.saved) for (const id of Object.keys(this.st.messages)) this.st.touched.add(id);
    // What was typed while it opened goes into the new draft.
    if (Object.keys(this.st.pending).length) {
      this.st.dirty = true;
      this.st.saveState = { state: "unsaved" };
      this.flush();
    }
    this.publish();
    this.wake();
    return null;
  }

  /** A fresh state for target that keeps the page as the filer had it: staged typing, open parts, touched rows. */
  private carried(target: FormTarget): St {
    const old = this.st;
    const s = freshState(target);
    s.pending = { ...old.pending };
    s.openParts = old.openParts;
    s.current = old.current;
    s.touched = old.touched;
    s.showAll = old.showAll;
    s.moreOpen = old.moreOpen;
    s.profileOpen = old.profileOpen;
    return s;
  }

  private wake() {
    const waiters = this.readyWaiters;
    this.readyWaiters = [];
    for (const w of waiters) w();
  }

  /** Take what the host opened (a return, a sample, or a sample rebuilt for its period). */
  private takeOpened(r: ScreenFormOpened) {
    const s = this.st;
    s.draft = r.draft;
    s.status = r.status;
    s.editable = r.editable;
    s.storedPeriod = r.stored_period ?? null;
    s.blank = new Set(r.blank || []);
    s.periodParts = new Map((r.period_parts || []).map((p) => [p.key, p]));
    s.periodSet = Boolean(r.period_set);
    s.submittedAt = r.submitted_at || null;
    s.submittedDryRun = Boolean(r.submitted_dry_run);
    s.submittedViaRelay = Boolean(r.submitted_via_relay);
    s.confirmedAt = r.confirmed_at || null;
    s.amends = r.amends || null;
    s.filedOriginal = r.filed_original || null;
    s.filedOriginalView = r.filed_original_view || null;
    s.values = this.shown(r.values);
    if (s.periodSet) {
      s.periodValues = Object.fromEntries([...s.periodParts].map(([k, p]) => [k, p.part === "quarter" ? String(s.values[k] === "true") : String(s.values[k] ?? "")]));
      s.periodAsked = JSON.stringify(s.periodValues);
      s.periodAnswered = s.periodAsked;
      s.periodComplete = true;
    }
    s.options = (r.options || {}) as Record<string, unknown[]>;
    s.locks = r.locks || {};
    s.lockNotes = r.lock_notes || {};
    s.notesAlways = new Set((r as { notes_always?: string[] }).notes_always || []);
    s.profile = new Set(r.profile || []);
    s.profileEdit = new Set(r.profile_edit || []);
    s.needs = r.needs || {};
    s.whys = r.whys || {};
    s.waits = r.waits || {};
    s.off = r.off || {};
    s.penalties = r.penalties || [];
    s.poolTexts = r.pools || {};
    s.profileUpdates = r.profile_updates || [];
    s.profileDiffers = r.profile_differs || [];
    s.notes = (r.notes || []).map((n) => ({ key: n.key, text: n.text, recorded: false }));
    s.saved = Boolean(r.draft && (r.draft as { id?: number }).id);
    s.quiet = r.quiet || null;
    s.kept = r.kept && typeof r.kept === "object" ? { ...r.kept } : {};
    s.autoStops = Array.isArray(r.auto_stops) ? r.auto_stops : [];
    s.stopEdits = r.stop_edits && typeof r.stop_edits === "object" ? r.stop_edits : {};
    s.stopKeys = effectiveStops(s.autoStops, s.stopEdits);
    s.stoppable = new Set(Array.isArray(r.stoppable) ? r.stoppable : []);
    s.profileSave = r.profile_save && typeof r.profile_save === "object" ? r.profile_save : {};
    s.profileChanged = Array.isArray(r.profile_changed) ? r.profile_changed : [];
    this.takeAsks(r.asks);
    s.factErrors = Array.isArray(r.fact_errors) ? r.fact_errors : [];
    this.setErrors(r.errors);
    s.saveState = this.restingSaveState();
    this.resetShown();
  }

  /** What the save line says while nothing is being saved. */
  private restingSaveState(): SaveState {
    const s = this.st;
    if (!s.editable || !s.draft) return { state: "idle" };
    if (s.sample && !s.periodSet) return { state: "manual", message: "Sample · set its period to save" };
    if (s.amends && !s.saved) return { state: "manual", message: "Amendment · not saved yet" };
    if (s.dirty) return { state: "unsaved" };
    return s.saved ? { state: "saved" } : { state: "idle" };
  }

  /** Whether autosave may save this return now. */
  private canAutosave(): boolean {
    const s = this.st;
    if (!s.draft || !s.editable || s.loading || s.removal || this.reopening) return false;
    if (s.target.mode === "filed_original") return false;
    if (s.amends && !s.saved) return false;
    if (s.sample) return s.periodSet && !s.existing && s.periodAnswered === s.periodAsked;
    return true;
  }

  private takeAsks(asks: ScreenFormAsk[] | undefined) {
    const s = this.st;
    s.asks = !s.sample && s.editable && Array.isArray(asks) ? asks : [];
    s.askAt = new Map();
    const first = this.schema?.parts[0]?.items.length ? "0:0" : null;
    for (const ask of s.asks) {
      const id = (ask.key && this.keyIndex.get(ask.key)) || first;
      if (!id) continue;
      if (!s.askAt.has(id)) s.askAt.set(id, []);
      s.askAt.get(id)!.push(ask);
    }
    for (const fact of Object.keys(s.askText)) if (!s.asks.some((a) => a.fact === fact)) delete s.askText[fact];
  }

  private unquiet(keys?: string[]) {
    const s = this.st;
    if (s.quiet && (!keys || keys.some((k) => s.quiet!.keys.includes(k)))) s.quiet = null;
  }

  /** The page has drawn the opening: its one-time notes are recorded seen (the host records each once). */
  notesDrawn() {
    const s = this.st;
    const pending = s.notes.filter((n) => !n.recorded);
    if (!pending.length || s.loading) return;
    let recorded: string[];
    try {
      recorded = bir.screen_form_notes_seen(pending.map((n) => n.key));
    } catch (_) {
      return;
    }
    s.notes = s.notes.filter((n) => n.recorded || recorded.includes(n.key)).map((n) => ({ ...n, recorded: true }));
    this.publish();
  }

  // ---------------------------------------------------------------- editing

  /** Typing in box k (or "<month>#date"); a sel or chk commits at once. Text is staged and flushed VALIDATE_DELAY later. */
  edit(k: string, text: string) {
    const s = this.st;
    if (!s.editable || !s.draft) return;
    const one = this.oneDates.get(k);
    if (one) return this.editOneDate(one.date, text);
    const kind = this.kindOf.get(k);
    if (kind === "sel" || kind === "chk" || kind === "radio") return this.choose({ [k]: text });
    this.sValue.get(k).set(text);
    if (s.periodParts.has(k)) return this.setPeriodPart({ [k]: text });
    let value = text;
    if (kind === "date") {
      const date = parseTypedDate(text);
      s.datesEdited.add(k);
      if (date || !text.trim()) this.setDateError(k, "");
      value = date ? formatMDY(date) : text;
    }
    this.stage({ [k]: value });
  }

  private editOneDate(date: OneDate, text: string) {
    this.sValue.get(date.oneKey).set(text);
    const typed = parseTypedDate(text);
    if (typed || !text.trim()) this.setDateError(date.month.k, "");
    if (!typed && text.trim()) return;
    const [m, d, y] = typed ? formatMDY(typed).split("/") : ["", "", ""];
    const edits: Record<string, string> = { [date.month.k]: m, [date.year.k]: y };
    if (date.day) edits[date.day.k] = d;
    this.stage(edits);
  }

  /**
   * Whether a split date is a year-end date held at December: the item before it is the "For: Calendar / Fiscal" pair (1702Q, 2551Q, 2550Q
   * ... Year Ended) and Calendar is ticked. A calendar year ends in December: 1702Q puts the month back at every fill and says nothing, the
   * others flag it. rows.tsx dateRange limits the calendar to it. Reads the boxes (reactive).
   */
  calendarYearEnd(date: DateParts): boolean {
    const [part, index] = (this.keyIndex.get(date.month.k) ?? "").split(":").map(Number);
    const before = index > 0 ? this.items.get(`${part}:${index - 1}`) : undefined;
    return (before?.f ?? []).some((f) => f.t === "radio" && /^calendar\b/i.test(f.l) && this.box(f.k).value() === "true");
  }

  /**
   * A day picked on the calendar beside an item's split month, day and year boxes ("YYYY-MM-DD") fills the parts that are open:
   * a locked part is skipped (the old app did the same), so the page's calendar offers only the days the locked parts allow
   * (rows.tsx dateRange passes min and max). A part that is a list takes the choice by its value, or a year list by the whole
   * year it shows ("26" listed for 2026). Only an open list that cannot hold the pick refuses the whole pick, so a date is never
   * half changed by a list.
   */
  pickDate(date: DateParts, iso: string) {
    if (!this.st.editable || !this.st.draft) return;
    const [y, m, d] = iso.slice(0, 10).split("-");
    if (!y || !m || !d) return;
    // A calendar year ends in December and the form puts any other month back: where the calendar could not leave the day out (the year is
    // not locked, a sample) the pick is refused with the reason, never reverted without a word.
    if (this.calendarYearEnd(date) && Number(m) !== 12 && !this.isShut(date.month.k)) {
      return void this.toast(CALENDAR_YEAR_ENDS, "triangle-alert");
    }
    const valueFor = (f: SchemaField, v: string): string | null => {
      if (!(f.t === "sel" && f.o)) return v;
      const listed = f.o.map(optionOf).filter((o) => o !== null);
      if (!listed.length) return v;
      const hit = listed.find((o) => o.value === v || o.value === String(Number(v)) || (f === date.year && o.label === String(Number(v))));
      return hit ? hit.value : null;
    };
    const parts: [SchemaField | null, string][] = [[date.month, m], [date.day, d], [date.year, y]];
    const edits: Record<string, string> = {};
    for (const [f, v] of parts) {
      if (!f || this.isShut(f.k)) continue;
      const chosen = valueFor(f, v);
      if (chosen === null) return void this.toast("That date is not on this form's list.", "triangle-alert");
      edits[f.k] = chosen;
    }
    if (!Object.keys(edits).length) return;
    this.setDateError(date.month.k, "");
    this.choose(edits);
  }

  private stage(edits: Record<string, string>) {
    const s = this.st;
    for (const [k, v] of Object.entries(edits)) {
      s.pending[k] = v;
      if (s.opened.has(k) && s.profileSave[k]) s.profileEdited.add(k);
      delete s.kept[k];
      delete s.rejected[k];
    }
    s.dirty = true;
    this.unquiet(Object.keys(edits));
    this.cancelAutosave();
    this.markUnsaved();
    const seq = ++s.editSeq;
    const token = this.token;
    if (this.flushTimer) clearTimeout(this.flushTimer);
    this.flushTimer = this.later(VALIDATE_DELAY, () => {
      this.flushTimer = null;
      if (token !== this.token || seq !== this.st.editSeq) return;
      this.flush();
    });
    this.later(LIVE_PREVIEW_DELAY, () => {
      if (token !== this.token || seq !== this.st.editSeq) return;
      this.livePreview();
    });
  }

  /** A choice (sel, chk, a radio pick, a cleared row): committed at once. */
  choose(edits: Record<string, string>) {
    const s = this.st;
    if (!s.editable || !s.draft) return;
    const period = Object.fromEntries(Object.entries(edits).filter(([k]) => s.periodParts.has(k)));
    if (Object.keys(period).length) {
      this.setPeriodPart(period);
      edits = Object.fromEntries(Object.entries(edits).filter(([k]) => !(k in period)));
      if (!Object.keys(edits).length) return;
    }
    Object.assign(s.pending, edits);
    s.dirty = true;
    this.unquiet(Object.keys(edits));
    for (const k of Object.keys(edits)) {
      delete s.kept[k];
      delete s.rejected[k];
    }
    s.chosenFrom = Object.fromEntries(Object.keys(edits).map((k) => [k, s.values[k] ?? ""]));
    for (const [k, v] of Object.entries(edits)) {
      s.values[k] = String(v);
      this.sValue.get(k).set(this.shownText(k));
    }
    this.cancelAutosave();
    this.markUnsaved();
    this.flush();
  }

  /** The save line says "Unsaved changes" the moment something is staged (an explicit-save return keeps its reason). */
  private markUnsaved() {
    const s = this.st;
    if (s.saveState.state === "manual") return;
    s.saveState = { state: "unsaved" };
    this.sSave.set(s.saveState);
  }

  /** A pick in a radio group, with the old group rules: untick in groups of 3+, a pair keeps one answer, grp siblings untick. */
  pickRadio(k: string) {
    const s = this.st;
    const id = this.keyIndex.get(k);
    const it = id ? this.items.get(id) : null;
    const f = this.fieldOf.get(k);
    if (!it || !f || this.isShut(k)) return;
    // A pick leaves the row as a blur does: its message may show now.
    s.touched.add(id!);
    const on = s.values[k] === "true";
    const period = s.periodParts.has(k);
    if (on && !period && (f.rg || 0) >= 3) return this.choose({ [k]: "false" });
    if (on && !period && !f.rg) return;
    const edits: Record<string, string> = { [k]: "true" };
    if (f.grp) for (const r of it.f || []) if (r.t === "radio" && r.k !== k && r.grp === f.grp) edits[r.k] = "false";
    this.choose(edits);
  }

  /** A box gained focus: an item still open with Edit elsewhere goes back to its profile look. */
  focus(k: string) {
    const s = this.st;
    s.focusKey = k;
    const here = this.keyIndex.get(this.oneDates.get(k)?.date.month.k ?? k);
    if (this.endProfileEdit([...s.editing].filter((key) => this.keyIndex.get(key) !== here))) this.publish();
  }

  /** A box lost focus: its row is touched (its message may show now), an amount or a date takes its shown form, and it flushes. */
  blur(k: string) {
    const s = this.st;
    const one = this.oneDates.get(k);
    const baseKey = one ? one.date.month.k : k;
    const id = this.keyIndex.get(baseKey);
    if (s.focusKey === k) s.focusKey = null;
    const kind = one ? "date" : this.kindOf.get(k);
    const v = this.sValue.get(k);
    const raw = untracked(() => v());
    if (kind === "money" && !this.isShut(k)) v.set(formatMoney(raw, this.wholeKeys.has(k)));
    else if (kind === "date" && !this.isShut(k)) {
      const date = parseTypedDate(raw);
      if (date) {
        v.set(formatLongDate(date));
        this.setDateError(baseKey, "");
      } else this.setDateError(baseKey, raw.trim() && (one || s.datesEdited.has(k)) ? DATE_ERROR : "");
    }
    if (id) s.touched.add(id);
    const editKeys = this.itemEditKeys(baseKey);
    if (editKeys.some((key) => s.editing.has(key))) {
      const token = this.token;
      this.later(50, () => {
        if (token !== this.token) return;
        const now = this.st.focusKey;
        if (now && this.keyIndex.get(now) === this.keyIndex.get(baseKey)) return;
        if (this.endProfileEdit(this.itemEditKeys(baseKey))) this.publish();
      });
    }
    if (Object.keys(s.pending).length) this.flush();
    else this.publish();
  }

  /** The part whose box has the keyboard now, or null: the page keeps that part's rows drawn wherever it scrolls. */
  focusedPart(): number | null {
    const k = this.st.focusKey;
    const id = k ? this.keyIndex.get(this.oneDates.get(k)?.date.month.k ?? k) : undefined;
    return id ? Number(id.split(":")[0]) : null;
  }

  /** Mark a row touched without a blur (a pick in a choice, a schedule row closed). */
  touch(id: string) {
    this.st.touched.add(id);
    this.publish();
  }

  private setDateError(k: string, msg: string) {
    const id = this.keyIndex.get(k);
    if (!id) return;
    if (msg) this.st.dateErrors[id] = msg;
    else delete this.st.dateErrors[id];
  }

  /** Push the draft and staged edits to an open print preview (best effort). */
  private livePreview() {
    const s = this.st;
    if (!s.previewOpen || !s.draft) return;
    try {
      bir.screen_form_preview_sync(s.target.formCode, s.draft, s.pending, this.unlockedFor(s.pending));
    } catch (_) {
      /* best effort */
    }
  }

  /** Apply staged edits to the draft (sync), then validate (async); a successful fill schedules autosave. */
  flush() {
    const s = this.st;
    if (this.flushTimer) {
      clearTimeout(this.flushTimer);
      this.flushTimer = null;
    }
    if (!s.draft || this.reopening) return;
    const edits = s.pending;
    s.pending = {};
    let filled = false;
    if (Object.keys(edits).length) {
      this.retireRemovalUndo();
      try {
        const r = this.fill(s.draft, edits);
        if (r.removes) return this.holdRemoval(r, edits);
        this.applyFill(r, edits);
        filled = true;
      } catch (e) {
        s.pending = { ...edits, ...s.pending };
        s.saveState = { state: "failed", message: errorText(e) };
        this.toast(errorText(e), "triangle-alert");
      }
    }
    this.validate();
    if (filled) this.scheduleAutosave();
  }

  /**
   * The host's fill for staged edits, with the rule for a number box the filer emptied. Select-all and Backspace, Cut: the box
   * reports "" and the host refuses a blank amount or whole number ("must be an amount ... (got ``)"), so the draft kept its
   * old number and the box got it back. A refused blank in a money, percent or integer box is staged as the host's cleared
   * value instead, zero, and `edits` is changed to say so (the caller applies and records what the host took). The draft and
   * the box then agree, and a box the host does hold blank (a nullable amount, a schedule row cell, a text) stays blank:
   * only a refusal asks again. `cleared` lists the boxes that now hold zero: applyFill shows it in them.
   */
  private fill(draft: object, edits: Record<string, string>): Filled {
    const code = this.st.target.formCode;
    const r: Filled = bir.screen_form_fill(code, draft, edits, this.unlockedFor(edits));
    if (r.removes) return r;
    const refused = Object.keys(r.rejected || {}).filter((k) => k in edits && String(edits[k]).trim() === "" && NUMBER_KINDS.has(this.kindOf.get(k) ?? ""));
    if (!refused.length) return r;
    const zero = Object.fromEntries(refused.map((k) => [k, CLEARED]));
    const z: Filled = bir.screen_form_fill(code, r.draft, zero, this.unlockedFor(zero));
    const cleared = refused.filter((k) => !(k in (z.rejected || {})));
    if (!cleared.length) return r;
    // The first answer's refusals stay for the boxes that could not be cleared either.
    const rejected = { ...r.rejected };
    for (const k of cleared) {
      delete rejected[k];
      edits[k] = CLEARED;
    }
    return { ...z, rejected, cleared };
  }

  /** Take what the host applied for edits. */
  private applyFill(r: Filled, edits: Record<string, string>) {
    const s = this.st;
    this.accepted(edits, r.rejected);
    if (s.sample && Array.isArray(r.blank)) s.blank = new Set(r.blank);
    const before = s.values;
    s.draft = r.draft;
    s.values = this.shown(r.values);
    s.options = (r.options || {}) as Record<string, unknown[]>;
    if (Array.isArray(r.penalties)) s.penalties = r.penalties;
    if (r.pools) s.poolTexts = r.pools;
    const always = (r as { notes_always?: string[] }).notes_always;
    if (Array.isArray(always)) s.notesAlways = new Set(always);
    for (const [k, lock] of Object.entries(r.relocks || {})) {
      if (lock == null) {
        delete s.locks[k];
        delete s.lockNotes[k];
      } else {
        s.locks[k] = lock;
        const note = r.relock_notes?.[k];
        if (note) s.lockNotes[k] = note;
      }
    }
    this.syncShown(before, edits);
    this.livePreview();
    this.sayYearEndPutBack(edits);
    // A refused box shows what the draft holds again; its message shows (its row is touched) until it is edited again.
    for (const [k, msg] of Object.entries(r.rejected || {})) {
      const id = this.keyIndex.get(k);
      s.messages[id || "form"] = msg;
      if (!id) continue;
      s.rejected[k] = msg;
      s.touched.add(id);
      if (!(k in s.pending)) this.sValue.get(k).set(this.shownText(k));
    }
    // An emptied number box the host holds as zero shows the zero (the box selects it, so the next digit replaces it).
    for (const k of r.cleared || []) if (!(k in s.pending)) this.sValue.get(k).set(this.shownText(k));
  }

  /**
   * A month chosen from a year-end date's list while Calendar is ticked, and the fill put another back (1702Q puts December back at every
   * fill, without a word): the filer is told why, as a calendar pick is. A form that keeps the month (2551Q flags it on the row, 2550Q does
   * not mind) has nothing put back and nothing to say here.
   */
  private sayYearEndPutBack(edits: Record<string, string>) {
    const s = this.st;
    for (const [k, chosen] of Object.entries(edits)) {
      const item = this.items.get(this.keyIndex.get(k) ?? "");
      const date = item?.f ? dateParts(item.f) : null;
      if (!date || date.month.k !== k || !this.calendarYearEnd(date)) continue;
      const asked = Number(String(chosen).slice(0, 2));
      if (asked >= 1 && asked <= 12 && asked !== Number(String(s.values[k] ?? "").slice(0, 2))) return void this.toast(CALENDAR_YEAR_ENDS, "triangle-alert");
    }
  }

  /** An answer that removes typed rows asks first: the old answer shows until Continue. */
  private holdRemoval(r: ScreenFormFilled, edits: Record<string, string>) {
    const s = this.st;
    const chosen = s.chosenFrom || {};
    for (const k of Object.keys(edits)) if (k in chosen) s.values[k] = chosen[k] ?? "";
    const typed = Object.fromEntries(Object.entries(edits).filter(([k]) => !(k in chosen)));
    s.removal = { r, edits, typed, draft: s.draft! };
    for (const k of Object.keys(chosen)) this.sValue.get(k).set(this.shownText(k));
    s.overlay = { kind: "removal", message: r.removes!.message };
    this.markUnsaved();
    this.publish();
  }

  /** Continue: the answer and its removal apply; Undo (on the toast) brings both back. */
  confirmRemoval() {
    const s = this.st;
    const held = s.removal;
    if (!held) return;
    s.removal = null;
    s.overlay = null;
    this.applyFill(held.r, held.edits);
    s.dirty = true;
    this.validate();
    const state = s;
    const undo = () => this.undoRemoval(held.draft, state, undo);
    this.removalUndo = undo;
    this.undoToast = this.toast(`${held.r.removes!.done}.`, "trash", { label: "Undo", onPress: undo });
    // Any save retires the Undo (it would bring back a draft older than the stored one), so autosave waits it out.
    this.scheduleAutosave();
    const token = this.token;
    this.later(UNDO_WINDOW, () => {
      if (token !== this.token || this.removalUndo !== undo) return;
      this.retireRemovalUndo();
      this.scheduleAutosave();
    });
  }

  /** Cancel: the old answer and the rows stay, and so does what was typed with it. */
  cancelRemoval() {
    const s = this.st;
    const held = s.removal;
    s.removal = null;
    s.overlay = null;
    if (held && Object.keys(held.typed).length) {
      const typed = held.typed;
      try {
        const r = this.fill(s.draft!, typed);
        if (r.removes) {
          for (const k of Object.keys(typed)) this.sValue.get(k).set(this.shownText(k));
        } else {
          this.applyFill(r, typed);
          s.dirty = true;
          this.validate();
          this.scheduleAutosave();
        }
      } catch (e) {
        s.pending = { ...typed, ...s.pending };
        this.toast(errorText(e), "triangle-alert");
      }
    }
    this.publish();
  }

  /** Whether the removal's Undo is still offered (a later edit or save retires it). */
  canUndoRemoval(): boolean {
    return this.removalUndo !== null;
  }

  /** The removal's Undo, as its toast runs it (tests). */
  undoLastRemoval() {
    this.removalUndo?.();
  }

  /** The removal's Undo goes (an edit, a save, leaving the page), and its toast with it. */
  private retireRemovalUndo() {
    this.removalUndo = null;
    const id = this.undoToast;
    this.undoToast = null;
    if (id) this.hooks.dismissToast?.(id);
  }

  /** Undo a removal: the draft from before the answer comes back, on the same page only. */
  private undoRemoval(draft: object, state: St, undo: () => void) {
    const s = this.st;
    if (s !== state || this.removalUndo !== undo || !s.editable) return;
    this.removalUndo = null;
    this.undoToast = null;
    // Never a draft older than the stored one (a save retires Undo; this guards the rest).
    const id = (d: object | null) => (d as { id?: number | null } | null)?.id ?? null;
    if (id(draft) !== id(s.draft)) return void this.toast("Undo is no longer available: the return was saved since.", "triangle-alert");
    try {
      const r = bir.screen_form_fill(s.target.formCode, draft, {}, []);
      const before = { ...s.values };
      this.applyFill(r, {});
      for (const k of this.allKeys) if (before[k] !== s.values[k]) this.sValue.get(k).set(this.shownText(k));
    } catch (e) {
      return this.toast(errorText(e), "triangle-alert");
    }
    s.dirty = true;
    this.validate();
    this.scheduleAutosave();
  }

  /** Validate the draft off the engine thread; take values, options, off, penalties and errors. */
  private validate() {
    const s = this.st;
    if (!s.draft) return;
    const token = this.token;
    s.checking = true;
    const seq = ++s.validateSeq;
    const draft = s.draft;
    this.publish();
    bir.screen_form_validate(s.target.formCode, draft).then(
      (r) => {
        if (token !== this.token || this.disposed || seq !== this.st.validateSeq) return;
        const st = this.st;
        if (st.draft === draft) st.draft = r.draft;
        if (st.sample && Array.isArray(r.blank)) st.blank = new Set(r.blank);
        st.values = this.shown({ ...st.values, ...r.values });
        st.options = (r.options || {}) as Record<string, unknown[]>;
        st.off = r.off || {};
        if (Array.isArray(r.penalties)) st.penalties = r.penalties;
        if (r.pools) st.poolTexts = r.pools;
        st.factErrors = Array.isArray(r.fact_errors) ? r.fact_errors : [];
        this.syncComputed();
        this.setErrors(r.errors);
        st.formErrors = [];
        st.checking = false;
        this.publish();
      },
      (e) => {
        if (token !== this.token || this.disposed) return;
        this.st.formErrors = [errorText(e)];
        if (seq === this.st.validateSeq) this.st.checking = false;
        this.publish();
      },
    );
  }

  /** Bind each validation message to an item (bindError). */
  private setErrors(errors: ScreenFormError[] | undefined) {
    const s = this.st;
    s.errors = [...(errors || []), ...(s.factErrors || [])].filter((e) => !(s.quiet && e.field === s.quiet.field));
    s.messages = {};
    s.messageKeys = {};
    s.messageCounts = {};
    s.checkable = {};
    s.electionAt = null;
    const items = this.rowIds.map((id) => ({ id, it: this.items.get(id)! }));
    for (const e of s.errors) {
      const id = bindError(e, items, (k) => this.keyIndex.get(k));
      if (!id) continue;
      s.messages[id] = s.messages[id] ? `${s.messages[id]} · ${e.message}` : e.message;
      (s.messageKeys[id] ||= []).push(...(e.keys || []));
      s.messageCounts[id] = (s.messageCounts[id] || 0) + 1;
      if (e.dismiss) (s.checkable[id] ||= []).push({ token: e.dismiss, label: e.dismiss_label || "Checked" });
      if (e.field === "annual_income_tax_election" && ELECTION_UNRECORDED.test(e.message)) s.electionAt = id;
    }
    for (const [k, msg] of Object.entries(s.rejected)) {
      const id = this.keyIndex.get(k);
      if (!id) continue;
      s.messages[id] = s.messages[id] ? `${msg} · ${s.messages[id]}` : msg;
      (s.messageKeys[id] ||= []).push(k);
      s.messageCounts[id] = (s.messageCounts[id] || 0) + 1;
    }
  }

  // ---------------------------------------------------------------- shown values

  /** A sample's blank boxes stay blank until typed; a period part shows what was typed into it. */
  private shown(values: Record<string, string>): Record<string, string> {
    const s = this.st;
    if (!s.sample || !s.blank.size) return values;
    const out = { ...values };
    for (const k of s.blank) {
      if (s.periodParts.has(k)) out[k] = s.periodValues[k] ?? "";
      else if (k in out && !s.typed.has(k)) out[k] = "";
    }
    return out;
  }

  /** The text box k shows for what the draft holds. */
  private shownText(k: string): string {
    const s = this.st;
    const one = this.oneDates.get(k);
    if (one) return shownDate(oneDateText(s.values, one.date));
    const pool = this.pools.get(k);
    if (pool) return String(s.poolTexts[pool.keys[0]] ?? "");
    const kind = this.kindOf.get(k);
    const raw = String(s.values[k] ?? "");
    if (kind === "money") {
      const zero = this.isShut(k) && (this.isComputedBox(k) || this.penaltyGroupOf(k) !== null);
      const v = raw || (zero && !this.reasonFor(k) ? "0" : "");
      return v === "" ? "" : formatMoney(v, this.wholeKeys.has(k));
    }
    if (kind === "date") return shownDate(raw);
    if (kind === "sel") return this.selShown(k).value;
    return raw;
  }

  /** A select's shown value and choices (old selectBox). */
  private selShown(k: string): { value: string; options: ChoiceOption[] } {
    const s = this.st;
    const f = this.fieldOf.get(k);
    const listed = (s.options[k] || f?.o || []).map(optionOf).filter((o): o is ChoiceOption => Boolean(o));
    const value = String(s.values[k] ?? "");
    const selected = listed.find((o) => o.selected);
    const known = listed.some((o) => o.value === value);
    const shown = selected ? selected.value : known ? value : /^(0+|-)$/.test(value) ? "" : value;
    const options = shown && !selected && !known ? [...listed, { value: shown, label: shown }] : listed;
    return { value: shown, options };
  }

  /** Every box shows what the draft holds (an opening, a rebuild, a restore). */
  private resetShown() {
    const pending = this.st.pending;
    for (const k of this.allKeys) if (!(k in pending)) this.sValue.get(k).set(this.shownText(k));
    for (const [k, { date }] of this.oneDates) {
      if ([date.month.k, date.day?.k, date.year.k].some((p) => p && p in pending)) continue;
      this.sValue.get(k).set(this.shownText(k));
    }
  }

  /** Shut boxes follow the draft; a box the user can type keeps its text unless the host changed it (old syncChanged). */
  private syncShown(before: Record<string, string>, edits: Record<string, string>) {
    const s = this.st;
    for (const k of this.allKeys) {
      if (k in edits) {
        // A box the host changed to something other than what was typed (a schedule row moved up into a cleared
        // row) shows the host's value, unless typed again since.
        if (!(k in s.pending) && !this.pools.has(k) && this.hostChanged(k, edits[k])) this.sValue.get(k).set(this.shownText(k));
        continue;
      }
      if (this.isShut(k) || this.pools.has(k) || (before[k] ?? "") !== (s.values[k] ?? "")) this.sValue.get(k).set(this.shownText(k));
    }
    for (const [k, { date }] of this.oneDates) {
      const parts = [date.month.k, date.day?.k, date.year.k].filter(Boolean) as string[];
      if (parts.some((p) => p in edits)) continue;
      if (oneDateText(before, date) !== oneDateText(s.values, date)) this.sValue.get(k).set(this.shownText(k));
    }
  }

  /** The draft holds something else for box k than the text typed into it. */
  private hostChanged(k: string, typed: string): boolean {
    const host = String(this.st.values[k] ?? "");
    const kind = this.kindOf.get(k);
    if (kind === "money" || kind === "pct" || kind === "int") return !sameNumber(typed, host);
    if (kind === "sel" || kind === "chk" || kind === "radio") return true;
    if (kind === "date") {
      const a = parseTypedDate(typed);
      const b = parseTypedDate(host);
      if (a && b) return formatMDY(a) !== formatMDY(b);
    }
    return typed.trim().toLowerCase() !== host.trim().toLowerCase();
  }

  /** Computed and locked boxes follow the draft (after a validation). */
  private syncComputed() {
    for (const k of this.allKeys) if (this.isShut(k)) this.sValue.get(k).set(this.shownText(k));
  }

  // ---------------------------------------------------------------- locks

  private isComputedBox(k: string): boolean {
    return this.computedKeys.has(k) || /^computed/.test(this.st.locks[k] || "");
  }

  private penaltyGroupOf(k: string): ScreenFormPenalties | null {
    return (this.st.penalties || []).find((g) => g.keys.includes(k)) || null;
  }

  /** Locked on the page: off, a penalty not opened, or a host lock unless a profile box opened with Edit. */
  private isLocked(k: string): boolean {
    const s = this.st;
    if (s.off[k]) return true;
    const penalty = this.penaltyGroupOf(k);
    if (penalty) return !penalty.applies || !(penalty.edited || s.penaltyOpen.has(k));
    return s.locks[k] !== undefined && !(s.profileEdit.has(k) && s.editing.has(k));
  }

  /** The box takes no typing at all. */
  private isShut(k: string): boolean {
    const one = this.oneDates.get(k);
    if (one) {
      const parts = [one.date.month, one.date.day, one.date.year].filter(Boolean) as SchemaField[];
      return !this.st.editable || parts.every((f) => f.c || this.isLocked(f.k));
    }
    return !this.st.editable || this.computedKeys.has(k) || this.isLocked(k);
  }

  /** What a locked blank box shows in its place. */
  private reasonFor(k: string): string {
    const s = this.st;
    if (s.blank.has(k) && s.needs[k]) return s.needs[k];
    return s.whys[k] || s.waits[k] || "";
  }

  /** Boxes of an edit batch the page has open (screen_form_fill's unlocked). */
  private unlockedFor(edits: Record<string, unknown>): string[] {
    const s = this.st;
    return Object.keys(edits).filter((k) => s.locks[k] === undefined || s.opened.has(k) || s.penaltyOpen.has(k));
  }

  private accepted(edits: Record<string, string>, rejected: Record<string, string> | undefined) {
    const s = this.st;
    for (const [k, v] of Object.entries(edits)) {
      if (rejected && k in rejected) continue;
      s.typed.add(k);
      if (s.sample) s.typedEdits[k] = v;
    }
  }

  /** Why an item's shut boxes show blank, in the filer's words (old lockNotesFor). */
  private lockNotesFor(it: SchemaItem): string[] {
    const s = this.st;
    if (!s.editable || !s.draft) return [];
    const notes: string[] = [];
    const add = (text: string) => {
      if (text && !notes.includes(text)) notes.push(text);
    };
    const note = (k: string) => add(s.lockNotes[k] || (this.computedKeys.has(k) ? "Computed by the form." : ""));
    const shut = (k: string, c?: 1) => Boolean(c) || this.isLocked(k);
    const waits = (k: string) => {
      if (s.off[k] && this.isComputedBox(k)) add("Computed by the form.");
      return Boolean(this.reasonFor(k) || s.off[k]);
    };
    const always = (k: string) => s.notesAlways.has(k);
    const blankText = (k: string, t: string) => {
      const v = String(s.values[k] ?? "").trim();
      if (t === "money" && this.isComputedBox(k)) return false;
      if (t === "sel") return v === "" || /^(0+|-)$/.test(v);
      return v === "";
    };
    if (it.rows) {
      (it.keys || []).forEach((row) =>
        row.forEach((k, c) => {
          const col = (it.cols || [])[c];
          if (!col || col.t === "hidden" || !shut(k, col.c) || waits(k)) return;
          if (always(k) || (col.t === "radio" || col.t === "chk" ? !ticked(s.values[k]) : blankText(k, col.t))) note(k);
        }),
      );
      return notes;
    }
    const fs = it.f || [];
    const radios = fs.filter((f) => f.t === "radio");
    if (radios.length && radios.every((f) => shut(f.k, f.c)) && !radios.some((f) => ticked(s.values[f.k])) && !radios.some((f) => waits(f.k))) note(radios[0].k);
    for (const f of fs) {
      if (f.t === "radio" || !shut(f.k, f.c) || waits(f.k)) continue;
      if (always(f.k) || (f.t === "chk" ? !ticked(s.values[f.k]) : blankText(f.k, f.t))) note(f.k);
    }
    return notes;
  }

  // ---------------------------------------------------------------- profile boxes

  private itemEditKeys(k: string): string[] {
    const id = this.keyIndex.get(k);
    if (!id) return [k];
    return (this.items.get(id)!.f || []).map((f) => f.k).filter((key) => this.st.profileEdit.has(key));
  }

  /** Edit on a profile item: its profile boxes take typing; the edit stays on this return. */
  startProfileEdit(id: string) {
    const s = this.st;
    const it = this.items.get(id);
    if (!s.editable || !it) return;
    const keys = (it.f || []).map((f) => f.k).filter((k) => s.profileEdit.has(k));
    this.endProfileEdit([...s.editing].filter((k) => this.keyIndex.get(k) !== id));
    if (it.l === "Additional details") s.moreOpen.add(id);
    for (const k of keys) {
      s.editing.add(k);
      s.opened.add(k);
    }
    this.publish();
    const first = keys.find((k) => this.kindOf.get(k) !== "radio");
    if (first) this.focusers.get(first)?.();
  }

  /** Back to the profile look; the value stays on this return. */
  endProfileEdit(keys: string[]): boolean {
    let changed = false;
    for (const k of keys) changed = this.st.editing.delete(k) || changed;
    return changed;
  }

  /** A part's items the profile filled whole: they fold into one "From tax profile" group. */
  private tuckedItems(pi: number): number[] {
    const s = this.st;
    const part = this.schema?.parts[pi];
    if (!part || !s.profile.size) return [];
    const out: number[] = [];
    part.items.forEach((it, ii) => {
      if (it.rows || it.l === "Additional details") return;
      const typed = (it.f || []).filter((f) => !f.c);
      if (typed.length && typed.every((f) => s.profile.has(f.k))) out.push(ii);
    });
    return out;
  }

  /** Fold or unfold a part's "From tax profile" group. */
  toggleProfileFold(pi: number) {
    const s = this.st;
    if (s.profileOpen.has(pi)) s.profileOpen.delete(pi);
    else s.profileOpen.add(pi);
    this.publish();
  }

  /** "Save to profile too" on a profile item: on, the next save writes its boxes to the profile. */
  setToProfile(id: string, on: boolean) {
    const s = this.st;
    for (const k of this.saveableKeys(id)) {
      if (on) s.toProfile.add(k);
      else s.toProfile.delete(k);
    }
    if (on) {
      s.dirty = true;
      this.scheduleAutosave();
    }
    this.publish();
  }

  private saveableKeys(id: string): string[] {
    const s = this.st;
    const it = this.items.get(id);
    if (!it || !s.editable || s.sample) return [];
    return (it.f || []).map((f) => f.k).filter((k) => s.profileEdit.has(k) && s.profileSave[k]);
  }

  /** After a save: the switched-on profile boxes go to the tax profile. What it says, or null. */
  private saveToProfile(): string | null {
    const s = this.st;
    const keys = [...s.toProfile].filter((k) => s.profileEdited.has(k));
    if (!keys.length) return null;
    let r;
    try {
      r = bir.screen_form_save_to_profile(s.target.formCode, s.target.year, s.target.period, s.draft!, keys);
      this.wrote("screen_form_save_to_profile");
    } catch (e) {
      this.toast(plainRefusal(errorText(e)), "triangle-alert");
      return null;
    }
    s.draft = r.draft;
    const before = s.values;
    s.values = this.shown({ ...s.values, ...(r.values || {}) });
    if (r.pools) s.poolTexts = r.pools;
    this.syncShown(before, {});
    for (const k of keys) {
      s.toProfile.delete(k);
      s.profileEdited.delete(k);
    }
    s.electionReady = undefined;
    return [`Saved to the tax profile too: ${(r.saved || []).join(", ")}.`, ...(r.notes || [])].join(" ");
  }

  /** "Use profile values" on a draft that differs from the profile. */
  useProfileValues() {
    const s = this.st;
    if (!s.draft || !s.profileDiffers.length) return;
    this.flush();
    if (!this.applyUseProfile()) return;
    s.profileDiffers = [];
    s.dirty = true;
    if (s.amends && !s.saved) return this.publish();
    this.saveSoon();
  }

  /** screen_form_use_profile, taken into the page; null when it failed (toasted). */
  private applyUseProfile(facts?: ScreenFormFact[]): { asks?: ScreenFormAsk[] } | null {
    const s = this.st;
    const before = s.values;
    try {
      const r = bir.screen_form_use_profile(s.target.formCode, s.target.year, s.target.period, s.draft!, facts);
      s.draft = r.draft;
      s.values = this.shown(r.values);
      s.options = (r.options || s.options) as Record<string, unknown[]>;
      if (r.pools) s.poolTexts = r.pools;
      if (Array.isArray(r.penalties)) s.penalties = r.penalties;
      this.syncShown(before, {});
      // A validation sent before this would bring back the pre-profile numbers: a new one replaces it.
      this.validate();
      return r;
    } catch (e) {
      this.toast(plainRefusal(errorText(e)), "triangle-alert");
      return null;
    }
  }

  // ---------------------------------------------------------------- penalties, from, notices, stops, more

  /** Edit penalties: the row's group opens together. */
  openPenalties(id: string) {
    const s = this.st;
    const g = this.rowPenalty(id);
    if (!s.editable || !g || !g.applies) return;
    for (const k of g.keys) s.penaltyOpen.add(k);
    this.publish();
  }

  /** Use the computed amounts: the group's overrides are cleared (a removal question may come first). */
  usePenaltyComputed(id: string) {
    const s = this.st;
    const g = this.rowPenalty(id);
    if (!g) return;
    if (g.edited || g.kept) this.choose(Object.fromEntries(g.keys.map((k) => [k, ""])));
    for (const k of g.keys) s.penaltyOpen.delete(k);
    this.publish();
  }

  private rowPenalty(id: string): ScreenFormPenalties | null {
    const it = this.items.get(id);
    if (!it) return null;
    return (it.f || []).map((f) => this.penaltyGroupOf(f.k)).find((g) => g && g.applies) || null;
  }

  /** "Use it" on an item's from hint: the whole-peso amount goes into its box as if typed. */
  useFrom(id: string) {
    const s = this.st;
    const from = this.items.get(id)?.from;
    if (!from || !s.editable || this.isShut(from.to)) return;
    const pesos = wholePesosOf(s.values[from.k]);
    if (!pesos) return;
    const shown = formatMoney(String(pesos.amount), true);
    this.sValue.get(from.to).set(shown);
    s.pending[from.to] = shown;
    s.dirty = true;
    this.unquiet([from.to]);
    this.flush();
  }

  /** Mark a notice checked (the host records it), then check again. */
  markChecked(token: string) {
    const s = this.st;
    if (!s.draft) return;
    try {
      bir.screen_form_check_notice(s.target.formCode, s.draft, token);
    } catch (e) {
      return this.toast(errorText(e), "triangle-alert");
    }
    this.validate();
  }

  private stopBoxes(it: SchemaItem): string[] {
    const s = this.st;
    if (!s.stoppable.size) return [];
    return keysOf(it).filter((k) => s.stoppable.has(k));
  }

  /** "Stop here" on row id: on or off for every box of it that may carry one. */
  setStop(id: string, on: boolean) {
    const s = this.st;
    const it = this.items.get(id);
    if (!it || !s.editable) return;
    for (const k of this.stopBoxes(it)) {
      const call = stopCall(k, on, s.autoStops);
      if ((s.stopEdits[k] ?? null) === call) continue;
      try {
        const r = bir.set_template_stop(s.target.tin, s.target.formCode, k, call);
        this.wrote("set_template_stop");
        if (r?.stop_edits && typeof r.stop_edits === "object") s.stopEdits = r.stop_edits;
      } catch (e) {
        return this.toast(errorText(e), "triangle-alert");
      }
    }
    s.stopKeys = effectiveStops(s.autoStops, s.stopEdits);
    if (on) s.passed.delete(id);
    this.publish();
  }

  /** "Additional details": its fields shown or folded. */
  toggleMore(id: string) {
    const s = this.st;
    if (s.moreOpen.has(id)) s.moreOpen.delete(id);
    else s.moreOpen.add(id);
    this.publish();
  }
  moreOpen(id: string): boolean {
    return this.st.moreOpen.has(id);
  }

  // ---------------------------------------------------------------- facts and the election

  private taxYear(): number {
    const s = this.st;
    return Number((s.draft as { taxable_year?: number } | null)?.taxable_year ?? s.target.year);
  }

  private taxpayerName(): string {
    const root = String(this.st.target.tin).slice(0, 9);
    try {
      const tp = bir.list_taxpayers().find((x) => x.tin_root === root);
      if (tp) return tp.name;
    } catch (_) {
      /* the TIN then */
    }
    return root;
  }

  private electionReadiness(): IncomeTaxElectionReadiness | null {
    const s = this.st;
    if (s.electionReady === undefined) {
      try {
        s.electionReady = bir.income_tax_election_readiness(s.target.tin, this.taxYear());
      } catch (_) {
        s.electionReady = null;
      }
    }
    return s.electionReady;
  }

  /** A choice card's answer picked. */
  pickAsk(fact: ScreenFormFact, value: unknown) {
    this.st.askPick[fact] = value;
    this.publish();
  }

  /** A date or text card's box (kept by the page's input; read on Save). */
  /** What was typed into a fact card so far (a card scrolled away and back is drawn again with it). */
  askTextOf(fact: ScreenFormFact): string {
    return this.st.askText[fact] ?? "";
  }

  typeAsk(fact: ScreenFormFact, text: string) {
    this.st.askText[fact] = text;
  }

  /** A fact card's Save: the answer goes to the tax profile, the draft takes it, the return is saved. */
  saveAsk(fact: ScreenFormFact) {
    const s = this.st;
    const ask = s.asks.find((a) => a.fact === fact);
    if (!s.draft || !s.editable || s.sample || !ask) return;
    let value: unknown;
    if (ask.control === "choice") {
      if (!(fact in s.askPick)) return;
      value = s.askPick[fact];
    } else {
      const raw = (s.askText[fact] ?? "").trim();
      if (ask.control === "date") {
        const date = parseTypedDate(raw);
        if (!date) return this.toast(DATE_ERROR, "triangle-alert");
        value = `${date.y}-${String(date.m).padStart(2, "0")}-${String(date.d).padStart(2, "0")}`;
      } else {
        if (!raw) return this.toast(`Type the ${ask.label.toLowerCase()} first.`, "triangle-alert");
        value = raw;
      }
    }
    const at = [...s.askAt].find(([, asks]) => asks.includes(ask))?.[0] ?? null;
    if (Object.keys(s.pending).length) this.flush();
    try {
      if (fact === "birth_date") {
        bir.update_taxpayer(String(s.target.tin).slice(0, 9), { birth_date: value } as never);
        this.wrote("update_taxpayer");
      } else {
        bir.set_year_facts(s.target.tin, this.taxYear(), { [fact]: value } as never);
        this.wrote("set_year_facts");
      }
    } catch (e) {
      return this.toast(plainRefusal(errorText(e)), "triangle-alert");
    }
    s.electionReady = undefined;
    const r = this.applyUseProfile([fact]);
    this.takeAsks(r && Array.isArray(r.asks) ? r.asks : s.asks.filter((a) => a !== ask));
    delete s.askPick[fact];
    s.factErrors = s.factErrors.filter((e) => e.field !== `tax_profile.${fact}`);
    this.setErrors(s.errors.filter((e) => !String(e.field).startsWith("tax_profile.")));
    if (at) s.askSavedAt.set(at, `${ask.label} saved to the tax profile. Later returns use it.`);
    s.dirty = true;
    if (s.amends && !s.saved) this.flush();
    else this.saveSoon();
    this.publish();
    this.toast(`Saved to ${this.taxpayerName()}'s tax profile.`, "circle-check");
  }

  /** Item 13's answer picked. */
  pickElection(value: "graduated" | "eight_percent") {
    this.st.electionPick = value;
    this.publish();
  }

  /** Item 13's Save: on the tax profile for the year, the draft takes it, the return is saved and checked. */
  recordElection() {
    const s = this.st;
    const pick = s.electionPick as "graduated" | "eight_percent" | null;
    if (!s.draft || !s.editable || !pick) return;
    if (this.electionReadiness()?.gap !== "ready") return this.publish();
    const year = this.taxYear();
    const at = s.electionAt;
    try {
      bir.record_income_tax_election(s.target.tin, year, pick);
      this.wrote("record_income_tax_election");
    } catch (e) {
      s.electionReady = undefined;
      this.publish();
      return this.toast(plainRefusal(errorText(e)), "triangle-alert");
    }
    s.electionReady = undefined;
    if (!this.applyUseProfile()) return this.validate();
    const radios = ELECTION_RADIOS[s.target.formCode];
    if (radios && (s.draft as { item_13_election?: string }).item_13_election === "unanswered") {
      for (const [choice, k] of Object.entries(radios)) {
        const v = choice === pick ? "true" : "false";
        s.pending[k] = v;
        s.values[k] = v;
        this.sValue.get(k).set(v);
      }
    }
    s.dirty = true;
    s.electionPick = null;
    s.electionSavedAt = at;
    this.flush();
    if (!(s.sample || (s.amends && !s.saved))) this.saveSoon();
    this.toast(`Saved to ${this.taxpayerName()}'s tax profile for ${year}.`, "circle-check");
  }

  /** Item 13's "Update from tax profile": the return follows the profile now. */
  followProfile() {
    const s = this.st;
    if (!s.draft || !s.editable) return;
    this.flush();
    if (!this.applyUseProfile()) return;
    s.profileDiffers = [];
    s.profileRefreshed = true;
    s.dirty = true;
    const item13 = (s.draft as { item_13_election?: string }).item_13_election === "unanswered" ? " Item 13 still needs an answer: choose Not applicable." : "";
    if (s.sample || (s.amends && !s.saved)) this.flush();
    else this.saveSoon();
    this.publish();
    this.toast(`This return now follows ${this.taxpayerName()}'s tax profile.${item13}`, "circle-check");
  }

  /** The election card's action. */
  electionAction(id: "fp-election-retry" | "fp-election-follow") {
    if (id === "fp-election-follow") return this.followProfile();
    this.st.electionReady = undefined;
    this.publish();
  }

  private electionCard(): ElectionCard | null {
    const s = this.st;
    const ready = this.electionReadiness();
    const name = this.taxpayerName();
    const year = this.taxYear();
    if (ready?.gap === "ready") return { kind: "ask", question: `Which income-tax rate did ${name} choose for ${year}?`, pick: s.electionPick };
    const type = (t: string | null | undefined) => (t ? t.toLowerCase() : "non-individual");
    const draftType = (s.draft as { taxpayer_type?: string } | null)?.taxpayer_type;
    if (!ready) {
      return { kind: "blocked", title: `Buwiz could not read ${name}'s tax profile for ${year}.`, body: "Try again in a moment. The rate can be chosen once the profile is read.", action: { id: "fp-election-retry", label: "Try again" } };
    }
    const r = ready as IncomeTaxElectionReadiness & { classification?: string | null };
    if (r.gap === "not_business_income" && !r.classification && s.asks.some((a) => a.fact === "tax_classification")) return null;
    if (r.gap === "not_business_income") {
      return {
        kind: "blocked",
        title: r.classification === "PurelyCompensation" ? `${name}'s tax profile has ${name} earning compensation income only in ${year}.` : `${name}'s tax profile does not say whether ${name} is self-employed or has mixed income in ${year}.`,
        body: "The income-tax rate is chosen by self-employed and mixed-income earners. If that is wrong, change the income type on the tax profile; then come back here to choose the rate.",
        action: null,
      };
    }
    if (draftType === "Individual" && !s.profileRefreshed) {
      const t = type(r.taxpayer_type);
      return {
        kind: "blocked",
        title: `This return still treats ${name} as an individual, but the tax profile registers ${name} as a ${t} in ${year}.`,
        body: `Only individuals choose this income-tax rate. Update this return from the tax profile: it then follows the ${t} registration, and the rate question goes away.`,
        action: { id: "fp-election-follow", label: "Update from tax profile" },
      };
    }
    if (draftType === "Individual") {
      return {
        kind: "blocked",
        title: `This quarter of ${name}'s tax profile is still an individual's registration, while ${year}'s latest is a ${type(r.taxpayer_type)}'s.`,
        body: "Check the registration dates on the tax profile. Until they agree, this return asks for the income-tax rate and the profile cannot record one.",
        action: null,
      };
    }
    return {
      kind: "blocked",
      title: `${name}'s tax profile is registered as ${r.taxpayer_type ? `a ${r.taxpayer_type}` : "a non-individual"} in ${year}.`,
      body: "Only individuals choose this income-tax rate. If that registration is wrong, correct it on the tax profile.",
      action: null,
    };
  }

  // ---------------------------------------------------------------- parts and stepping

  togglePart(pi: number) {
    const s = this.st;
    if (s.openParts.has(pi)) s.openParts.delete(pi);
    else {
      s.openParts.add(pi);
      s.current = pi;
    }
    this.publish();
  }

  /** Expand all, or Collapse all when every part is open. */
  toggleAllParts() {
    const s = this.st;
    if (!this.schema) return;
    if (this.schema.parts.every((_p, i) => s.openParts.has(i))) s.openParts = new Set();
    else s.openParts = new Set(this.schema.parts.map((_p, i) => i));
    this.publish();
  }

  /** Navigator and pill: open the chosen part and fold the others. row: the row a step goes to. */
  jumpTo(pi: number, row: string | null = null) {
    const s = this.st;
    s.openParts = new Set([pi]);
    s.current = pi;
    s.stepAt = row;
    this.publish();
    this.sReveal.set({ row, part: pi, seq: ++this.revealSeq });
  }

  /** The part the scroll shows now (the page reports it; the pill names it). */
  setCurrent(pi: number) {
    if (this.st.current === pi) return;
    this.st.current = pi;
    this.sCurrent.set(pi);
  }

  /** The next part with a shown message after the open one (the navigator's "next ↓"). */
  nextNeedingPart(): number | null {
    const s = this.st;
    if (!this.schema) return null;
    const needing = this.schema.parts.map((_p, i) => i).filter((i) => this.partErrors(i) > 0);
    if (!needing.length) return null;
    const openOne = s.openParts.size === 1 ? [...s.openParts][0] : -1;
    return needing.find((i) => i > openOne) ?? needing[0];
  }

  /** Where ‹ (-1) or › (1) goes: a to-do row, Check past the last, or the previous/next part when nothing is to do. */
  stepTarget(dir: number): { part: number; row: string | null } | { check: true } | null {
    const s = this.st;
    if (!this.schema) return null;
    const ids = this.todoNow().map((r) => r.id);
    if (ids.length) {
      const row = nextRow(ids, dir, s.stepAt, s.current, s.stepFromTop);
      if (row) return { part: Number(row.split(":")[0]), row };
      // Past the last row › ran Check: ‹ steps back from the end, to the last row to do.
      if (dir < 0 && !s.stepAt && s.stepFromTop) {
        const last = ids[ids.length - 1];
        return { part: Number(last.split(":")[0]), row: last };
      }
      return dir > 0 ? { check: true } : null;
    }
    const part = s.current + dir;
    if (part >= 0 && part < this.schema.parts.length) return { part, row: null };
    // Nothing shows yet but the host has errors: past the last part, Check shows them.
    return dir > 0 && s.editable && s.errors.length > 0 ? { check: true } : null;
  }

  /** ‹ and › (Cmd/Ctrl+Left/Right): a row step opens its part and asks the page to reveal and focus the row. */
  step(dir: number) {
    const s = this.st;
    const target = this.stepTarget(dir);
    if (!target) return;
    if ("check" in target) {
      if (s.stepAt) s.passed.add(s.stepAt);
      s.stepAt = null;
      s.stepFromTop = true;
      this.publish();
      this.check();
      return;
    }
    if (s.stepAt && dir > 0) s.passed.add(s.stepAt);
    s.stepFromTop = false;
    // The target's part opens; the parts already open stay open (after Expand all a step folds nothing).
    s.openParts.add(target.part);
    s.current = target.part;
    s.stepAt = target.row;
    this.publish();
    this.sReveal.set({ row: target.row, part: target.part, seq: ++this.revealSeq });
  }

  /** The keyboard into a row's box: its first stop box, else its first empty required box, else its first open box. */
  focusRow(row: string): boolean {
    const s = this.st;
    const it = this.items.get(row);
    if (!it || it.rows || !s.editable) return false;
    const fs = (it.f || []).filter((f) => !f.c && !this.isLocked(f.k));
    const entry = this.todoNow().find((r) => r.id === row);
    const empty = (f: SchemaField) => (f.t === "radio" || f.t === "chk" ? s.values[f.k] !== "true" : !String(s.values[f.k] ?? "").trim());
    const pick = fs.find((f) => entry?.stops.includes(f.k)) || fs.find((f) => f.r && empty(f)) || fs[0];
    if (!pick) return false;
    const date = oneDate(it);
    const key = date ? date.oneKey : pick.t === "radio" ? `radio:${row}` : pick.k;
    const focus = this.focusers.get(key);
    if (!focus) return false;
    focus();
    return true;
  }

  /** Messages as they show: a row's after it was touched, every one after Check. */
  private shownMessages(): Record<string, string> {
    const s = this.st;
    const out: Record<string, string> = {};
    for (const id of new Set([...Object.keys(s.messages), ...Object.keys(s.dateErrors)])) {
      const msg = s.dateErrors[id] || s.messages[id] || "";
      if (msg && (s.showAll || s.touched.has(id))) out[id] = msg;
    }
    return out;
  }

  /** How many messages each row with a shown message carries (a date's own sentence counts once). */
  private shownCounts(): Record<string, number> {
    const s = this.st;
    const out: Record<string, number> = {};
    for (const id of Object.keys(this.shownMessages())) out[id] = Math.max(s.dateErrors[id] ? 1 : 0, s.messageCounts[id] || 0, 1);
    return out;
  }

  /** How many rows of part pi hold a validation message, shown or not (the old partErrors: the badges, "n parts need attention"). */
  private partErrors(pi: number): number {
    return Object.keys(this.st.messages).filter((id) => id.startsWith(`${pi}:`)).length;
  }

  /** The to-do list (old todo()): every row with a message counts, whether its text shows yet or not. */
  private todoNow(): TodoRow[] {
    const s = this.st;
    if (!this.schema || !s.draft) return [];
    return todoRows({
      parts: this.schema.parts as unknown as TodoPart[],
      messages: s.messages,
      values: s.values,
      stops: s.editable ? s.stopKeys : new Set(),
      passed: s.passed,
      blocked: (k) => !s.editable || this.isLocked(k),
    });
  }

  // ---------------------------------------------------------------- check, save, queue, print

  /** Check: every message shows; a toast says how many items need attention once validation is back. */
  check() {
    this.looked((n) => this.toast(n ? attentionText(n) : "No problems found.", n ? "triangle-alert" : "circle-check"));
  }

  /** Check's look: every message shows, and once validation is back `done` hears how many items need attention. */
  private looked(done: (n: number) => void) {
    const s = this.st;
    this.unquiet();
    s.showAll = true;
    this.flush();
    const token = this.token;
    const wait = () => {
      if (token !== this.token || this.disposed) return;
      if (this.st.checking) return void this.later(100, wait);
      // Every message counted once, as the pill and the part badges count them (not the rows they sit on).
      done(Object.values(this.shownCounts()).reduce((a, b) => a + b, 0));
    };
    this.later(50, wait);
  }

  private cancelAutosave() {
    if (this.saveTimer) {
      clearTimeout(this.saveTimer);
      this.saveTimer = null;
    }
    // Never left saying "Saving…" for a save that will not run.
    if (this.st.saveState.state === "saving") this.st.saveState = this.restingSaveState();
  }

  /** After a successful fill: AUTOSAVE_DELAY of quiet saves (when this return may be autosaved). */
  private scheduleAutosave() {
    this.cancelAutosave();
    if (!this.canAutosave() || this.removalUndo) {
      if (this.st.saveState.state !== "failed") this.st.saveState = this.restingSaveState();
      this.publish();
      return;
    }
    const token = this.token;
    this.saveTimer = this.later(AUTOSAVE_DELAY, () => {
      this.saveTimer = null;
      if (token !== this.token) return;
      this.autosave();
    });
  }

  /** Save at once (a fact, the election, Use profile values): the autosave path without the quiet. */
  private saveSoon() {
    this.cancelAutosave();
    if (!this.canAutosave()) return this.publish();
    this.autosave();
  }

  private autosave() {
    const s = this.st;
    if (!this.canAutosave()) return;
    // Typing came in meanwhile: its flush schedules the next save.
    if (Object.keys(s.pending).length) return;
    if (!s.dirty && s.saved) return;
    s.saveState = { state: "saving" };
    this.publish();
    const token = this.token;
    this.saveTimer = this.later(SAVING_PAINT, () => {
      this.saveTimer = null;
      if (token !== this.token || this.disposed) return;
      if (this.st.sample) this.saveSample(null);
      else this.saveReturn(false);
      this.publish();
    });
  }

  /** The explicit save: an amendment's first save, a sample's "Save as return" (asks the period when not set). */
  saveNow() {
    const s = this.st;
    if (!s.draft || !s.editable) return;
    this.cancelAutosave();
    if (s.sample) return this.saveSample(null);
    this.saveReturn(true);
    this.publish();
  }

  /** Save a return (old save()). announce: an explicit save toasts. True when saved. */
  private saveReturn(announce: boolean): boolean {
    const s = this.st;
    if (!s.draft) return false;
    this.unquiet();
    let edits = s.pending;
    s.pending = {};
    // A save of what was edited, or the first save of a return that was not stored yet (an amendment saved with nothing typed).
    const changed = s.dirty || Object.keys(edits).length > 0 || !s.saved;
    try {
      if (Object.keys(edits).length) {
        this.retireRemovalUndo();
        const f = this.fill(s.draft, edits);
        if (f.removes) {
          s.saveState = this.restingSaveState();
          this.holdRemoval(f, edits);
          return false;
        }
        this.applyFill(f, edits);
        edits = {};
      }
      const r = bir.screen_form_save(s.target.formCode, s.draft);
      this.wrote("screen_form_save");
      // Saved, the return has a new state: a removal's Undo would bring back a draft older than the stored one.
      this.retireRemovalUndo();
      if (r.draft) s.draft = r.draft;
      if (r.status) s.status = r.status;
      s.dirty = false;
      s.saved = true;
      s.saveState = { state: "saved" };
      if (changed) this.hooks.onSaved?.(s.target);
      const profiled = s.toProfile.size ? this.saveToProfile() : null;
      if (profiled) this.toast(`Draft saved. ${profiled}`, "save");
      else if (announce) this.toast("Draft saved.", "save");
      this.validate();
      return true;
    } catch (e) {
      s.pending = { ...edits, ...s.pending };
      s.saveState = { state: "failed", message: errorText(e) };
      // A refusal shows every message, so what blocks it is on screen.
      s.showAll = true;
      this.publish();
      return false;
    }
  }

  /**
   * Save on a Form library sample: it becomes the return for its period. A period still waiting for the host's answer
   * is asked first; a period that has a stored return is offered, never saved over; a period not set yet is asked
   * for in the period dialog. Saved, the session reopens as the return it became, then then() runs on it.
   */
  private saveSample(then: (() => void) | null) {
    const s = this.st;
    if (s.periodAsked && s.periodAnswered !== s.periodAsked && Object.keys(s.periodValues).length) {
      s.saveAfterAnswer = { then };
      if (s.periodInFlight !== s.periodAsked) {
        s.periodSeq++;
        this.askPeriod(s.periodAsked);
      }
      return;
    }
    if (s.existing) return this.offerExisting(s.existing);
    if (!s.periodSet) {
      s.saveState = this.restingSaveState();
      return this.askSamplePeriod("", then);
    }
    let edits = s.pending;
    s.pending = {};
    let r;
    try {
      if (Object.keys(edits).length) {
        const f = this.fill(s.draft!, edits);
        this.accepted(edits, f.rejected);
        s.draft = f.draft;
        s.values = this.shown(f.values);
        if (f.pools) s.poolTexts = f.pools;
        edits = {};
      }
      r = bir.screen_form_save(s.target.formCode, s.draft!);
      this.wrote("screen_form_save");
    } catch (e) {
      s.pending = { ...edits, ...s.pending };
      s.saveState = { state: "failed", message: errorText(e) };
      s.showAll = true;
      return this.publish();
    }
    if (!r.saved) {
      s.opens = (r.opens as St["opens"]) || null;
      s.saveState = this.restingSaveState();
      if (r.existing) return this.offerExisting(r.existing);
      return this.publish();
    }
    s.dirty = false;
    const opens = r.opens!;
    const target: FormTarget = { tin: s.target.tin, formCode: s.target.formCode, year: opens.year, period: opens.period, key: opens.key ?? null, mode: "return" };
    this.toast("Saved. The sample is now your return for this period.", "save");
    this.periodWaiting = true;
    // The page stays up while it reopens: what is typed meanwhile waits and goes into the return.
    void this.open(target, true).then((error) => {
      this.periodWaiting = false;
      if (error) return;
      if (!this.isDirty()) this.st.saveState = { state: "saved" };
      this.publish();
      this.hooks.onSaved?.(target);
      this.hooks.onRetarget?.(target);
      if (then) then();
    });
  }

  /** Saved first when anything changed; false when it could not be (its reason shows). */
  private ensureSaved(): boolean {
    const s = this.st;
    if (s.sample) {
      this.saveSample(null);
      return false;
    }
    if (!s.dirty && !Object.keys(s.pending).length && s.saved) return true;
    this.cancelAutosave();
    return this.saveReturn(false) && !this.st.dirty;
  }

  /** 1601C and 2551Q keep their own queue path, keyed by month or quarter. */
  private usesPeriodPath(): boolean {
    return this.st.target.formCode === "1601C" || this.st.target.formCode === "2551Q";
  }

  /** The session token of a Queue that is looking at the return (Check's look) before it asks, else -1: a second press waits for it. */
  private lookingAt = -1;

  /**
   * Queue: saved first; tax facts not set that change what is filed stop it; then it looks, as Check does, and a return the host will refuse
   * says so (the toast, every message on its row, the pill) and nothing is asked; the others are confirmed once (the tier warning) or queued.
   */
  queue() {
    const s = this.st;
    if (s.sample) return this.saveSample(() => this.queue());
    if (!this.ensureSaved()) return this.publish();
    let check: { blocks: ScreenFormError[]; warns: string[] } | null = null;
    try {
      check = bir.screen_form_queue_check(s.target.formCode, s.draft!);
    } catch (_) {
      /* Queue's own checks still refuse what must not go. */
    }
    if (check?.blocks?.length) {
      s.factErrors = check.blocks;
      s.showAll = true;
      this.setErrors(s.errors.filter((e) => !String(e.field).startsWith("tax_profile.")));
      this.publish();
      return this.toast(check.blocks[0].message, "triangle-alert");
    }
    const warns = check?.warns ?? [];
    if (!this.looksFirst()) return this.askQueue(warns);
    if (this.lookingAt === this.token) return;
    this.lookingAt = this.token;
    this.looked((n) => {
      this.lookingAt = -1;
      // What Check lists is the page's reading, not the host's verdict, so it is said once. A second Queue with the same list unchanged goes
      // to the host, which decides (it can accept what Check still lists, as 2551Q's does): nothing the page thinks keeps a return from
      // being queued.
      const listed = n > 0 ? this.refusalList() : "";
      if (listed && listed !== this.toldRefusal) {
        this.toldRefusal = listed;
        return void this.toast(attentionText(n), "triangle-alert");
      }
      this.toldRefusal = "";
      this.askQueue(warns);
    });
  }

  /** The list a Queue was last told of instead of being asked ("" when none): the same list again goes to the host. */
  private toldRefusal = "";

  /**
   * Whether Queue looks before it asks. 2551Q's Queue does not judge the draft the page holds: it reads the stored return again and reconciles
   * it with the year's tax profile first (queue_form_draft), so the Item 13 election can be answered there while Check still lists it (a
   * Queue the host accepted, with Check listing it, was measured). For 2551Q what Check lists is not what the host refuses: Queue still
   * asks, and the host decides. The others judge the saved draft the page validated (1601C's is read back from the row just saved).
   */
  private looksFirst(): boolean {
    return this.st.target.formCode !== "2551Q";
  }

  /**
   * The messages of the list the host's Queue refuses for, as one text ("" when none): its validation's, not a page notice (dismissible or
   * listed in schema.ts) and not a page-only message.
   */
  private refusalList(): string {
    const s = this.st;
    return s.errors
      .filter((e) => !e.dismiss && !isPageNotice(s.target.formCode, e.field))
      .map((e) => `${e.field}: ${e.message}`)
      .sort()
      .join("\n");
  }

  /** Nothing left to fix: the tier warning is asked once, else the return is queued. */
  private askQueue(warns: string[]) {
    const s = this.st;
    if (warns.length) {
      s.queueWarn = { warns };
      s.overlay = { kind: "queue-confirm", warns };
      return this.publish();
    }
    this.queueNow();
  }

  confirmQueue() {
    const s = this.st;
    if (!s.queueWarn) return;
    s.queueWarn = null;
    s.overlay = null;
    this.publish();
    this.queueNow();
  }

  cancelQueueConfirm() {
    this.st.queueWarn = null;
    this.st.overlay = null;
    this.publish();
  }

  private queueNow() {
    const s = this.st;
    try {
      if (this.usesPeriodPath()) bir.queue_form_draft(s.target.tin, s.target.formCode, s.target.year, s.target.period || null);
      else bir.screen_form_queue(s.target.formCode, s.draft!);
      this.wrote(this.usesPeriodPath() ? "queue_form_draft" : "screen_form_queue");
      this.toast("Queued for background submission.", "send");
      this.hooks.onQueueChanged?.(s.target);
      void this.reload();
    } catch (e) {
      // Refused: every message shows, so what blocks it is on screen. The host lists what it refused as "field: message; ..." (its own
      // names for the boxes): the rows and the pill carry the messages, and the toast counts them as Check does.
      s.showAll = true;
      this.publish();
      const listed = listedFields(errorText(e));
      this.toast(listed ? attentionText(listed) : errorText(e), "triangle-alert");
    }
  }

  cancelQueue() {
    const s = this.st;
    if (s.sample) return;
    const name = this.usesPeriodPath() ? "cancel_queued" : "screen_form_cancel_queue";
    try {
      if (this.usesPeriodPath()) bir.cancel_queued(s.target.tin, s.target.formCode, s.target.year, s.target.period || null);
      else bir.screen_form_cancel_queue(s.target.formCode, s.draft!);
      this.wrote(name);
      this.toast("Queue cancelled. The return is an editable draft again.", "undo-2");
      this.hooks.onQueueChanged?.(s.target);
      void this.reload();
    } catch (e) {
      // Refused: the worker has most likely sent it already (the frame's Cancel queue says it in the same sentence). Either way the
      // page and the lists read the return again, so neither goes on saying Queued.
      this.toast(cancelQueueMessage(e), "triangle-alert");
      this.wrote(name);
      void this.reload();
    }
  }

  /** Reopen the return as stored (after queueing or cancelling). keep: the page stays drawn while the host answers (a read nobody asked for). */
  reload(keep = false): Promise<string | null> {
    const t = this.st.target;
    return this.open({ ...t, mode: t.mode === "amend" ? "return" : t.mode }, keep);
  }

  /** Whether the page shows a stored return as Queued and read-only: what the submission worker moves on behind the page's back (reactive). */
  queuedShown(): boolean {
    const h = this.header();
    return h.status === "Queued" && !h.loading && !h.error && !h.sample && !h.editable;
  }

  /** Whether id (a row of the frame's returns lists, bir.list_returns) is this page's return: both name the same stored draft. */
  isReturn(id: number): boolean {
    const s = this.st;
    return !s.sample && s.target.mode === "return" && (s.draft as { id?: number } | null)?.id === id;
  }

  /**
   * The frame's lists say the submission worker has moved this Queued return on (it tells nobody): read it again, with the page staying
   * drawn, so it shows what is stored and stops offering Cancel queue. Never over typing, over a read that is under way, or on a page that
   * does not show a Queued, read-only return.
   */
  refreshQueued(): void {
    const s = this.st;
    if (this.disposed || s.loading || this.reopening || s.sample || s.editable || s.status !== "Queued" || this.isDirty()) return;
    void this.reload(true);
  }

  /**
   * Print preview: of the saved return (saved first), an unsaved amendment from the page, a sample as it stands. A device with
   * no second window (a phone) says so, once: nothing is saved, asked of the host or claimed opened (the preview host would
   * say the same sentence a second time, beside "opened ... follows your edits live").
   */
  print() {
    const s = this.st;
    if (!s.draft) return;
    if (!hasSecondWindow && !Print.previewsNatively()) return void this.toast(NO_PREVIEW, "printer");
    if (s.sample) {
      try {
        bir.screen_form_print_sample(s.target.formCode, s.draft, s.pending, this.unlockedFor(s.pending));
        s.previewOpen = true;
        if (hasSecondWindow) this.toast("Print preview opened. It follows your edits live.", "printer");
      } catch (e) {
        this.toast(errorText(e), "triangle-alert");
      }
      return;
    }
    if (s.amends && !s.saved) this.flush();
    else if (s.editable && !this.ensureSaved()) return;
    try {
      bir.screen_form_print(s.target.formCode, s.draft!);
      s.previewOpen = true;
      if (hasSecondWindow) this.toast("Print preview opened. It follows your edits live.", "printer");
    } catch (e) {
      this.toast(errorText(e), "triangle-alert");
    }
  }

  /** View an amendment's filed original (read-only); backToAmendment returns. */
  openFiledOriginal() {
    const s = this.st;
    if (!s.filedOriginal) return;
    // An unsaved amendment is never saved here: its first save reverts the filed return (the old page asked first).
    if (this.isDirty() && ((s.amends && !s.saved) || !this.ensureSaved())) return void this.toast("Save your edits first: viewing the filed original leaves this return.", "triangle-alert");
    s.back = { ...s.target };
    void this.open({ ...s.target, mode: "filed_original", snapshotId: s.filedOriginal.id });
  }

  backToAmendment() {
    const back = this.st.back;
    if (back) void this.open(back);
  }

  // ---------------------------------------------------------------- a sample's period

  /** A sample's period part changed: asked about PERIOD_ASK_DELAY after the last change. */
  private setPeriodPart(parts: Record<string, string>) {
    const s = this.st;
    s.periodValues = { ...s.periodValues, ...parts };
    for (const [k, v] of Object.entries(parts)) {
      s.values[k] = String(v);
      this.sValue.get(k).set(String(v));
    }
    const asked = JSON.stringify(s.periodValues);
    if (asked === s.periodAsked) return this.publish();
    s.periodAsked = asked;
    this.closeOffer();
    s.opens = null;
    s.existing = null;
    const token = this.token;
    const seq = ++s.periodSeq;
    this.periodWaiting = true;
    this.later(PERIOD_ASK_DELAY, () => {
      if (token !== this.token || seq !== this.st.periodSeq) return;
      this.askPeriod(asked);
    });
    this.publish();
  }

  private askPeriod(asked: string) {
    const s = this.st;
    const token = this.token;
    s.periodInFlight = asked;
    this.periodWaiting = false;
    bir.screen_form_sample_period(s.target.tin, s.target.formCode, JSON.parse(asked)).then(
      (r) => {
        if (s.periodInFlight === asked) s.periodInFlight = null;
        if (token !== this.token || this.disposed || asked !== this.st.periodAsked) return;
        s.periodAnswered = asked;
        this.takePeriod(r);
        if (s.saveAfterAnswer) {
          const { then } = s.saveAfterAnswer;
          s.saveAfterAnswer = null;
          if (this.st === s) this.afterPeriodForSave(then);
        }
      },
      (e) => {
        if (s.periodInFlight === asked) s.periodInFlight = null;
        if (token === this.token) this.toast(errorText(e), "triangle-alert");
      },
    );
  }

  /** The host's answer for a sample's typed period. */
  private takePeriod(r: ScreenFormSamplePeriod) {
    const s = this.st;
    s.periodComplete = Boolean(r.complete);
    s.opens = r.complete ? r.opens || null : null;
    s.existing = r.existing || null;
    const period = ++s.periodToken;
    this.closeOffer();
    const offer = () => {
      if (r.existing) this.offerExisting(r.existing);
    };
    if (r.sample) {
      this.rebuild(r.sample);
      offer();
      // Its period set, a sample saves as you type: what was typed before the period is saved now.
      if (!r.existing && Object.keys(s.typedEdits).length) {
        s.dirty = true;
        this.scheduleAutosave();
      }
    } else if (s.periodSet) this.toPlaceholder(period, offer);
    else offer();
    this.publish();
  }

  /** The placeholder sample again (the period not set), then then(). */
  private toPlaceholder(period: number, then: (() => void) | null) {
    const s = this.st;
    const token = this.token;
    bir.screen_form_sample(s.target.tin, s.target.formCode).then(
      (r) => {
        if (token !== this.token || this.disposed || period !== this.st.periodToken) return;
        this.rebuild(r);
        if (then) then();
        this.publish();
      },
      (e) => {
        if (token === this.token) this.toast(errorText(e), "triangle-alert");
      },
    );
  }

  /** Take a sample the host built, with what the filer typed outside the period applied again. */
  private rebuild(sample: ScreenFormOpened) {
    const s = this.st;
    const typed = s.typedEdits;
    const periodValues = s.periodValues;
    this.takeOpened(sample);
    if (!s.periodSet) s.periodValues = periodValues;
    s.typedEdits = typed;
    for (const k of Object.keys(typed)) s.typed.add(k);
    if (Object.keys(typed).length) {
      try {
        const f = this.fill(s.draft!, typed);
        s.draft = f.draft;
        s.values = this.shown(f.values);
        s.options = (f.options || {}) as Record<string, unknown[]>;
      } catch (e) {
        this.toast(errorText(e), "triangle-alert");
      }
    }
    if (!s.periodSet) for (const [k, v] of Object.entries(periodValues)) s.values[k] = v;
    s.saveState = this.restingSaveState();
    this.resetShown();
    this.validate();
  }

  private closeOffer() {
    if (this.st?.overlay?.kind === "existing") this.st.overlay = null;
  }

  /** A stored return already has the sample's period: the dialog offers what its status allows. */
  private offerExisting(existing: ScreenFormExisting) {
    const s = this.st;
    s.existing = existing;
    if (existing.status === "Draft" && this.fastLane.drafts) return this.takeExisting("load", true);
    s.overlay = { kind: "existing", existing };
    this.publish();
  }

  /** "Don't ask again for drafts" (while this page lives). */
  setFastLane(on: boolean) {
    this.fastLane = { drafts: on };
  }

  /** The filer's choice for a stored return of the sample's period; each can be undone once. */
  takeExisting(choice: "load" | "view" | "amend" | "keep", auto = false) {
    const s = this.st;
    const existing = s.existing;
    this.closeOffer();
    if (!s.sample || !existing || !s.opens) return this.publish();
    if (choice === "keep") {
      const before = this.snapshotPeriod();
      this.keepForm();
      return this.toast("Kept this form; its period is cleared.", "undo-2", { label: "Undo", onPress: () => this.restorePeriod(before, existing) });
    }
    const sample = s;
    const { year, period, key } = s.opens;
    const target: FormTarget = { tin: s.target.tin, formCode: s.target.formCode, year, period, key: key ?? null, mode: choice === "amend" ? "amend" : "return" };
    const what = {
      load: auto ? `Loaded your saved ${existing.label} draft (you chose not to be asked again)` : `Loaded your saved ${existing.label} draft`,
      view: `Opened ${existing.label} read-only`,
      amend: `Started an amended ${existing.label} return`,
    }[choice];
    void this.open(target).then((error) => {
      if (error) {
        this.restoreForm(sample, null);
        return this.toast(`Could not open ${existing.label}: ${error}`, "triangle-alert");
      }
      this.hooks.onRetarget?.(target);
      this.toast(`${what}.`, "undo-2", { label: "Undo", onPress: () => this.restoreForm(sample, existing) });
    });
  }

  /** Undo a Load, View or Amend: the sample comes back exactly as it was, with its offer open again. */
  private restoreForm(sample: St, offer: ScreenFormExisting | null) {
    if (this.st === sample) return;
    const s = this.st;
    if (this.isDirty()) return this.toast("Undo would lose edits you have not saved. Save them first, or leave this return.", "triangle-alert");
    const savedAmendment = Boolean(s.amends && s.saved);
    this.cancelAutosave();
    this.st = sample;
    this.token++;
    if (offer && sample.existing === offer) sample.overlay = { kind: "existing", existing: offer };
    this.resetShown();
    this.publish();
    this.hooks.onRetarget?.(sample.target);
    if (savedAmendment) this.toast("Back to the sample. The amended return was saved and stays saved.", "undo-2");
  }

  private snapshotPeriod() {
    const s = this.st;
    return {
      state: s,
      fields: {
        draft: s.draft, values: { ...s.values }, options: s.options, locks: s.locks, lockNotes: s.lockNotes,
        notesAlways: s.notesAlways, blank: s.blank, needs: s.needs, whys: s.whys, errors: s.errors, messages: s.messages,
        formErrors: s.formErrors, periodValues: { ...s.periodValues }, periodAsked: s.periodAsked, periodSet: s.periodSet,
        opens: s.opens, existing: s.existing,
      },
      shown: Object.fromEntries([...this.allKeys, ...this.oneDates.keys()].map((k) => [k, this.shownOf(k)])),
    };
  }

  private restorePeriod(snapshot: ReturnType<FormSession["snapshotPeriod"]>, offer: ScreenFormExisting) {
    const s = this.st;
    if (s !== snapshot.state) return;
    s.periodToken++;
    s.periodSeq++;
    Object.assign(s, snapshot.fields);
    for (const [k, v] of Object.entries(snapshot.shown)) this.sValue.get(k).set(v);
    if (s.existing === offer) s.overlay = { kind: "existing", existing: offer };
    this.publish();
  }

  /** Keep this form: the same sample, every period part blank again. */
  private keepForm() {
    const s = this.st;
    s.periodValues = {};
    s.periodAsked = JSON.stringify({});
    s.periodAnswered = null;
    s.periodComplete = false;
    s.periodSeq++;
    s.opens = null;
    s.existing = null;
    for (const k of s.periodParts.keys()) {
      s.values[k] = "";
      this.sValue.get(k).set("");
    }
    if (s.periodSet) this.toPlaceholder(++s.periodToken, null);
    this.publish();
  }

  /** The host answered the period Save waited for. */
  private afterPeriodForSave(then: (() => void) | null) {
    const s = this.st;
    if (s.existing) return;
    if (!s.periodComplete) return this.askSamplePeriod("Enter the whole period to save this sample.", then);
    this.saveSample(then);
  }

  /** The period dialog for a sample's Save. */
  private askSamplePeriod(error: string, then: (() => void) | null) {
    const s = this.st;
    this.periodThen = then;
    const parts = [...s.periodParts].map(([key, p]) => ({ key, part: p.part, quarter: p.quarter, value: String(s.periodValues[key] ?? s.values[key] ?? "") }));
    s.overlay = { kind: "sample-period", parts, error };
    this.publish();
  }

  closeSamplePeriod() {
    if (this.st.overlay?.kind === "sample-period") this.st.overlay = null;
    this.periodThen = null;
    this.publish();
  }

  /** The period dialog's Save (part key -> text; a quarter's tick "true"/"false"): asked at once, then Save goes on. */
  takeSamplePeriod(parts: Record<string, string>) {
    const s = this.st;
    const then = this.periodThen;
    this.closeSamplePeriod();
    this.setPeriodPart(parts);
    if (s.periodAnswered === s.periodAsked) return this.afterPeriodForSave(then);
    s.saveAfterAnswer = { then };
    s.periodSeq++;
    this.askPeriod(s.periodAsked!);
  }

  // ---------------------------------------------------------------- publishing

  /** Push the plain state into the signals; an equal value notifies nobody. */
  private publish() {
    if (this.disposed) return;
    batch(() => {
      this.publishBoxes();
      this.publishRows();
      this.publishPage();
    });
  }

  private publishBoxes() {
    const s = this.st;
    const shownMsgKeys = new Set<string>();
    for (const [id, keys] of Object.entries(s.messageKeys)) {
      if (!(s.showAll || s.touched.has(id))) continue;
      for (const k of keys) shownMsgKeys.add(k);
    }
    for (const k of this.allKeys) {
      const shut = this.isShut(k);
      this.sLocked.get(k).set(shut);
      this.sOff.get(k).set(s.off[k] || "");
      const kind = this.kindOf.get(k);
      const blank = kind === "chk" || kind === "radio" ? !ticked(s.values[k]) : !String(s.values[k] ?? "").trim();
      this.sHint.get(k).set(shut && blank ? this.reasonFor(k) : "");
      if (kind === "sel") this.sOptions.get(k).set(this.selShown(k).options);
      this.sInvalid.get(k).set(shownMsgKeys.has(k));
    }
    for (const [k, { date, row }] of this.oneDates) {
      const parts = [date.month, date.day, date.year].filter(Boolean) as SchemaField[];
      const shut = this.isShut(k);
      this.sLocked.get(k).set(shut);
      this.sHint.get(k).set(shut && !oneDateText(s.values, date) ? parts.map((f) => this.reasonFor(f.k)).find(Boolean) || "" : "");
      this.sInvalid.get(k).set(Boolean((s.showAll || s.touched.has(row)) && (s.dateErrors[row] || parts.some((f) => shownMsgKeys.has(f.k)))));
    }
  }

  private publishRows() {
    const s = this.st;
    if (!this.schema) return;
    const shown = this.shownMessages();
    const tucked = this.schema.parts.map((_p, pi) => (s.profileOpen.has(pi) ? [] : this.tuckedItems(pi)));
    for (const id of this.rowIds) {
      const [pi, ii] = id.split(":").map(Number);
      this.sRow.get(id).set(this.rowInfo(id, this.items.get(id)!, shown, tucked[pi].includes(ii)));
    }
  }

  private rowInfo(id: string, it: SchemaItem, shown: Record<string, string>, tucked: boolean): RowInfo {
    const s = this.st;
    const fs = it.f || [];
    const keys = keysOf(it);
    const tin = isTinItem(it);
    const editKeys = fs.filter((x) => s.profileEdit.has(x.k)).map((x) => x.k);
    const fromProfile = (tin && !it.typed_tin) || fs.some((x) => s.profile.has(x.k));
    const kept = keys.map((k) => s.kept[k]).filter(Boolean);
    const penalty = s.editable ? fs.map((f) => this.penaltyGroupOf(f.k)).find((g) => g && g.applies) || null : null;
    const penaltyOpen = Boolean(penalty) && (penalty!.edited || penalty!.keys.some((k) => s.penaltyOpen.has(k)));
    const stopBoxes = this.stopBoxes(it);
    const editingHere = editKeys.some((k) => s.editing.has(k));
    const saveable = this.saveableKeys(id);
    const toOn = saveable.some((k) => s.toProfile.has(k));
    const entry = saveable.length ? s.profileSave[saveable[0]] : null;
    let from = "";
    if (it.from && s.editable && !this.isShut(it.from.to)) {
      const pesos = wholePesosOf(s.values[it.from.k]);
      if (pesos) from = `${it.from.l}: ₱${formatMoney(String(pesos.amount), true)}${pesos.rounded ? " (rounded to whole pesos)" : ""}`;
    }
    return {
      message: shown[id] || "",
      notes: this.lockNotesFor(it),
      needs: [...new Set(keys.map((k) => s.off[k]).filter(Boolean))],
      kept: kept.includes("template") ? "template" : kept.includes("last") ? "last" : "",
      stepHere: s.stepAt === id,
      stoppable: s.editable && stopBoxes.length > 0,
      isStop: stopBoxes.some((k) => s.stopKeys.has(k)),
      fromProfile: fromProfile
        ? editKeys.length
          ? "From tax profile. Edit changes this return only; the profile stays as it is."
          : tin
            ? "From tax profile. Change it in the tax profile."
            : "From tax profile. It repeats a box above; use Edit there."
        : "",
      canEdit: editKeys.length > 0 && s.editable,
      editing: editingHere,
      toProfile:
        saveable.length && (editingHere || saveable.some((k) => s.profileEdited.has(k)))
          ? { on: toOn, note: toOn ? `Your next save writes it to the tax profile too. ${entry?.note || ""}`.trim() : "Off: the change stays on this return." }
          : null,
      penalty: penalty ? (penaltyOpen ? "computed" : "edit") : null,
      tucked,
      asks:
        s.editable && !s.sample
          ? (s.askAt.get(id) || []).map((a) => ({
              fact: a.fact,
              label: a.label,
              note: a.note,
              control: a.control,
              choices: a.choices || [],
              pick: a.fact in s.askPick ? JSON.stringify(s.askPick[a.fact]) : null,
            }))
          : [],
      askSaved: s.askSavedAt.get(id) || "",
      election: s.editable && s.electionAt === id ? this.electionCard() : null,
      electionSaved: s.electionSavedAt === id && s.electionAt !== id ? `Saved to the tax profile for ${this.taxYear()}. Later returns reuse it.` : "",
      checkable: s.editable ? s.checkable[id] || [] : [],
      from,
    };
  }

  private publishPage() {
    const s = this.st;
    const todo = this.todoNow();
    this.sTodo.set(todo);
    // The pill's red count (old drawPill): with rows to do, how many hold a message; else how many parts have one.
    const partsNeeding = this.schema ? this.schema.parts.filter((_p, i) => this.partErrors(i) > 0).length : 0;
    this.sAttention.set(todo.length ? todo.filter((r) => r.reason === "message").length : partsNeeding);
    if (this.schema) {
      this.schema.parts.forEach((p, pi) => {
        const tucked = this.tuckedItems(pi);
        const filled = tucked.reduce((n, ii) => {
          const fs = (p.items[ii].f || []).filter((f) => s.profile.has(f.k));
          return n + fs.filter((f) => f.t !== "radio").length + (fs.some((f) => f.t === "radio") ? 1 : 0);
        }, 0);
        this.sPart.get(String(pi)).set({
          errors: this.partErrors(pi),
          todo: todo.filter((r) => r.id.startsWith(`${pi}:`)).length,
          open: s.openParts.has(pi),
          fold: tucked.length
            ? {
                first: tucked[0],
                items: tucked,
                label: `From tax profile · ${filled} filled box${filled === 1 ? "" : "es"}`,
                flagged: tucked.filter((ii) => s.messages[`${pi}:${ii}`] || s.dateErrors[`${pi}:${ii}`]).length,
                open: s.profileOpen.has(pi),
              }
            : null,
        });
      });
      this.sAllOpen.set(this.schema.parts.every((_p, i) => s.openParts.has(i)));
    }
    const sent: string[] = [];
    if (s.submittedAt) sent.push(sentWords(s.submittedAt, s.submittedDryRun, s.submittedViaRelay));
    if (s.confirmedAt) sent.push(`confirmed ${shortIsoDate(s.confirmedAt)}`);
    this.sHeader.set({
      loading: s.loading,
      error: s.error,
      status: s.status,
      editable: s.editable,
      sample: s.sample,
      periodSet: s.periodSet,
      amends: Boolean(s.amends || s.filedOriginal),
      amendUnsaved: Boolean(s.amends && !s.saved && s.editable),
      filedOriginal: Boolean(s.filedOriginal && !s.filedOriginalView),
      filedOriginalView: s.filedOriginalView,
      submittedLine: !s.editable && !s.filedOriginalView && sent.length ? `${sent.join(", ")}. Nothing here can be changed.` : "",
      profileUpdates: s.profileUpdates,
      profileDiffers: s.editable ? s.profileDiffers : [],
      profileChanged: !s.editable ? s.profileChanged : [],
      notes: s.notes.map((n) => n.text),
      formErrors: s.formErrors,
    });
    this.sSave.set(s.saveState);
    this.sChecking.set(s.checking);
    this.sOverlay.set(s.overlay);
    this.sCurrent.set(s.current);
    this.sStepAt.set(s.stepAt);
  }

  // ---------------------------------------------------------------- plumbing

  private toast(message: string, icon?: string, action?: ToastAction): string | null {
    const id = this.hooks.toast?.(message, { icon, action });
    return typeof id === "string" ? id : null;
  }

  /** A write went through the door: the page around tells the frame's cached reads. */
  private wrote(name: string) {
    try {
      this.hooks.afterWrite?.(name);
    } catch (_) {
      /* the cache's trouble, not the save's */
    }
  }

  private later(ms: number, fn: () => void): ReturnType<typeof setTimeout> {
    const id = setTimeout(() => {
      this.timers.delete(id);
      if (!this.disposed) fn();
    }, ms);
    this.timers.add(id);
    return id;
  }

  /** Flush what is typed and save it now where autosave may. The reason it is not saved, or null. */
  private finish(): string | null {
    const s = this.st;
    if (!s.draft || !s.editable || s.loading || this.reopening) return null;
    this.cancelAutosave();
    if (Object.keys(s.pending).length) {
      const edits = s.pending;
      s.pending = {};
      try {
        const r = this.fill(s.draft, edits);
        if (r.removes) {
          s.pending = { ...edits, ...s.pending };
          return `Not saved: ${r.removes.message}`;
        }
        this.applyFill(r, edits);
        s.dirty = true;
      } catch (e) {
        s.pending = { ...edits, ...s.pending };
        return errorText(e);
      }
    }
    if (!s.dirty || s.sample || !this.canAutosave()) return null;
    if (this.saveReturn(false)) return null;
    return s.saveState.state === "failed" ? s.saveState.message : null;
  }

  /** Typing flushed and saved now where autosave may (the frame awaits it before quit or navigation). False when a save was refused (toasted). */
  async flushNow(): Promise<boolean> {
    if (this.disposed) return true;
    if (this.reopening || this.st.loading) await this.ready();
    if (this.reopening) await this.idle();
    const failed = this.finish();
    this.publish();
    if (failed) this.toast(failed, "triangle-alert");
    return !failed;
  }

  /** The page goes: what is typed is flushed and, where autosave may, saved (a refusal is toasted); Undo and timers stop. */
  dispose() {
    if (this.disposed) return;
    this.retireRemovalUndo();
    const failed = this.finish();
    if (failed) this.toast(failed, "triangle-alert");
    live.delete(this);
    this.disposed = true;
    for (const id of this.timers) clearTimeout(id);
    this.timers.clear();
    this.token++;
  }
}

/** Where a validation message goes: by key, then item number, then a TIN item, then label words, else the first item. */
export function bindError(e: ScreenFormError, items: { id: string; it: SchemaItem }[], idForKey: (k: string) => string | undefined): string | null {
  const words = (t: string) => String(t).toLowerCase().match(/[a-z]{4,}/g) || [];
  let id: string | null | undefined = null;
  for (const k of e.keys || []) {
    id = idForKey(k);
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
  return id || (items[0] && items[0].id) || null;
}
