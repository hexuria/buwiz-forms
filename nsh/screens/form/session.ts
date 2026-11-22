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
import { hasNativePreview, Print } from "../print/native";
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
  type ScreenFormProfilePrerequisite,
  type ScreenFormSamplePeriod,
  type ScreenFormStatus,
} from "../../bir";
import { errorText } from "../../lib/errors";
import { cancelQueueMessage } from "../../lib/queue_message";
import { effectiveStops, nextRow, stopCall, todoRows, type TodoPart, type TodoRow } from "../../lib/todo";
import { formatMoney, wholePesosOf } from "../../lib/money";
import { DATE_ERROR, formatIso, formatLongDate, formatMDY, isTwoDigitDate, oneDateText, parseTypedDate, shortIsoDate, shownDate } from "../../lib/dates";
import { dateRange } from "./dateRules";
import { sameAmount } from "../../ui/mask";
import type { BoxState, ChoiceOption, FormSchema, SchemaField, SchemaItem } from "./contract";
import { choiceLabel, dateParts, dateRanges, isPageNotice, isTinItem, keysOf, listedFields, oneDates, onePeriods, optionOf, plainRefusal, sentWords, withoutHidden, type DateParts, type DateRange, type OneDate, type OnePeriod } from "./schema";
import { parsePeriod, periodLong, periodText, PERIOD_ERROR, PERIOD_TWO_DIGIT, periodYear4 } from "./periodRules";
import { hostSentence, refusalItem, refusalLabel, refusalSentence, unfixableRefusal, type RefusalContext } from "./refusals";

/** The boxes that hold a number: emptied, the host holds zero in them where it holds no blank (see FormSession.fill). */
const NUMBER_KINDS = new Set(["money", "pct", "int"]);
/** What an emptied number box is staged as when the host refuses a blank: the host's cleared value. */
const CLEARED = "0";

/** What a fill answers, with the boxes an emptied number box was cleared in. */
type Filled = ScreenFormFilled & { cleared?: string[] };

/** What a device with no second window says for Print preview (screens/print/host.tsx and src/print.rs say the same). */
const NO_PREVIEW = "Print preview is not available on this device.";

/** What Check says of the items that need attention. */
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
  onOpenProfile?(section: string, year: number): void;
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
  choices: { value: unknown; label: string; short?: string }[];
  /** The picked choice's value as JSON, or null. */
  pick: string | null;
  /** Answered on this return already (F2): the chip reads "Saved to profile" and a re-pick saves again. */
  done: boolean;
}

export type ElectionCard =
  | { kind: "ask"; question: string; pick: string | null }
  | { kind: "blocked"; title: string; body: string; action: { id: "fp-election-retry" | "fp-election-follow"; label: string } | null };

/** Everything a row draws besides its boxes. Compared by JSON: an equal RowInfo notifies nobody. */
export interface RowInfo {
  /** The message the row shows now ("" until touched or Check). */
  message: string;
  /** The box-home sentences of this row's split date parts and TIN boxes, in box order (B3/B4). */
  boxMessages: { key: string; sentence: string }[];
  /** G1: every drawn box of the row is one another answer keeps shut — the whole row (number and title too) is not drawn. */
  hidden: boolean;
  /** G1: the row's drawn box keys that are gated away; the rest draw (a schedule's cells are never listed: its grid stays). */
  hiddenBoxes: string[];
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
  /** A TIN row whose boxes the profile holds locked draws its own "From the tax profile" line under them (PR 8); the row mark stays away then. */
  tinLine: boolean;
  /** Edit is offered (profile boxes the filer may open), and they are open now. */
  canEdit: boolean;
  editing: boolean;
  /** The "Save to profile?" answer's line under the box: saved (ok) or kept on this return only (subtle). */
  profileAsk: { text: string; ok: boolean } | null;
  /** Edit penalties / Use the computed amounts. */
  penalty: "edit" | "computed" | null;
  /** Folded into its part's "From tax profile" group right now. */
  tucked: boolean;
  /** The "Additional details" fold is open: the filer opened it, or a shown message, fact card or to-do step points inside (B10). */
  moreOpen: boolean;
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
  /** Items drawn now (G1: a row whose every box is gated away is not counted). */
  items: number;
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
  profilePrerequisites: ScreenFormProfilePrerequisite[];
}

/** The "Save to profile?" ask for one profile-backed field (Q1 P1b / Q1·a). */
export interface ProfileAsk {
  /** The profile field id (`profile_save`'s `field`): what `screen_form_profile_asked` remembers. */
  field: string;
  /** The field's label, for the title ("Save {label, lower case} …"). */
  label: string;
  /** The value as the box shows it, for the body. */
  value: string;
  /** The `profile_save` keys of this field, for `screen_form_save_to_profile`. */
  keys: string[];
  /** The taxpayer's name and the return's tax year, for the body. */
  name: string;
  year: number;
  /** The birth date and the name are the taxpayer's, not a year's: the body names no year. */
  personal: boolean;
}

export type Overlay =
  | null
  | { kind: "removal"; message: string }
  | { kind: "existing"; existing: ScreenFormExisting }
  | { kind: "queue-confirm"; warns: string[] }
  | { kind: "profile-ask"; ask: ProfileAsk }
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
  periodHeld: Record<string, string> | null;
  periodIncomplete: boolean;
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
  profilePrerequisites: ScreenFormProfilePrerequisite[];
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
  /** Fields whose "Save to profile?" answer is already remembered (host's `profile_asked`, plus this session's). */
  profileAsked: Set<string>;
  profileAskUnknown: boolean;
  guessed: Set<string>;
  /** The shown-value signature last asked per field: a dismissed ask re-asks only on a changed value. */
  profileAskSeen: Record<string, string>;
  /** rowId → the answer's line under the box ("Saved to the tax profile…" / "Kept on this return only…"). */
  profileAskMsg: Record<string, { text: string; ok: boolean }>;
  /** Facts answered on this session's fact cards (F2): their cards stay, marked saved. */
  askDone: Set<ScreenFormFact>;
  askDoneRaw: Map<ScreenFormFact, ScreenFormAsk>;
  profileChanged: string[];
  errors: ScreenFormError[];
  messages: Record<string, string>;
  /** Keys each row's messages name (a red border on those boxes). */
  messageKeys: Record<string, string[]>;
  /** How many messages each row carries (host errors bound to it and refused edits): what the counts add up. */
  messageCounts: Record<string, number>;
  /** Boxes the host refused at the last fill, with its message (kept until the box is edited again). */
  rejected: Record<string, string>;
  /** Refused keys whose refusal retyping cannot fix (shut/locked): they restore the stored value and do not hold Save or Queue. */
  unfixable: Set<string>;
  /** `dirty` as it was before the pending batch opened, so an all-unfixable batch can give it back. */
  dirtyBatch: boolean | null;
  /** The plain sentence under each refused box, keyed by draft key (B3/B4's box home). */
  boxMessages: Record<string, string>;
  formErrors: string[];
  checkable: Record<string, { token: string; label: string }[]>;
  electionAt: string | null;
  electionPick: string | null;
  electionSavedAt: string | null;
  profileRefreshed: boolean;
  dateErrors: Record<string, string>;
  datesEdited: Set<string>;
  pending: Record<string, string>;
  rangeHeld: Record<string, string>;
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
    typed: new Set(), typedEdits: {}, periodParts: new Map(), periodSet: false, periodValues: {}, periodHeld: null, periodIncomplete: false, periodAsked: null,
    periodAnswered: null, periodComplete: false, periodSeq: 0, periodToken: 0, periodInFlight: null, opens: null,
    existing: null, saveAfterAnswer: null, submittedAt: null, submittedDryRun: false, submittedViaRelay: false, confirmedAt: null, amends: null, filedOriginal: null,
    filedOriginalView: null, profileUpdates: [], profileDiffers: [], profilePrerequisites: [], notes: [], saved: false, quiet: null, kept: {},
    autoStops: [], stopEdits: {}, stopKeys: new Set(), stoppable: new Set(), passed: new Set(), stepAt: null,
    stepFromTop: false, asks: [], askAt: new Map(), askPick: {}, askText: {}, askSavedAt: new Map(), factErrors: [],
    profileSave: {}, profileEdited: new Set(), profileAsked: new Set(), profileAskUnknown: false, guessed: new Set(), profileAskSeen: {}, profileAskMsg: {},
    askDone: new Set(), askDoneRaw: new Map(), profileChanged: [], errors: [], messages: {},
    messageKeys: {}, messageCounts: {}, rejected: {}, unfixable: new Set(), dirtyBatch: null, boxMessages: {}, formErrors: [], checkable: {}, electionAt: null, electionPick: null, electionSavedAt: null,
    profileRefreshed: false, dateErrors: {}, datesEdited: new Set(), pending: {}, rangeHeld: {}, chosenFrom: {}, dirty: false,
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
  private oneDateParts = new Set<string>();
  /** Month+year pairs drawn as one MM/YYYY field, keyed "<month key>#period" (PR 3). */
  private onePeriods = new Map<string, { period: OnePeriod; row: string }>();
  private periodPartKeys = new Set<string>();
  /** From/to pairs drawn as one row of two date controls, by each end's edit key (PR 3, R1). */
  private dateRanges = new Map<string, { range: DateRange; row: string }>();
  /** Range ends that are text boxes edited as dates (2550Q's From). */
  private rangeTextKeys = new Set<string>();
  /** The keys a range's refusal sits on (each end's anchor). */
  private rangeAnchorKeys = new Set<string>();
  /** Keys of the split month/day/year boxes (dateParts groups that are not one_date items): a host zero in one shows blank (B15). */
  private datePartKeys = new Set<string>();

  // Published signals.
  private sValue = new SignalMap<string>("");
  private sLocked = new SignalMap<boolean>(true);
  private sHint = new SignalMap<string>("");
  private sOff = new SignalMap<string>("");
  private sOptions = new SignalMap<ChoiceOption[]>([], sameJson);
  private sInvalid = new SignalMap<boolean>(false);
  private sBoxMsg = new SignalMap<string>("");
  private sGuessed = signal(0);
  private sRow = new SignalMap<RowInfo>(null as unknown as RowInfo, sameJson);
  private sPart = new SignalMap<PartInfo>({ errors: 0, todo: 0, items: 0, open: false, fold: null }, sameJson);
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
        const dates = oneDates(it);
        const periods = onePeriods(it);
        const ranges = dateRanges(it);
        if (dates.length) {
          for (const date of dates) {
            this.oneDates.set(date.oneKey, { date, row: id });
            for (const key of [date.month.k, date.day?.k, date.year.k]) {
              if (key) {
                this.datePartKeys.add(key);
                this.oneDateParts.add(key);
              }
            }
          }
        } else if (!periods.length && !ranges.length) {
          // The same groups rows.tsx draws as split date boxes: their zero
          // parts are the host's blank, never a shown 0/00/0000 (B15).
          const fs = it.f || [];
          if (!fs.some((f) => f.t === "date")) {
            const parts = dateParts(fs);
            if (parts) {
              for (const k of [parts.month.k, parts.day?.k, parts.year.k]) if (k) this.datePartKeys.add(k);
            }
          }
        }
        for (const period of periods) {
          this.onePeriods.set(period.periodKey, { period, row: id });
          this.periodPartKeys.add(period.month.k);
          this.periodPartKeys.add(period.year.k);
        }
        for (const range of ranges) {
          for (const end of [range.from, range.to]) {
            this.dateRanges.set(end.editKey, { range, row: id });
            this.rangeAnchorKeys.add(end.anchor);
            if (end.field && end.field.t !== "date") this.rangeTextKeys.add(end.anchor);
          }
        }
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
      const boxMsg = this.sBoxMsg.get(k);
      b = {
        value: () => value(),
        locked: () => locked(),
        hint: () => hint(),
        off: () => off(),
        options: () => options(),
        invalid: () => invalid(),
        message: () => boxMsg(),
        zeroWhenBlank: this.kindOf.get(k) === "money" && this.computedKeys.has(k),
      };
      this.boxes.set(k, b);
    }
    return b;
  }

  /** A radio group's ticked key, or null (its own binding over the radios' value signals). */
  radioSelected(keys: string[]): Accessor<string | null> {
    const sigs = keys.map((k) => [k, this.sValue.get(k)] as const);
    return () => {
      this.sGuessed();
      if (keys.some((k) => this.st.guessed.has(k))) return null;
      return sigs.find(([, s]) => s() === "true")?.[0] ?? null;
    };
  }

  private bumpGuessed() {
    this.sGuessed.set(this.sGuessed() + 1);
  }

  /** Whether the box is one of the return period's own parts: a pick there sets the period, never an untick. */
  isPeriodBox(k: string): boolean {
    return this.st.periodParts.has(k);
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
  /** The plain sentence under box k (a refused edit, a one-box validation error), "" none. */
  boxMessageOf(k: string): string {
    return this.st.boxMessages[k] || "";
  }
  /** Every box's sentence now (tests and the page's grouped lines). */
  boxMessages(): Readonly<Record<string, string>> {
    return this.st.boxMessages;
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
    return this.allKeys.length - this.oneDates.size * 2 - this.onePeriods.size;
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
      this.bumpGuessed();
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
    s.periodHeld = null;
    s.periodIncomplete = false;
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
    s.profilePrerequisites = r.profile_prerequisites || [];
    s.notes = (r.notes || []).map((n) => ({ key: n.key, text: n.text, recorded: false }));
    s.saved = Boolean(r.draft && (r.draft as { id?: number }).id);
    s.quiet = r.quiet || null;
    s.kept = r.kept && typeof r.kept === "object" ? { ...r.kept } : {};
    s.autoStops = Array.isArray(r.auto_stops) ? r.auto_stops : [];
    s.stopEdits = r.stop_edits && typeof r.stop_edits === "object" ? r.stop_edits : {};
    s.stopKeys = effectiveStops(s.autoStops, s.stopEdits);
    s.stoppable = new Set(Array.isArray(r.stoppable) ? r.stoppable : []);
    s.profileSave = r.profile_save && typeof r.profile_save === "object" ? r.profile_save : {};
    s.profileAsked = new Set(Array.isArray(r.profile_asked) ? r.profile_asked : []);
    s.profileAskUnknown = r.profile_ask_unknown === true;
    s.guessed = new Set(Array.isArray(r.guessed) ? r.guessed : []);
    this.bumpGuessed();
    s.profileAskSeen = {};
    s.profileAskMsg = {};
    s.askDone = new Set();
    s.askDoneRaw = new Map();
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
    if (s.sample && (s.periodIncomplete || !s.periodSet)) return { state: "manual", message: "Sample · set its period to save" };
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
    const host = !s.sample && s.editable && Array.isArray(asks) ? asks : [];
    // F2: a fact answered on this session keeps its card (marked saved) so a wrong pick can be
    // corrected; the host no longer lists it once set, so it is re-appended from the stored ask.
    const done = [...s.askDoneRaw.values()].filter((a) => !host.some((h) => h.fact === a.fact));
    s.asks = [...host, ...done];
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
    const period = this.onePeriods.get(k);
    if (period) return this.editPeriod(period.period, text);
    const kind = this.kindOf.get(k);
    if (kind === "sel" || kind === "chk" || kind === "radio") return this.choose({ [k]: text });
    this.sValue.get(k).set(text);
    if (s.periodParts.has(k)) return this.setPeriodPart({ [k]: text });
    let value = text;
    if (kind === "date" || this.rangeTextKeys.has(k)) {
      const date = parseTypedDate(text);
      s.datesEdited.add(k);
      if (date || !text.trim()) this.setDateError(k, "");
      value = date ? formatMDY(date) : text;
    }
    this.stage({ [k]: value });
    this.checkRangeOrders();
  }

  private editOneDate(date: OneDate, text: string) {
    const s = this.st;
    const keys = [date.month.k, date.day?.k, date.year.k].filter((key): key is string => Boolean(key));
    this.sValue.get(date.oneKey).set(text);
    const typed = parseTypedDate(text);
    const reject = (reason: string) => {
      for (const key of keys) {
        delete s.pending[key];
        delete s.rejected[key];
        delete s.boxMessages[key];
        s.unfixable.delete(key);
      }
      s.rejected[date.month.k] = reason;
      s.boxMessages[date.month.k] = this.refusalText(date.month.k, reason);
      s.dirty = true;
      this.cancelAutosave();
      this.markUnsaved();
      this.publish();
    };
    const problem = this.oneDateRefusal(date, text);
    if (problem) return reject(problem);
    this.setDateError(date.month.k, "");
    for (const key of keys) {
      delete s.rejected[key];
      delete s.boxMessages[key];
      s.unfixable.delete(key);
    }
    const [m, d, y] = typed ? formatMDY(typed).split("/") : ["", "", ""];
    const edits: Record<string, string> = {};
    for (const [field, value] of [[date.month, m], [date.day, d], [date.year, y]] as const) {
      if (!field || this.isShut(field.k)) continue;
      const listed = field.t === "sel" ? this.listedOptions(field.k) : [];
      const match = listed.find((option) => option.value === value || option.value === String(Number(value)) || (field === date.year && option.label === String(Number(value))));
      if (value && listed.length && !match) {
        reject("not one of the listed choices");
        return;
      }
      edits[field.k] = value && match ? match.value : value;
    }
    const periodEdits: Record<string, string> = {};
    const ordinaryEdits: Record<string, string> = {};
    for (const [key, value] of Object.entries(edits)) {
      if (s.periodParts.has(key)) periodEdits[key] = value;
      else ordinaryEdits[key] = value;
    }
    if (Object.keys(periodEdits).length) this.setPeriodPart(periodEdits);
    if (Object.keys(ordinaryEdits).length) {
      this.stage(ordinaryEdits);
      this.checkRangeOrders();
    }
    this.publish();
  }

  /**
   * A month+year pair typed as one MM/YYYY ("12/2023", "12-2023", "122023", "12 2023"): judged whole like a one-date —
   * a bad period lands nothing, its sentence sits on the month box, and a blank batch clears both parts. A locked part
   * is skipped (the calendar-year and listed-choice rules mirror oneDateRefusal/pickDate).
   */
  private editPeriod(period: OnePeriod, text: string) {
    const s = this.st;
    const wasIncomplete = s.periodIncomplete;
    const keys = [period.month.k, period.year.k];
    this.sValue.get(period.periodKey).set(text);
    const typed = parsePeriod(text);
    const reject = (reason: string) => {
      for (const key of keys) {
        delete s.pending[key];
        delete s.rejected[key];
        delete s.boxMessages[key];
        s.unfixable.delete(key);
      }
      s.rejected[period.month.k] = reason;
      s.boxMessages[period.month.k] = this.refusalText(period.month.k, reason);
      s.periodHeld = null; // the box no longer says the held period: blur or Save must not commit it
      s.periodIncomplete = s.sample && Boolean(text.trim());
      s.dirty = true;
      this.cancelAutosave();
      this.markUnsaved();
      this.publish();
    };
    const problem = this.periodRefusal(period, text);
    if (problem) return reject(problem);
    s.periodIncomplete = s.sample && Boolean(text.trim() && (!typed || typed.yDigits === 3));
    this.setDateError(period.month.k, "");
    for (const key of keys) {
      delete s.rejected[key];
      delete s.boxMessages[key];
      s.unfixable.delete(key);
    }
    const edits: Record<string, string> = {};
    const year4 = typed ? (typed.yDigits <= 2 ? 2000 + typed.y : typed.y) : 0;
    if (!typed) {
      // A blank period clears both boxes in one batch.
      for (const field of [period.month, period.year]) if (!this.isShut(field.k)) edits[field.k] = "";
    } else {
      const store = (field: SchemaField, year: boolean): string | null => {
        if (field.t !== "sel") {
          if (year) return this.periodTwoDigit(period) ? String(year4 - 2000).padStart(2, "0") : String(year4);
          return String(typed.m).padStart(2, "0");
        }
        const listed = this.listedOptions(field.k);
        const want = year ? [String(year4), String(year4 - 2000), String(typed.y)] : [String(typed.m), String(typed.m).padStart(2, "0")];
        const match = listed.find((o) => want.includes(o.value) || want.includes(String(Number(o.value))) || (year && o.label.includes(String(year4))));
        if (listed.length && !match) return null;
        return match ? match.value : year ? String(year4) : String(typed.m).padStart(2, "0");
      };
      for (const [field, year] of [[period.month, false], [period.year, true]] as const) {
        if (this.isShut(field.k)) continue;
        const value = store(field, year);
        if (value === null) {
          reject("not one of the listed choices");
          return;
        }
        edits[field.k] = value;
      }
    }
    const periodEdits: Record<string, string> = {};
    const ordinaryEdits: Record<string, string> = {};
    for (const [key, value] of Object.entries(edits)) {
      // The period lookup needs the absolute year even on a two-digit box: a select year stores "25" the host cannot read.
      // All four digits go: "0025" is year 25, which the host refuses, never the two-digit "25" it reads as 2025.
      if (s.periodParts.has(key)) periodEdits[key] = key === period.year.k && typed ? String(year4).padStart(4, "0") : value;
      else ordinaryEdits[key] = value;
    }
    if (Object.keys(periodEdits).length) {
      if (s.sample && typed && typed.yDigits <= 2) {
        // A year still being typed on a sample: "12/20" is not December 2020 yet. Hold it until the year is whole or the box is left.
        s.periodHeld = { ...s.periodHeld, ...periodEdits };
      } else if (s.sample && typed && typed.yDigits === 3) {
        // A third year digit is still mid-typing: nothing held may commit, and a year like 202 is never sent.
        s.periodHeld = null;
      } else {
        s.periodHeld = null;
        const samePeriod = JSON.stringify({ ...s.periodValues, ...periodEdits }) === s.periodAsked;
        this.setPeriodPart(periodEdits);
        if (wasIncomplete && !s.periodIncomplete && samePeriod) {
          if (Object.keys(s.typedEdits).length) this.scheduleAutosave();
          else if (!Object.keys(s.pending).length) {
            // Only typed edits make a sample worth saving: with none, the refusal left nothing unsaved.
            s.dirty = false;
            s.saveState = this.restingSaveState();
          }
        }
      }
    }
    if (Object.keys(ordinaryEdits).length) {
      this.stage(ordinaryEdits);
      this.checkRangeOrders();
    }
    if (s.periodIncomplete) {
      this.cancelAutosave();
      s.saveState = this.restingSaveState();
    }
    this.publish();
  }

  /** Period edits held while a two-digit year was typed on a sample (editPeriod) are committed now: the year reads as typed. */
  private commitHeldPeriod() {
    const s = this.st;
    if (!s.periodHeld) return;
    const held = s.periodHeld;
    s.periodHeld = null;
    this.setPeriodPart(held);
  }

  /** The period rules a typed text must pass before anything is staged ("" when it may land). */
  private periodRefusal(period: OnePeriod, text: string): string {
    const typed = parsePeriod(text);
    if (!typed) return text.trim() ? PERIOD_ERROR : "";
    if (typed.m < 1 || typed.m > 12) return "the month is 1 to 12";
    const year4 = typed.yDigits <= 2 ? 2000 + typed.y : typed.y;
    if (typed.yDigits <= 2 && !this.periodTwoDigit(period) && !this.isShut(period.year.k)) return PERIOD_TWO_DIGIT;
    if (this.calendarYearEnd({ month: period.month, day: null, year: period.year }) && typed.m !== 12 && !this.isShut(period.month.k)) {
      return "calendar year ends in December";
    }
    // A locked part keeps what it holds: the typed period must agree with it.
    for (const [field, number, year] of [[period.month, typed.m, false], [period.year, year4, true]] as const) {
      if (!this.isShut(field.k)) continue;
      const raw = String(this.st.values[field.k] ?? "");
      const held = year ? periodYear4(raw) : Number(String(raw).replace(/\D.*$/, ""));
      if (Number.isFinite(held) && held !== number) return "date conflicts with a locked part";
    }
    return "";
  }

  /** Whether the period's year box stores two digits (the host's mark first, the box's own shape else). */
  private periodTwoDigit(period: OnePeriod): boolean {
    if (period.twoDigit) return true;
    const f = period.year;
    if (f.t === "sel") return true;
    if (f.m !== undefined && f.m <= 2) return true;
    return /^\d{1,2}$/.test(String(this.st.values[f.k] ?? ""));
  }

  /** A tentative sheet edit uses the same period rules without changing the draft. */
  periodProblem(period: OnePeriod, text: string): string {
    const reason = this.periodRefusal(period, text);
    return reason ? this.refusalText(period.month.k, reason) : "";
  }

  /** A month picked on the period's grid (1-12), with the year it belongs to (the held one, or `year`). */
  pickPeriodMonth(period: OnePeriod, month: number, year?: number) {
    if (!this.st.editable || !this.st.draft) return;
    const held = periodYear4(String(this.st.values[period.year.k] ?? ""));
    const y = Number.isInteger(year) && (year as number) >= 1900 ? (year as number) : held >= 1900 ? held : new Date().getFullYear();
    this.editPeriod(period, `${month}/${y}`);
  }

  /** The year the period's month grid stands on: the held one, else this one. */
  periodYear(period: OnePeriod): number {
    const y = periodYear4(String(this.st.values[period.year.k] ?? ""));
    return Number.isInteger(y) && y >= 1900 && y <= 9999 ? y : new Date().getFullYear();
  }

  /** Judge every registered from/to pair: hold both ends while reversed, then restore them when the range is fixed. */
  private checkRangeOrders() {
    const s = this.st;
    let changed = false;
    const seen = new Set<DateRange>();
    for (const { range, row } of this.dateRanges.values()) {
      if (seen.has(range)) continue;
      seen.add(range);
      if (range.derived) continue;
      const from = this.rangeEndDay(range.from);
      const to = this.rangeEndDay(range.to);
      const key = range.to.anchor;
      const reversed = Boolean(from && to && from > to);
      const keys = [...this.rangeEndPendingKeys(range.from), ...this.rangeEndPendingKeys(range.to)];
      const refused = s.rejected[key] === "end before start";
      const hasHeld = keys.some((pk) => pk in s.rangeHeld);
      if (reversed) {
        if (!refused) {
          s.rejected[key] = "end before start";
          s.boxMessages[key] = this.refusalText(key, "end before start");
          s.touched.add(row);
          changed = true;
        }
        for (const end of [range.from, range.to]) {
          for (const pk of this.rangeEndPendingKeys(end)) {
            if (!(pk in s.pending)) continue;
            s.rangeHeld[pk] = s.pending[pk];
            delete s.pending[pk];
            changed = true;
          }
        }
      } else if (refused || hasHeld) {
        delete s.rejected[key];
        delete s.boxMessages[key];
        s.unfixable.delete(key);
        const restore: Record<string, string> = {};
        for (const pk of keys) {
          if (!(pk in s.rangeHeld)) continue;
          if (!(pk in s.pending)) restore[pk] = s.rangeHeld[pk];
          delete s.rangeHeld[pk];
        }
        if (Object.keys(restore).length) this.stage(restore);
        changed = true;
      }
    }
    if (changed) {
      s.dirty = true;
      this.cancelAutosave();
      this.markUnsaved();
      this.publish();
    }
  }

  /** The draft keys a range end's staged edit lives under (a triple's parts, else the box's own). */
  private rangeEndPendingKeys(end: { anchor: string; date: OneDate | null; field: SchemaField | null }): string[] {
    if (end.date) {
      return [end.date.month.k, end.date.day?.k, end.date.year.k].filter((k): k is string => Boolean(k));
    }
    return end.field ? [end.field.k] : [end.anchor];
  }

  /** A range end's day as ISO date ("" while either part is blank or unparseable). Pending and held edits count. */
  rangeEndDay(end: { anchor: string; date: OneDate | null; field: SchemaField | null }): string {
    const s = this.st;
    const value = (k: string) => String(s.pending[k] ?? s.rangeHeld[k] ?? s.values[k] ?? "");
    if (end.date) {
      const d = end.date;
      const m = Number(value(d.month.k));
      const y = periodYear4(value(d.year.k));
      const day = d.day ? Number(value(d.day.k)) : 1;
      if (!(m >= 1 && m <= 12) || !(y >= 1900) || !(day >= 1 && day <= 31)) return "";
      return `${y}-${String(m).padStart(2, "0")}-${String(day).padStart(2, "0")}`;
    }
    if (end.field) {
      const date = parseTypedDate(value(end.field.k));
      return date ? formatIso(date) : "";
    }
    return "";
  }

  private oneDateRefusal(date: OneDate, text: string): string {
    const typed = parseTypedDate(text);
    if (!typed) return text.trim() ? isTwoDigitDate(text) ? "two-digit date year" : DATE_ERROR : "";
    if (this.calendarYearEnd(date) && typed.m !== 12 && !this.isShut(date.month.k)) return "calendar year ends in December";
    const iso = formatIso(typed);
    const { min, max } = dateRange(this, date);
    if (min && max && (iso < min || iso > max)) return `date outside allowed span ${min} ${max}`;
    for (const [field, number] of [[date.month, typed.m], [date.day, typed.d], [date.year, typed.y]] as const) {
      if (!field || !this.isShut(field.k)) continue;
      const raw = String(this.st.values[field.k] ?? "");
      const held = field === date.year && /^\d{1,2}$/.test(raw) ? 2000 + Number(raw) : Number(raw.slice(0, field === date.month ? 2 : undefined));
      if (held !== number) return "date conflicts with a locked part";
    }
    return "";
  }

  /** A tentative sheet edit uses the same rules without changing the draft. */
  oneDateProblem(date: OneDate, text: string): string {
    const reason = this.oneDateRefusal(date, text);
    return reason ? this.refusalText(date.month.k, reason) : "";
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
    if (!Object.keys(s.pending).length) s.dirtyBatch = s.dirty;
    for (const [k, v] of Object.entries(edits)) {
      s.pending[k] = v;
      if (s.opened.has(k) && s.profileSave[k]) s.profileEdited.add(k);
      delete s.kept[k];
      delete s.rejected[k];
      s.unfixable.delete(k);
      delete s.boxMessages[k];
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
    // B17: a sel box with a known list takes only a listed code; an
    // unlisted one is refused on the box without a host call, and the
    // stored value stays.
    const offList = new Set<string>();
    for (const [k, v] of Object.entries(edits)) {
      // A shut box (locked, computed, off) keeps its own refusal.
      if (this.kindOf.get(k) !== "sel" || !String(v).trim() || this.isShut(k)) continue;
      const listed = this.listedOptions(k);
      if (listed.length > 0 && !listed.some((o) => o.value === String(v))) {
        s.rejected[k] = "not one of the listed choices";
        offList.add(k);
      }
    }
    if (offList.size) {
      edits = Object.fromEntries(Object.entries(edits).filter(([k]) => !offList.has(k)));
      this.setErrors(s.errors);
      this.publish();
      if (!Object.keys(edits).length) return;
    }
    if (!Object.keys(s.pending).length) s.dirtyBatch = s.dirty;
    Object.assign(s.pending, edits);
    s.dirty = true;
    this.unquiet(Object.keys(edits));
    for (const k of Object.keys(edits)) {
      delete s.kept[k];
      delete s.rejected[k];
      s.unfixable.delete(k);
    }
    s.chosenFrom = Object.fromEntries(Object.keys(edits).map((k) => [k, s.values[k] ?? ""]));
    for (const [k, v] of Object.entries(edits)) {
      s.values[k] = String(v);
      this.sValue.get(k).set(this.shownText(k));
    }
    for (const k of Object.keys(edits)) delete s.boxMessages[k];
    this.checkRangeOrders();
    this.cancelAutosave();
    this.markUnsaved();
    this.flush();
    // Q1 P1b: a committed choice on a profile-backed box may raise "Save to profile?".
    this.maybeProfileAskKeys(Object.keys(edits));
  }

  /** The save line says "Unsaved changes" the moment something is staged (an explicit-save return keeps its reason). */
  private markUnsaved() {
    const s = this.st;
    if (s.sample && s.periodIncomplete) {
      s.saveState = this.restingSaveState();
      this.sSave.set(s.saveState);
      return;
    }
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
    const guessed = (it.f || [])
      .filter((radio) => radio.t === "radio" && (radio.grp || "") === (f.grp || "") && s.guessed.has(radio.k))
      .map((radio) => radio.k);
    if (guessed.length) {
      for (const key of guessed) s.guessed.delete(key);
      this.bumpGuessed();
    }
    if (on && !period && (f.rg || 0) >= 3 && !guessed.length) return this.choose({ [k]: "false" });
    if (on && !period && !f.rg && !guessed.length) return;
    const edits: Record<string, string> = { [k]: "true" };
    if (f.grp) for (const r of it.f || []) if (r.t === "radio" && r.k !== k && r.grp === f.grp) edits[r.k] = "false";
    this.choose(edits);
  }

  /** The draft key a synthetic control stands for (a one-date's or a period's month box). */
  private anchorKey(k: string): string {
    return this.oneDates.get(k)?.date.month.k ?? this.onePeriods.get(k)?.period.month.k ?? k;
  }

  /** A box gained focus: an item still open with Edit elsewhere goes back to its profile look. */
  focus(k: string) {
    const s = this.st;
    s.focusKey = k;
    const here = this.keyIndex.get(this.anchorKey(k));
    if (this.endProfileEdit([...s.editing].filter((key) => this.keyIndex.get(key) !== here))) this.publish();
  }

  /** A box lost focus: its row is touched (its message may show now), an amount or a date takes its shown form, and it flushes. */
  blur(k: string) {
    const s = this.st;
    const one = this.oneDates.get(k);
    const period = this.onePeriods.get(k);
    if (period) this.commitHeldPeriod();
    const baseKey = one ? one.date.month.k : period ? period.period.month.k : k;
    const id = this.keyIndex.get(baseKey);
    if (s.focusKey === k) s.focusKey = null;
    const kind = one || period ? "date" : this.kindOf.get(k);
    const v = this.sValue.get(k);
    const raw = untracked(() => v());
    if (kind === "money" && !this.isShut(k)) v.set(formatMoney(raw, this.wholeKeys.has(k)));
    else if (kind === "date" && !this.isShut(k)) {
      const date = parseTypedDate(raw);
      if (date && !(baseKey in s.rejected)) {
        v.set(formatLongDate(date));
        this.setDateError(baseKey, "");
      } else if (!one) this.setDateError(baseKey, raw.trim() && s.datesEdited.has(k) ? DATE_ERROR : "");
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
    this.syncHostCase(k);
    // Q1 P1b: leaving a profile-backed box with a new complete value asks "Save to profile?" once.
    this.maybeProfileAskKeys([baseKey]);
  }

  /**
   * B20: a text box the host answered in another case or outer spaces takes the host's text once it
   * is left (0605's official map capital()s name, address and line of business, so "Retail" is
   * "RETAIL" in the draft's shown values). While the box has the keys it keeps what was typed —
   * no caret jumps — and a refused edit keeps its typed text (B19).
   */
  private syncHostCase(k: string) {
    const s = this.st;
    if (!s.draft || this.reopening) return;
    const kind = this.kindOf.get(k);
    if (kind === "money" || kind === "pct" || kind === "int" || kind === "sel" || kind === "chk" || kind === "radio" || kind === "date") return;
    if (this.oneDates.has(k) || this.onePeriods.has(k) || this.datePartKeys.has(k) || this.periodPartKeys.has(k) || this.rangeTextKeys.has(k) || this.pools.has(k)) return;
    if (s.rejected[k] !== undefined || s.unfixable.has(k) || this.isShut(k)) return;
    const box = this.sValue.get(k);
    if (!box) return;
    const shown = untracked(box);
    const host = String(s.values[k] ?? "");
    if (shown === host || shown.trim().toLowerCase() !== host.trim().toLowerCase()) return;
    box.set(this.shownText(k));
  }

  /** The part whose box has the keyboard now, or null: the page keeps that part's rows drawn wherever it scrolls. */
  focusedPart(): number | null {
    const k = this.st.focusKey;
    const id = k ? this.keyIndex.get(this.anchorKey(k)) : undefined;
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
    s.profilePrerequisites = r.profile_prerequisites || [];
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
    this.syncShown(before, edits, r.rejected);
    this.checkRangeOrders();
    this.livePreview();
    this.sayYearEndPutBack(edits);
    const rej = r.rejected || {};
    // A batch the host refused whole for shut/locked boxes changed nothing: give `dirty` back (R7).
    const heldDirty = s.dirtyBatch;
    s.dirtyBatch = null;
    const allUnfixable =
      Object.keys(edits).length > 0 && Object.keys(edits).every((k) => k in rej && unfixableRefusal(rej[k]));
    // A refused box shows its own sentence under it; its message shows (its row is touched) until it is edited again.
    for (const [k, msg] of Object.entries(rej)) {
      const sentence = this.refusalText(k, msg);
      s.boxMessages[k] = s.boxMessages[k] ? `${s.boxMessages[k]} ${sentence}` : sentence;
      const id = this.keyIndex.get(k);
      if (!id) {
        s.messages["form"] = s.messages["form"] ? `${s.messages["form"]} ${sentence}` : sentence;
        continue;
      }
      s.rejected[k] = msg;
      s.touched.add(id);
      if (unfixableRefusal(msg)) {
        // R7: retyping cannot fix a shut/locked box: it goes back to the stored value, keeps its
        // sentence, and does not hold Save at "Unsaved" or block Queue.
        s.unfixable.add(k);
        this.sValue.get(k).set(this.shownText(k));
      }
      // B19: a fixable refusal keeps what was typed; the draft never took it.
    }
    if (allUnfixable && heldDirty !== null) {
      s.dirty = heldDirty;
      s.saveState = this.restingSaveState();
      this.sSave.set(s.saveState);
    }
    // An emptied number box the host holds as zero shows the zero (the box selects it, so the next digit replaces it).
    for (const k of r.cleared || []) if (!(k in s.pending)) this.sValue.get(k).set(this.shownText(k));
  }

  /** A shut box whose gate opened since its refusal drops the stale sentence (R7). */
  private liftOpenGates() {
    const s = this.st;
    for (const k of [...s.unfixable]) {
      if (this.isShut(k)) continue;
      delete s.rejected[k];
      delete s.boxMessages[k];
      s.unfixable.delete(k);
      const id = this.keyIndex.get(k);
      if (id) s.messageKeys[id] = (s.messageKeys[id] || []).filter((x) => x !== k);
    }
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
        this.liftOpenGates();
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

  /** What a refusal sentence is built from for box k (B3/B4's context). */
  private refusalCtx(k: string, raw: string): RefusalContext {
    const s = this.st;
    const id = this.keyIndex.get(k);
    const it = id ? this.items.get(id) : undefined;
    const item = refusalItem(it?.no);
    const f = this.fieldOf.get(k);
    const label = refusalLabel(f?.l, it?.l, k);
    return {
      raw, key: k, item, label,
      kind: this.kindOf.get(k) ?? "", form: s.target.formCode,
      part: this.datePartName(k),
    };
  }

  /** "month"/"day"/"year" when k is a split date part, else undefined. */
  private datePartName(k: string): string | undefined {
    const id = this.keyIndex.get(k);
    const it = id ? this.items.get(id) : undefined;
    const date = it?.f ? dateParts(it.f) : null;
    if (!date) return undefined;
    if (date.month.k === k) return "month";
    if (date.day?.k === k) return "day";
    if (date.year.k === k) return "year";
    return undefined;
  }

  /** A refused fill's plain sentence for box k. */
  private refusalText(k: string, raw: string): string {
    return refusalSentence(this.refusalCtx(k, raw));
  }

  /** A host validation message as the page shows it (0605's four, plus the leak net). */
  private validationText(k: string | null, raw: string, field?: string): string {
    return hostSentence(this.refusalCtx(k ?? "", raw), field);
  }

  /** Bind each validation message to an item (bindError). */
  private setErrors(errors: ScreenFormError[] | undefined) {
    const s = this.st;
    s.errors = [...(errors || []), ...(s.factErrors || [])].filter((e) => !(s.quiet && e.field === s.quiet.field));
    s.messages = {};
    s.messageKeys = {};
    s.messageCounts = {};
    s.boxMessages = {};
    s.checkable = {};
    s.electionAt = null;
    const items = this.rowIds.map((id) => ({ id, it: this.items.get(id)! }));
    // A refused edit's sentence sits on its box; it counts toward the row it belongs to.
    for (const [k, msg] of Object.entries(s.rejected)) {
      const sentence = this.refusalText(k, msg);
      s.boxMessages[k] = s.boxMessages[k] ? `${s.boxMessages[k]} ${sentence}` : sentence;
      const id = this.keyIndex.get(k);
      if (id) (s.messageKeys[id] ||= []).push(k);
    }
    for (const e of s.errors) {
      const keys = (e.keys || []).filter((k) => this.keyIndex.has(k));
      // A validation error naming exactly one drawn box is that box's message (B3/B4).
      if (keys.length === 1) {
        const k = keys[0];
        const id = this.keyIndex.get(k)!;
        const sentence = this.validationText(k, e.message, String(e.field));
        s.boxMessages[k] = s.boxMessages[k] ? `${s.boxMessages[k]} ${sentence}` : sentence;
        (s.messageKeys[id] ||= []).push(k);
        if (e.dismiss) (s.checkable[id] ||= []).push({ token: e.dismiss, label: e.dismiss_label || "Checked" });
        if (e.field === "annual_income_tax_election" && ELECTION_UNRECORDED.test(e.message)) s.electionAt = id;
        continue;
      }
      const id = bindError(e, items, (k) => this.keyIndex.get(k));
      if (!id) continue;
      // A split date group's item-level sentence shows only when no part has its own box message.
      const it = this.items.get(id);
      const date = it?.f ? dateParts(it.f) : null;
      if (date && [date.month.k, date.day?.k, date.year.k].some((pk) => pk && s.boxMessages[pk])) continue;
      const sentence = this.validationText(null, e.message, String(e.field));
      s.messages[id] = s.messages[id] ? `${s.messages[id]} ${sentence}` : sentence;
      (s.messageKeys[id] ||= []).push(...(e.keys || []));
      s.messageCounts[id] = (s.messageCounts[id] || 0) + 1;
      if (e.dismiss) (s.checkable[id] ||= []).push({ token: e.dismiss, label: e.dismiss_label || "Checked" });
      if (e.field === "annual_income_tax_election" && ELECTION_UNRECORDED.test(e.message)) s.electionAt = id;
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
    if (one) {
      const parts = [one.date.month, one.date.day, one.date.year].filter((field): field is SchemaField => Boolean(field));
      if (parts.some((field) => !String(s.values[field.k] ?? "").trim() || /^0+$/.test(String(s.values[field.k]).trim()))) return "";
      return shownDate(oneDateText(s.values, one.date));
    }
    const period = this.onePeriods.get(k);
    if (period) {
      const p = period.period;
      const parts = [p.month, p.year];
      if (parts.some((field) => !String(s.values[field.k] ?? "").trim() || /^0+$/.test(String(s.values[field.k]).trim()))) return "";
      return periodText(String(s.values[p.month.k] ?? ""), String(s.values[p.year.k] ?? ""));
    }
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
    // A split date part the host holds as zero (its blank, e.g. 0619E's
    // cleared day) shows blank, never 0/00/0000; the draft keeps the zero.
    if (this.datePartKeys.has(k) && /^0+$/.test(raw)) return "";
    return raw;
  }

  /** A select's listed choices, without the shown value appended for display. */
  private listedOptions(k: string): ChoiceOption[] {
    const s = this.st;
    const f = this.fieldOf.get(k);
    return (s.options[k] || f?.o || []).map(optionOf).filter((o): o is ChoiceOption => Boolean(o));
  }

  /** A select's shown value and choices (old selectBox). */
  private selShown(k: string): { value: string; options: ChoiceOption[] } {
    const s = this.st;
    const listed = this.listedOptions(k);
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
    for (const [k, { period }] of this.onePeriods) {
      if ([period.month.k, period.year.k].some((p) => p in pending)) continue;
      this.sValue.get(k).set(this.shownText(k));
    }
  }

  /** Shut boxes follow the draft; a box the user can type keeps its text unless the host changed it (old syncChanged). */
  private syncShown(before: Record<string, string>, edits: Record<string, string>, rejected?: Record<string, string>) {
    const s = this.st;
    for (const k of this.allKeys) {
      if (k in edits) {
        // A refused edit keeps what was typed (B19); only an accepted write the host
        // changed to something other than what was typed shows the host's value.
        if (rejected && k in rejected) continue;
        // A box the host changed to something other than what was typed (a schedule row moved up into a cleared
        // row) shows the host's value, unless typed again since.
        if (!(k in s.pending) && !this.pools.has(k) && (this.oneDateParts.has(k) || this.hostChanged(k, edits[k]))) this.sValue.get(k).set(this.shownText(k));
        continue;
      }
      if (this.isShut(k) || this.pools.has(k) || (before[k] ?? "") !== (s.values[k] ?? "")) this.sValue.get(k).set(this.shownText(k));
    }
    for (const [k, { date }] of this.oneDates) {
      const parts = [date.month.k, date.day?.k, date.year.k].filter(Boolean) as string[];
      if (parts.some((p) => p in s.rejected) || s.focusKey === k) continue;
      const changedByHost = parts.some((p) => p in edits && this.hostChanged(p, edits[p]));
      if (parts.some((p) => p in edits) && !changedByHost) continue;
      if (changedByHost || oneDateText(before, date) !== oneDateText(s.values, date)) this.sValue.get(k).set(this.shownText(k));
    }
    for (const [k, { period }] of this.onePeriods) {
      const parts = [period.month.k, period.year.k];
      if (parts.some((p) => p in s.rejected) || s.focusKey === k) continue;
      const changedByHost = parts.some((p) => p in edits && this.hostChanged(p, edits[p]));
      if (parts.some((p) => p in edits) && !changedByHost) continue;
      const beforeText = periodText(before[parts[0]] ?? "", before[parts[1]] ?? "");
      const nowText = periodText(s.values[parts[0]] ?? "", s.values[parts[1]] ?? "");
      if (changedByHost || beforeText !== nowText) this.sValue.get(k).set(this.shownText(k));
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
    const period = this.onePeriods.get(k);
    if (period) {
      const parts = [period.period.month, period.period.year];
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

  // ---------------------------------------------------------------- gated boxes (G1)

  /** The host's is_blank on what the draft holds: "", "false", or only 0 . , - (conditions.rs). */
  private blankShown(k: string): boolean {
    const v = String(this.st.values[k] ?? "").trim();
    return v === "" || v === "false" || [...v].every((c) => "0.,-".includes(c));
  }

  /**
   * A box another answer keeps shut (s.off) draws nothing — unless it still holds something the filer must see:
   * a value from an old draft, an import or a pre-B16 write stays drawn faded with its "Needs …" line, and a box
   * carrying a sentence (its own, a row message's, or a to-do stop) stays too, so no text points at nothing.
   */
  private hiddenBox(k: string): boolean {
    const s = this.st;
    if (!s.off[k] || !this.blankShown(k) || s.boxMessages[k] || s.stopKeys.has(k)) return false;
    const id = this.keyIndex.get(k);
    return !(id && s.messages[id] && (s.messageKeys[id] || []).includes(k));
  }

  /** The cell keys a schedule draws (its hidden columns' cells are not drawn). */
  private drawnCells(it: SchemaItem): string[] {
    const cols = it.cols || [];
    const out: string[] = [];
    for (const row of it.keys || []) {
      row.forEach((k, c) => {
        const col = cols[c];
        if (col && col.t !== "hidden") out.push(k);
      });
    }
    return out;
  }

  /** The keys of the boxes a row draws: a schedule's cells, else its fields (a pooled line's head only). */
  private drawnKeys(it: SchemaItem): string[] {
    return it.rows ? this.drawnCells(it) : (it.f || []).map((f) => f.k);
  }

  /** G1: the whole row is gated away — every drawn box shut, and no message or to-do points at it. */
  private rowHidden(id: string, it: SchemaItem, todoHere: boolean, said: boolean): boolean {
    const s = this.st;
    if (todoHere || said || s.dateErrors[id]) return false;
    const drawn = this.drawnKeys(it);
    return drawn.length > 0 && drawn.every((k) => this.hiddenBox(k));
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
    const id = this.keyIndex.get(this.anchorKey(k));
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
      s.profilePrerequisites = r.profile_prerequisites || [];
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

  openProfilePrerequisite(field: ScreenFormProfilePrerequisite["field"]) {
    this.hooks.onOpenProfile?.(field, this.taxYear());
  }

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

  // ------------------------------------------------- "Save to profile?" (Q1 P1b / Q1·a)

  /** The `profile_save` keys of one profile field (a fact's box keys, or a profile box's). */
  private profileSaveKeys(field: string): string[] {
    const s = this.st;
    return Object.keys(s.profileSave).filter((k) => s.profileSave[k].field === field);
  }

  /** A changed profile-backed box may raise the ask only on a return the filer can still change. */
  private askAllowed(): boolean {
    const s = this.st;
    return s.target.mode === "return" && s.editable && !s.sample && s.status === "Draft" && !s.amends && !!s.draft;
  }

  /** The shown-value signature of a field's boxes: a dismissed ask re-asks only on a changed value. */
  private askSignature(keys: string[]): string {
    const s = this.st;
    return keys.map((k) => String(s.values[k] ?? "")).join("");
  }

  /** Whether the field's boxes now show a value that differs from the profile's. */
  private profileDiffers(keys: string[]): boolean {
    const s = this.st;
    return keys.some(
      (k) => this.normProfileVal(s.values[k]) !== this.normProfileVal(s.profileSave[k]?.profile),
    );
  }

  private normProfileVal(v: unknown): string {
    return String(v ?? "").replace(/[,\s₱]/g, "").trim().toLowerCase();
  }

  /** After a box commit (choose) or a blur that flushed: ask once per field on a changed accepted value. */
  private maybeProfileAskKeys(keys: string[]) {
    const s = this.st;
    if (!this.askAllowed()) return;
    const fields = new Set<string>();
    for (const k of keys) {
      const e = s.profileSave[k];
      if (e) fields.add(e.field);
    }
    for (const field of fields) this.maybeProfileAsk(field);
  }

  private maybeProfileAsk(field: string) {
    const s = this.st;
    if (s.profileAskUnknown) return;
    if (s.profileAsked.has(field)) return;
    if (s.overlay?.kind === "profile-ask") return;
    const keys = this.profileSaveKeys(field);
    if (!keys.length) return;
    const ask = this.profileAskFor(field, keys);
    if (!ask) return;
    const sig = this.askSignature(ask.keys);
    if (s.profileAskSeen[field] === sig) return;
    s.profileAskSeen[field] = sig;
    s.overlay = { kind: "profile-ask", ask };
    this.publish();
  }

  /** The ask for one field, or null when its boxes do not hold a new complete accepted value. */
  private profileAskFor(field: string, keys: string[]): ProfileAsk | null {
    const s = this.st;
    const entry = s.profileSave[keys[0]];
    if (!entry) return null;
    const name = this.taxpayerName();
    const year = this.taxYear();
    const personal = field === "birth_date" || field === "name";
    const radio = keys.every((k) => this.kindOf.get(k) === "radio" || this.kindOf.get(k) === "chk");
    if (radio) return this.profileAskRadio(field, keys, entry.label, name, year, personal);
    const date = this.profileDateParts(keys);
    if (date) return this.profileAskDate(field, keys, entry.label, date, name, year, personal);
    return this.profileAskValue(field, keys, entry.label, name, year, personal);
  }

  /** A radio/select card's field: the pick the filer made this session, when it differs from the profile's. */
  private profileAskRadio(
    field: string,
    keys: string[],
    label: string,
    name: string,
    year: number,
    personal: boolean,
  ): ProfileAsk | null {
    const s = this.st;
    const picked = keys.filter((k) => String(s.values[k] ?? "") === "true" && s.typed.has(k));
    if (!picked.length) return null;
    const k = picked[picked.length - 1];
    if (keys.some((x) => s.rejected[x] !== undefined || s.unfixable.has(x))) return null;
    // The profile already shows this pick: nothing new to save.
    if (String(s.profileSave[k]?.profile ?? "") === "true") return null;
    const f = this.fieldOf.get(k);
    const value = (f && choiceLabel(f)) || k;
    if (!value.trim()) return null;
    return { field, label, value, keys, name, year, personal };
  }

  /** A split or one-date field's month/day/year boxes as a date, when the keys form one. */
  private profileDateParts(keys: string[]): DateParts | null {
    const byItem = new Map<string, string[]>();
    for (const k of keys) {
      const id = this.keyIndex.get(k);
      if (!id) continue;
      const arr = byItem.get(id);
      if (arr) arr.push(k);
      else byItem.set(id, [k]);
    }
    for (const [id, ks] of byItem) {
      const it = this.items.get(id);
      if (!it) continue;
      const d = dateParts(it.f || []);
      if (!d) continue;
      const partKeys = [d.month.k, d.day?.k, d.year.k].filter((x): x is string => Boolean(x));
      if (partKeys.length > 0 && partKeys.every((pk) => keys.includes(pk))) return d;
    }
    return null;
  }

  /** A date field: every part filled (never half-typed), accepted, and different from the profile. */
  private profileAskDate(
    field: string,
    keys: string[],
    label: string,
    date: DateParts,
    name: string,
    year: number,
    personal: boolean,
  ): ProfileAsk | null {
    const s = this.st;
    const partKeys = [date.month.k, date.day?.k, date.year.k].filter((x): x is string => Boolean(x));
    if (partKeys.some((k) => this.isShut(k))) return null;
    if (!partKeys.every((k) => String(s.values[k] ?? "").trim())) return null;
    if (partKeys.some((k) => s.rejected[k] !== undefined || s.unfixable.has(k))) return null;
    if (!partKeys.some((k) => s.typed.has(k))) return null;
    if (!this.profileDiffers(partKeys)) return null;
    const value = shownDate(oneDateText(s.values, date));
    if (!value.trim()) return null;
    return { field, label, value, keys, name, year, personal };
  }

  /** A text/money/select field: a complete accepted non-empty value that differs from the profile. */
  private profileAskValue(
    field: string,
    keys: string[],
    label: string,
    name: string,
    year: number,
    personal: boolean,
  ): ProfileAsk | null {
    const s = this.st;
    const primary = keys[0];
    if (keys.some((k) => this.isShut(k))) return null;
    if (keys.some((k) => s.rejected[k] !== undefined || s.unfixable.has(k))) return null;
    if (!keys.some((k) => s.typed.has(k))) return null;
    if (!this.profileDiffers(keys)) return null;
    const value = keys.map((k) => this.shownText(k)).map((t) => t.trim()).filter(Boolean).join(", ");
    if (!value || !this.shownText(primary).trim()) return null;
    return { field, label, value, keys, name, year, personal };
  }

  /** The row a field's box sits on, for the answer's status line. */
  private askRowId(keys: string[]): string | null {
    for (const k of keys) {
      const id = this.keyIndex.get(k);
      if (id) return id;
    }
    return null;
  }

  /** The answer's line under the box: saved (ok) or kept on this return only (subtle). */
  private profileAskResult(ask: ProfileAsk, ok: boolean) {
    const id = this.askRowId(ask.keys);
    if (!id) return;
    this.st.profileAskMsg[id] = ok
      ? { ok: true, text: "Saved to the tax profile. Later returns use it." }
      : { ok: false, text: "Kept on this return only." };
  }

  /** "Only this return": nothing is written; the answer is remembered for this taxpayer and year. */
  profileAskOnly() {
    const s = this.st;
    const o = s.overlay;
    if (o?.kind !== "profile-ask") return;
    const ask = o.ask;
    s.overlay = null;
    try {
      bir.screen_form_profile_asked(s.target.tin, this.taxYear(), ask.field, "only");
      this.wrote("screen_form_profile_asked");
    } catch (e) {
      this.toast(plainRefusal(errorText(e)), "triangle-alert");
    }
    s.profileAsked.add(ask.field);
    this.profileAskResult(ask, false);
    this.publish();
  }

  /** "Save to profile": the return is stored first (the host needs a saved draft), then the boxes go. */
  profileAskSave() {
    const s = this.st;
    const o = s.overlay;
    if (o?.kind !== "profile-ask") return;
    const ask = o.ask;
    s.overlay = null;
    // The host writes a stored draft's boxes: save the return first when it is not stored yet.
    if (s.dirty || Object.keys(s.pending).length || !s.saved) this.saveReturn(false);
    const draftId = s.draft && (s.draft as { id?: number }).id;
    if (!s.draft || !draftId) {
      this.toast("Save the return first, then save to the tax profile.", "triangle-alert");
      this.publish();
      return;
    }
    try {
      const r = bir.screen_form_save_to_profile(s.target.formCode, s.target.year, s.target.period, s.draft, ask.keys);
      this.wrote("screen_form_save_to_profile");
      s.draft = r.draft;
      const before = s.values;
      s.values = this.shown({ ...s.values, ...(r.values || {}) });
      if (r.pools) s.poolTexts = r.pools;
      this.syncShown(before, {});
      for (const k of ask.keys) s.profileEdited.delete(k);
      try {
        bir.screen_form_profile_asked(s.target.tin, this.taxYear(), ask.field, "save");
        this.wrote("screen_form_profile_asked");
      } catch (_) {
        // The profile write landed; a lost record only means a later return may ask again.
      }
      s.profileAsked.add(ask.field);
      s.electionReady = undefined;
      this.profileAskResult(ask, true);
      const saved = (r.saved || []).join(", ");
      this.toast(saved ? `Saved to the tax profile: ${saved}.` : "Saved to the tax profile.", "circle-check");
    } catch (e) {
      // Nothing was written and nothing remembered: the next changed leave asks again.
      this.toast(plainRefusal(errorText(e)), "triangle-alert");
    }
    this.publish();
  }

  /** Escape, scrim or swipe-down: no answer — nothing saved, nothing remembered, asks again on a changed leave. */
  dismissProfileAsk() {
    const s = this.st;
    if (s.overlay?.kind !== "profile-ask") return;
    s.overlay = null;
    this.publish();
  }

  /** A choice card's answer picked. */
  pickAsk(fact: ScreenFormFact, value: unknown) {
    const s = this.st;
    s.askPick[fact] = value;
    const ask = s.asks.find((a) => a.fact === fact);
    // F2: a choice card has no Save — the pick is the explicit answer and saves at once.
    if (ask?.control === "choice") return this.saveAsk(fact);
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
    // F2: the card stays for the session (a wrong pick can be corrected); the host no longer lists
    // the fact once set, so its ask is re-appended by takeAsks and marked done for the "Saved" chip.
    s.askDone.add(fact);
    s.askDoneRaw.set(fact, ask);
    const r = this.applyUseProfile([fact]);
    this.takeAsks(r && Array.isArray(r.asks) ? r.asks : s.asks.filter((a) => a !== ask));
    if (ask.control !== "choice") delete s.askPick[fact];
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
    this.st.openParts = new Set();
    this.revealTarget(pi, row);
  }

  private revealTarget(pi: number, row: string | null) {
    const s = this.st;
    s.openParts.add(pi);
    if (row && this.tuckedItems(pi).includes(Number(row.split(":")[1]))) s.profileOpen.add(pi);
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
    this.revealTarget(target.part, target.row);
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
    const date = oneDates(it).find((candidate) => [candidate.month.k, candidate.day?.k, candidate.year.k].includes(pick.k));
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

  /**
   * Whether the row prints this box's sentence on a line of its own, at the row's width, instead of the box under itself: date
   * parts, a TIN's segments and the pick kinds (their controls have no message line), a whole-date box (its control is cut to
   * the box's height), and a box alone in its row (nothing sits beside it, so its sentence need not wrap at the box's width).
   * Boxes side by side keep their sentences under themselves.
   */
  private rowPrints(k: string): boolean {
    const id = this.keyIndex.get(k);
    const it = id ? this.items.get(id) : undefined;
    if (["sel", "chk", "radio", "date"].includes(this.kindOf.get(k) ?? "") || this.datePartKeys.has(k) || this.periodPartKeys.has(k)
      || this.rangeAnchorKeys.has(k)
      || (it && oneDates(it).some((date) => [date.month.k, date.day?.k, date.year.k].includes(k)))) return true;
    return Boolean(it && !it.rows && (isTinItem(it) || (it.f || []).length === 1));
  }

  /** A box's sentence as it shows: its row's after it was touched, every one after Check (a refusal touches its row as it lands). */
  private shownBoxMsg(k: string): string {
    const s = this.st;
    const sentence = s.boxMessages[k];
    if (!sentence) return "";
    const id = this.keyIndex.get(k);
    return s.showAll || !id || s.touched.has(id) ? sentence : "";
  }

  /** How many box-home sentences row id carries (each box's message counts once). */
  private boxCountFor(id: string): number {
    const it = this.items.get(id);
    if (!it) return 0;
    const keys = it.rows ? (it.keys || []).flat() : keysOf(it);
    return keys.filter((k) => this.st.boxMessages[k]).length;
  }

  /** How many messages each row with a shown message carries (a date's own sentence counts once). */
  private shownCounts(): Record<string, number> {
    const s = this.st;
    const shown = this.shownMessages();
    const out: Record<string, number> = {};
    const ids = new Set([...Object.keys(shown), ...this.rowIds.filter((id) => this.boxCountFor(id) > 0)]);
    for (const id of ids) {
      const it = this.items.get(id);
      const dateSaid = it && oneDates(it).some((date) => [date.month.k, date.day?.k, date.year.k].some((key) => key && this.shownBoxMsg(key)));
      const row = shown[id] && !dateSaid ? Math.max(s.dateErrors[id] ? 1 : 0, s.messageCounts[id] || 0, 1) : 0;
      const n = row + this.boxCountFor(id);
      if (n) out[id] = n;
    }
    return out;
  }

  /** How many rows of part pi hold a validation message, shown or not (the old partErrors: the badges, "n parts need attention"). */
  private partErrors(pi: number): number {
    return this.rowIds.filter((id) => id.startsWith(`${pi}:`) && (this.st.messages[id] || this.st.dateErrors[id] || this.boxCountFor(id) > 0)).length;
  }

  /** The to-do list (old todo()): every row with a message counts, whether its text shows yet or not. */
  private todoNow(): TodoRow[] {
    const s = this.st;
    if (!this.schema || !s.draft) return [];
    return todoRows({
      parts: this.schema.parts as unknown as TodoPart[],
      messages: { ...s.messages, ...Object.fromEntries(this.rowIds.filter((id) => this.boxCountFor(id) > 0 && !s.messages[id]).map((id) => [id, " "])) },
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
    if (!this.canAutosave() || this.st.periodIncomplete || this.removalUndo) {
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
    if (!this.canAutosave() || this.st.periodIncomplete) return this.publish();
    this.autosave();
  }

  private autosave() {
    const s = this.st;
    if (!this.canAutosave() || s.periodIncomplete) {
      if (s.periodIncomplete) {
        s.saveState = this.restingSaveState();
        this.publish();
      }
      return;
    }
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
      // B19: a refused edit was never in the draft; the return still has it unsaved. An unfixable
      // one (shut/locked) put the stored value back, so it does not count (R7).
      if (Object.keys(s.rejected).some((k) => !s.unfixable.has(k))) s.dirty = true;
      s.saveState = s.dirty ? this.restingSaveState() : { state: "saved" };
      if (changed) this.hooks.onSaved?.(s.target);
      if (announce) this.toast("Draft saved.", "save");
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
    this.commitHeldPeriod();
    if (s.periodIncomplete) {
      s.saveState = this.restingSaveState();
      return this.askSamplePeriod("", then);
    }
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
    // B19: a refused edit is still unsaved work; Queue waits for it to be fixed or cleared. An
    // unfixable one (shut/locked) put the stored value back, so it does not hold Queue (R7).
    if (Object.keys(s.rejected).some((k) => !s.unfixable.has(k))) {
      s.showAll = true;
      this.publish();
      // R6: Queue refuses like a failed Check — the count of refused boxes and messages, not one sentence.
      const n = Object.values(this.shownCounts()).reduce((a, b) => a + b, 0);
      return this.toast(attentionText(n), "triangle-alert");
    }
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
      return this.refuse(check.blocks[0].message, this.refusalAction(check.blocks));
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
        return void this.refuse(attentionText(n), this.refusalAction(s.errors));
      }
      this.toldRefusal = "";
      this.askQueue(warns);
    });
  }

  /** The list a Queue was last told of instead of being asked ("" when none): the same list again goes to the host. */
  private toldRefusal = "";

  /** The refusal toast still up (its Show item / Open tax profile), if any. */
  private refusalToast: string | null = null;

  /** A Queue refusal: its toast replaces the last one, whose action would point at the old list. */
  private refuse(message: string, action: ToastAction | undefined): string | null {
    this.retireRefusal();
    this.refusalToast = this.toast(message, "triangle-alert", action);
    return this.refusalToast;
  }

  /**
   * Queue goes on (the question or the host): the refusal toast goes. Left up, it would float over the question's dialog,
   * whose scrim takes a press on its Show item and closes the dialog instead (seen on iOS).
   */
  private retireRefusal() {
    const id = this.refusalToast;
    this.refusalToast = null;
    if (id) this.hooks.dismissToast?.(id);
  }

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
    this.retireRefusal();
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
      const message = errorText(e);
      const refusal = plainRefusal(message);
      const count = listedFields(refusal);
      const shown = new Set(s.errors
        .filter((error) => !error.dismiss && !isPageNotice(s.target.formCode, error.field)
          && Object.values(s.messages).some((text) => text.includes(error.message)))
        .map((error) => `${error.field}: ${error.message}`));
      const alreadyShown = count > 0 && refusal.split(/;\s+(?=[a-z][a-z0-9_.]*: )/).every((entry) => shown.has(entry));
      const errors = count ? refusal.split(/;\s+(?=[a-z][a-z0-9_.]*: )/).map((entry) => {
        const at = entry.indexOf(": ");
        const field = entry.slice(0, at);
        const error = { field, message: entry.slice(at + 2), keys: s.errors.find((old) => old.field === field)?.keys || [field] };
        const row = this.refusalRow(error);
        if (row && !error.keys.some((key) => this.keyIndex.has(key))) error.keys = keysOf(this.items.get(row)!);
        return error;
      }) : [];
      s.showAll = true;
      const mapped = errors.filter((error) => this.refusalRow(error));
      const fields = new Set(mapped.map((error) => error.field));
      this.setErrors([...s.errors.filter((old) => !fields.has(old.field) && !s.factErrors.includes(old)), ...mapped]);
      this.publish();
      this.refuse(alreadyShown ? attentionText(count) : message, this.refusalAction(errors));
    }
  }

  private refusalRow(error: ScreenFormError): string | null {
    return bindError(error, this.rowIds.map((id) => ({ id, it: this.items.get(id)! })), (key) => this.keyIndex.get(key), false);
  }

  private refusalAction(errors: ScreenFormError[]): ToastAction | undefined {
    const sections: Record<string, string> = {
      annual_income_tax_election: "income_tax_election",
      income_tax_election: "income_tax_election",
      tax_classification: "tax_classification",
      eopt_tier: "eopt_tier",
      is_government_withholding_entity: "is_government_withholding_entity",
      atc_codes: "atc_codes",
      business_start_date: "branch",
      line_of_business: "branch",
      rdo_code: "branch",
      tin: "taxpayer",
      taxpayer_type: "taxpayer",
      name: "taxpayer",
      taxpayer_name: "taxpayer",
      registered_name: "taxpayer",
      birth_date: "taxpayer",
      registered_address: "address",
      zip_code: "address",
      phone: "address",
      email: "address",
    };
    for (const error of errors) {
      const field = error.field.replace(/^tax_profile\./, "");
      const profileOnly = error.field.startsWith("tax_profile.") || field === "annual_income_tax_election";
      const row = this.refusalRow(error);
      if (!profileOnly && row && this.st.editable && keysOf(this.items.get(row)!).some((key) =>
        !this.isShut(key) || (this.st.profileEdit.has(key) && !this.st.off[key]))) {
        return { label: "Show item", onPress: () => this.jumpTo(Number(row.split(":")[0]), row) };
      }
      const profile = profileOnly
        || (error.keys.length > 0 && error.keys.every((key) => this.st.profile.has(key)));
      if (profile) {
        const section = Object.prototype.hasOwnProperty.call(sections, field) ? sections[field] : undefined;
        if (section && this.hooks.onOpenProfile)
          return { label: "Open tax profile", onPress: () => this.hooks.onOpenProfile?.(section, this.taxYear()) };
        continue;
      }
      if (row) return { label: "Show item", onPress: () => this.jumpTo(Number(row.split(":")[0]), row) };
    }
    return undefined;
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
    if (!hasSecondWindow && !hasNativePreview && !Print.previewsNatively()) return void this.toast(NO_PREVIEW, "printer");
    if (s.sample) {
      try {
        bir.screen_form_print_sample(s.target.formCode, s.draft, s.pending, this.unlockedFor(s.pending));
        s.previewOpen = !hasNativePreview;
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
      s.previewOpen = !hasNativePreview;
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
      shown: Object.fromEntries([...this.allKeys, ...this.oneDates.keys(), ...this.onePeriods.keys()].map((k) => [k, this.shownOf(k)])),
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
    s.periodIncomplete = false;
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
    s.periodHeld = null;
    s.periodIncomplete = false;
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
      const todo = this.todoNow();
      this.publishRows(todo);
      this.publishPage(todo);
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
      // A box's sentence follows the same rule as a row's: after its row was left, or after Check (a refused
      // edit's row is touched when the refusal lands, so its sentence still shows at once).
      const boxMsg = this.shownBoxMsg(k);
      this.sInvalid.get(k).set(shownMsgKeys.has(k) || Boolean(boxMsg));
      // A box whose sentence the row prints draws none of its own.
      this.sBoxMsg.get(k).set(this.rowPrints(k) ? "" : boxMsg);
    }
    for (const [k, { date, row }] of this.oneDates) {
      const parts = [date.month, date.day, date.year].filter(Boolean) as SchemaField[];
      const shut = this.isShut(k);
      this.sLocked.get(k).set(shut);
      this.sHint.get(k).set(!oneDateText(s.values, date) ? parts.map((f) => this.sHint.get(f.k)()).find(Boolean) || "" : "");
      const open = s.showAll || s.touched.has(row);
      const boxMsg = open && parts.some((f) => Boolean(s.boxMessages[f.k]));
      const rowMsg = open && Boolean(s.dateErrors[row] || parts.some((f) => shownMsgKeys.has(f.k)));
      this.sInvalid.get(k).set(boxMsg || rowMsg);
    }
    for (const [k, { period, row }] of this.onePeriods) {
      const parts = [period.month, period.year];
      const shut = this.isShut(k);
      this.sLocked.get(k).set(shut);
      this.sHint.get(k).set(!periodText(String(s.values[period.month.k] ?? ""), String(s.values[period.year.k] ?? "")) ? parts.map((f) => this.sHint.get(f.k)()).find(Boolean) || "" : "");
      const open = s.showAll || s.touched.has(row);
      const boxMsg = open && parts.some((f) => Boolean(s.boxMessages[f.k]));
      const rowMsg = open && Boolean(s.dateErrors[row] || parts.some((f) => shownMsgKeys.has(f.k)));
      this.sInvalid.get(k).set(boxMsg || rowMsg);
    }
  }

  private publishRows(todo: TodoRow[]) {
    const s = this.st;
    if (!this.schema) return;
    const shown = this.shownMessages();
    const tucked = this.schema.parts.map((_p, pi) => (s.profileOpen.has(pi) ? [] : this.tuckedItems(pi)));
    // A row on the to-do list is never gated away: the pill and the badges keep pointing at a drawn row.
    const todoIds = new Set(todo.map((r) => r.id));
    for (const id of this.rowIds) {
      const [pi, ii] = id.split(":").map(Number);
      this.sRow.get(id).set(this.rowInfo(id, this.items.get(id)!, shown, tucked[pi].includes(ii), todoIds.has(id)));
    }
  }

  private rowInfo(id: string, it: SchemaItem, shown: Record<string, string>, tucked: boolean, todoHere: boolean): RowInfo {
    const s = this.st;
    const fs = it.f || [];
    const keys = keysOf(it);
    const tin = isTinItem(it);
    // G1: the drawn box keys another answer keeps shut (a schedule's cells are never hidden on their own).
    const hiddenBoxes = it.rows ? [] : this.drawnKeys(it).filter((k) => this.hiddenBox(k));
    const editKeys = fs.filter((x) => s.profileEdit.has(x.k)).map((x) => x.k);
    const fromProfile = (tin && !it.typed_tin) || fs.some((x) => s.profile.has(x.k));
    const kept = keys.map((k) => s.kept[k]).filter(Boolean);
    const penalty = s.editable ? fs.map((f) => this.penaltyGroupOf(f.k)).find((g) => g && g.applies) || null : null;
    const penaltyOpen = Boolean(penalty) && (penalty!.edited || penalty!.keys.some((k) => s.penaltyOpen.has(k)));
    const stopBoxes = this.stopBoxes(it);
    const editingHere = editKeys.some((k) => s.editing.has(k));
    let from = "";
    if (it.from && s.editable && !this.isShut(it.from.to)) {
      const pesos = wholePesosOf(s.values[it.from.k]);
      if (pesos) from = `${it.from.l}: ₱${formatMoney(String(pesos.amount), true)}${pesos.rounded ? " (rounded to whole pesos)" : ""}`;
    }
    const date = it.f ? dateParts(it.f) : null;
    // The boxes whose sentences the row prints as lines above its own message (rowPrints).
    const groupKeys = new Set<string>(keys.filter((k) => this.rowPrints(k)));
    // A split date's parts are one date: its sentence shows once, on the month box. Box sentences show
    // under the same rule as the row's own: after the row was left, or after Check.
    const msgOpen = s.showAll || s.touched.has(id);
    const dateMsgs =
      msgOpen && date
        ? [date.month.k, date.day?.k, date.year.k].filter((pk): pk is string => pk !== undefined && Boolean(s.boxMessages[pk]))
        : [];
    // R5: a split date shows every part's sentence in box order, dropping exact duplicates.
    const seenDate = new Set<string>();
    const dateShown = dateMsgs.filter((k) => {
      const sentence = s.boxMessages[k];
      if (seenDate.has(sentence)) return false;
      seenDate.add(sentence);
      return true;
    });
    const registered = oneDates(it);
    const grouped = new Set(registered.flatMap((d) => [d.month.k, d.day?.k, d.year.k].filter((k): k is string => Boolean(k))));
    const periodRegistered = onePeriods(it);
    const periodGrouped = new Set(periodRegistered.flatMap((p) => [p.month.k, p.year.k]));
    const oneDateShown = registered.flatMap((d) => {
      const seen = new Set<string>();
      return [d.month.k, d.day?.k, d.year.k].filter((k): k is string => {
        if (!k || !s.boxMessages[k] || seen.has(s.boxMessages[k])) return false;
        seen.add(s.boxMessages[k]);
        return true;
      });
    });
    const periodShown = periodRegistered.flatMap((p) => {
      const seen = new Set<string>();
      return [p.month.k, p.year.k].filter((k) => {
        if (!s.boxMessages[k] || seen.has(s.boxMessages[k])) return false;
        seen.add(s.boxMessages[k]);
        return true;
      });
    });
    // Boxes of one row refused for the same reason (a shut part's four TIN boxes) print that sentence once.
    const said = new Set<string>();
    const firstSaying = (k: string) => !said.has(s.boxMessages[k]) && Boolean(said.add(s.boxMessages[k]));
    const shownKeys = !msgOpen ? [] : registered.length || periodRegistered.length
      ? [...oneDateShown, ...periodShown, ...keys.filter((k) => !grouped.has(k) && !periodGrouped.has(k) && s.boxMessages[k] && groupKeys.has(k))]
      : date && dateShown.length ? dateShown : keys.filter((k) => s.boxMessages[k] && groupKeys.has(k) && firstSaying(k));
    const asks =
      s.editable && !s.sample
        ? (s.askAt.get(id) || []).map((a) => ({
            fact: a.fact,
            label: a.label,
            note: a.note,
            control: a.control,
            choices: a.choices || [],
            pick: a.fact in s.askPick ? JSON.stringify(s.askPick[a.fact]) : null,
            done: s.askDone.has(a.fact),
          }))
        : [];
    // B10: an "Additional details" fold stays open while the filer opened it or a shown message,
    // fact card or to-do step points inside; it folds back when nothing does.
    const pointed =
      it.l === "Additional details" &&
      (Boolean(shown[id]) || keys.some((k) => Boolean(this.shownBoxMsg(k))) || asks.length > 0 || s.stepAt === id);
    return {
      message: oneDateShown.length || periodShown.length ? "" : shown[id] || "",
      boxMessages: shownKeys.map((k) => ({ key: k, sentence: s.boxMessages[k] })),
      hidden: this.rowHidden(id, it, todoHere, Boolean(shown[id])),
      hiddenBoxes,
      notes: this.lockNotesFor(it),
      // A gated-away box's "Needs …" line is not drawn either; a schedule keeps every cell's (its grid stays).
      needs: [...new Set(keys.filter((k) => it.rows || !this.hiddenBox(k)).map((k) => s.off[k]).filter(Boolean))],
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
      // The line under a TIN's boxes (PR 8): a TIN with every box locked — the profile's, not a typed party's
      // (typed_tin) and not one still open for Edit (an open Edit box is not locked). s.profile lists only the
      // profile boxes Edit may open, so the locked state itself is the test, not profile membership.
      tinLine: tin && !it.typed_tin && fs.length > 0 && fs.every((x) => this.isLocked(x.k)),
      canEdit: editKeys.length > 0 && s.editable,
      editing: editingHere,
      profileAsk: s.profileAskMsg[id] ?? null,
      penalty: penalty ? (penaltyOpen ? "computed" : "edit") : null,
      tucked,
      moreOpen: s.moreOpen.has(id) || pointed,
      asks,
      askSaved: s.askSavedAt.get(id) || "",
      election: s.editable && s.electionAt === id ? this.electionCard() : null,
      electionSaved: s.electionSavedAt === id && s.electionAt !== id ? `Saved to the tax profile for ${this.taxYear()}. Later returns reuse it.` : "",
      checkable: s.editable ? s.checkable[id] || [] : [],
      from,
    };
  }

  private publishPage(todo: TodoRow[]) {
    const s = this.st;
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
          items: p.items.filter((_it, ii) => !this.sRow.get(`${pi}:${ii}`)().hidden).length,
          open: s.openParts.has(pi),
          fold: tucked.length
            ? {
                first: tucked[0],
                items: tucked,
                label: `From tax profile · ${filled} filled box${filled === 1 ? "" : "es"}`,
                flagged: tucked.filter((ii) => s.messages[`${pi}:${ii}`] || s.dateErrors[`${pi}:${ii}`] || this.boxCountFor(`${pi}:${ii}`) > 0).length,
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
      profilePrerequisites: s.editable ? s.profilePrerequisites : [],
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
export function bindError(e: ScreenFormError, items: { id: string; it: SchemaItem }[], idForKey: (k: string) => string | undefined, fallback = true): string | null {
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
  return id || (fallback && items[0] && items[0].id) || null;
}
