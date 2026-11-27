// The form page: one renderer for all 43 forms (the old shell's form_page.js). The session (./session.ts) owns the
// state; this file draws it from nsh/ui parts and the field controls of ./controls.
//
// The page paints no background of its own: it sits on the surface it is mounted on (the frame's card).
// Layout. Wide (the page 760 points and up): the pinned 208-point "On this form" navigator beside the form, which
// scrolls on its own. Narrow: one scroll, rows stacked. At every width the floating pill steps the to-dos (‹ ›,
// Cmd/Ctrl+Left/Right); its section list is a popover when wide and a bottom sheet under 600.
// Part titles stick to the top of the scroll while their part shows (class "sticky": NSH's position sticky, which
// keeps a view at the top of the scroll view around it while its parent is in view; website/modules/tailwind-reference.md).
// Rendering cost: a heavy annual form expanded (1701, 1700, 1702RT) mounted about 10,000 views and every frame of
// scrolling paid for all of them (100-point steps took 650 ms against 170 ms on a 2551Q). Each open part's rows are a
// List in the page's viewport (viewport="parent", website/docs/lists.md "Lists in a page"): only the rows near the
// viewport exist and their cells are recycled, while the part cards, their sticky titles, the header and the notes stay
// plain children of the one Scroll. A row's state lives in the session's keyed signals (s.row(id), s.box(k)); a cell
// given another item draws a new row (an Each keyed by the item), so nothing created in a row moves to another item.
// A row holding the keyboard focus or an open popover stays bound wherever the page scrolls (lists' kept rows). A step
// to a row not drawn yet asks its part's list to scroll to it (scrollToIndex), then measures and focuses it.
// Lists that scrolled far away keep their recycled cells (hidden, still views), so a part's list exists only while its
// card is within a viewport above or two below the one on screen (or holds the keyboard); elsewhere an open part keeps
// its rows' last measured height as an empty box, and its list is made again as it comes near.
// The pill names the part at the top of the viewport: the scroll offset against the part cards' laid-out tops.
import type { Accessor, ListRef, ScrollRef, ViewRef } from "nsh";
import { cleanup, computed, effect, keymap, List, openUrl, Scroll, signal, untracked, View, windowSize } from "nsh";
import { BREAK, SIZE } from "../../theme/tokens";
import {
  AppButton,
  CheckboxLabel,
  Chip,
  Code,
  Dialog,
  Icon,
  type IconName,
  IconButton,
  ICONS,
  Line,
  ListRow,
  MetaLine,
  Popover,
  PressCell,
  Sheet,
  Spinner,
  Text,
  TextField,
  phoneLayout,
  TOUCH,
  toast as kitToast,
  dismissToast as kitDismissToast,
} from "../../ui";
import { app, store } from "../../shell";
import { returnedFromAccountSite } from "../taxpayer/tins";
import { DRY_RUN_WORD } from "../../lib/statusWord";
import { ItemRow, ProfileFold } from "./rows";
import { ACCOUNT_FIX, existingOfferFor, filedOriginalStrip, FILED_KEPT, PERIOD_PART_LABELS, profileChangedNote, partName, profileDiffersNote, profileUpdatesNote, SAMPLE_NOTE, sectionNames } from "./schema";
import { formUi } from "./formUi";
import { FormSession, type FormTarget, type Overlay, type SessionHooks, type ToastAction } from "./session";

export type { FormTarget } from "./session";
/** Every open form page's typing flushed and saved now; the frame awaits it before quit or leaving the page. */
export { flushNow } from "./session";

/** What the frame mounts. The frame's Toaster should sit above the pill while this page shows (bottom 96). */
export interface FormPageProps {
  tin: string;
  formCode: string;
  year: number;
  period: number;
  /** The stored return's exact key (a transaction's MMDD, 2553's quarter). */
  key?: number | null;
  mode: FormTarget["mode"];
  /** filed_original: the history snapshot. */
  snapshotId?: number;
  /** The page wants to go (nothing calls it yet: the frame's own back does). */
  onClose?: () => void;
  /** The page now shows another return (a sample became its return, an offer loaded one): the frame follows it. */
  onOpenReturn?: (target: FormTarget) => void;
  /** Open the taxpayer's profile (the frame's panel). */
  onOpenProfile?: (section?: string, year?: number) => void;
  /** A toast; the kit's toast (needs a Toaster mounted) when not given. */
  toast?: (message: string, options?: { icon?: string; action?: ToastAction }) => string | void;
  /** Close a toast this page showed (with props.toast); the kit's dismissToast when not given. */
  dismissToast?: (id: string) => void;
  /** A changed return was saved (Recent). */
  onSaved?: (target: FormTarget) => void;
  /** Tests: the session the page made. */
  onSession?: (session: FormSession) => void;
}

const PILL_W = 488;
const NARROW_PILL_HEIGHT = 60;
// The pill's height, its 16 pt bottom gap and 12 pt of air: what a focused row keeps below it.
const PILL_CLEAR = NARROW_PILL_HEIGHT + 16 + 12;
/** The old shell's cap on a form page's column, padding included (app/main.js layout(): 1120 for "form"). */
const FORM_COLUMN_MAX = 1120;

function kitIcon(name: string | undefined): IconName | undefined {
  return name && name in ICONS ? (name as IconName) : undefined;
}

export function FormPage(props: FormPageProps) {
  const hooks: SessionHooks = {
    toast: (message, o) => {
      if (props.toast) return props.toast(message, o);
      return kitToast(message, { icon: kitIcon(o?.icon), action: o?.action });
    },
    dismissToast: (id) => (props.toast ? props.dismissToast?.(id) : kitDismissToast(id)),
    // A write through the door: every cached read of the frame (Filings, Home, the profile) reads again.
    afterWrite: (name) => store.bump(name),
    onSaved: (t) => props.onSaved?.(t),
    onRetarget: (t) => props.onOpenReturn?.(t),
    onOpenProfile: (section, year) => props.onOpenProfile ? props.onOpenProfile(section, year) : app.go("profile", { edit: section, year, fromForm: true }),
    // A queue or a cancel-queue needs no hook of its own: the session tells afterWrite its host function's name and store.bump(name) starts the
    // frame's fast Queued watch over (the re-reads at once and after 1, 3 and 10 s, and the watch's 30 s of 3 s looks from store.lastQueueAction).
  };
  const s = new FormSession(
    { tin: props.tin, formCode: props.formCode, year: props.year, period: props.period, key: props.key ?? null, mode: props.mode, snapshotId: props.snapshotId },
    hooks,
  );
  props.onSession?.(s);
  void s.open().then(() => s.notesDrawn());
  cleanup(() => s.dispose());

  // The submission worker sends a Queued return and tells nobody. The frame reads its lists again (its Queued watch and polls, and a queue
  // action at once and after 1, 3 and 10 s), so while this page says Queued it looks at what the lists hold: once they no longer list this
  // return as Queued, the session reads it again (the page stays drawn) and shows what is stored. The lists are read only while the page says
  // Queued, nothing here asks the host itself or runs a timer, and the effect ends with the page. A page the lists keep disagreeing with
  // reads once: `behind` is a boolean, so only its change moves the effect, not each look of the frame at the lists.
  const shownQueued = computed(() => s.queuedShown());
  const behind = computed(() => shownQueued() && !store.returns("filed").some((r) => r.status === "Queued" && s.isReturn(r.id)));
  effect(() => {
    if (behind()) untracked(() => s.refreshQueued());
  });

  const width = signal(untracked(() => windowSize().width));
  // The old shell's measures (app/main.js layout(), form_page.js drawFormSplit and formColumnBox), from the page's own
  // width (the pane it fills): 32 side padding (16 under 640), a column of at most 1120 less that padding, the side
  // list from 760 of column, set in by the column's centring edge, and the form 22 beside the list, at most
  // column - 236 wide (820 for a column of 1056), its text starting at the left of its scroll.
  const padX = () => (width() < BREAK.narrowContent ? 16 : 32);
  // A touch screen always has the phone form (the owner's tablet pick): no side list, the bottom pill, the form in one column, which
  // on a tablet keeps a page's readable width (the frame's page column, 880 with its padding) and stays centred.
  const column = () =>
    TOUCH
      ? Math.max(200, Math.min(SIZE.contentMax, width()) - padX() * 2)
      : Math.max(200, Math.min(FORM_COLUMN_MAX, width()) - padX() * 2);
  const edge = () => Math.max(padX(), Math.floor((width() - column()) / 2));
  const formW = () => column() - 208 - 28;
  const wide = () => !TOUCH && column() >= BREAK.formNav;
  const narrow = () => TOUCH || width() < BREAK.phone;

  // Not while a text box has the keys: there Cmd/Ctrl+Left/Right move the caret (index.d.ts KeyBindingOptions "notEditing").
  keymap.bind("mod+left", () => s.step(-1), { when: "notEditing" });
  keymap.bind("mod+right", () => s.step(1), { when: "notEditing" });
  // The frame's palette steps through this too ("Next to do", "Previous to do"), while the pill shows.
  cleanup(
    formUi.attach({
      pillShown: () => Boolean(s.schema) && !s.header().error && !s.header().loading,
      step: (dir) => s.step(dir),
      // What an agent's commands ask (shell/agent): which return this is, when it has loaded, a fill committed as a choice.
      target: () => ({ tin: props.tin, formCode: props.formCode, year: props.year, period: props.period }),
      ready: () => s.ready(),
      choose: (edits, clear) => s.choose(edits, clear),
    }),
  );

  // A step: once its row is drawn, scroll it near the top (under the sticky title) and put the keyboard in it.
  // A part the navigator or a part step went to: its card's top to the top of the viewport.
  const rowRefs = new Map<string, ViewRef>();
  const lists = new Map<number, PartList>();
  const cards = new Map<number, ViewRef>();
  // Where the part cards start in the scroll's content: the column's top, the parts box's top in it, each card's in that.
  const tops = { column: 0, parts: 0, cards: [] as number[], heights: [] as number[] };
  let scrollRef: ScrollRef | undefined;
  let scrollY = 0;
  let viewH = untracked(() => windowSize().height);
  // The parts whose rows are drawn now ("0,1,2"): near the viewport, or holding the keyboard.
  const near = signal("");
  // A part's rows come when its card is within a viewport above or two below; they go only past three above or four
  // below, so a part whose height changes as its rows are measured never flips between drawn and not drawn.
  const updateNear = () => {
    const base = tops.column + tops.parts;
    const keep = s.focusedPart();
    const now = new Set(untracked(near).split(",").filter(Boolean).map(Number));
    const within = (pi: number, above: number, below: number) => {
      const top = base + tops.cards[pi];
      return top <= scrollY + below * viewH && top + (tops.heights[pi] ?? 0) >= scrollY - above * viewH;
    };
    const shown = tops.cards
      .map((_top, pi) => (pi === keep || (now.has(pi) ? within(pi, 3, 4) : within(pi, 1, 2)) ? pi : -1))
      .filter((pi) => pi >= 0)
      .join(",");
    if (shown !== untracked(near)) near.set(shown);
  };
  const isNear = (pi: number) => near().split(",").includes(String(pi));
  const followScroll = (y: number) => {
    scrollY = y;
    let at = 0;
    tops.cards.forEach((top, pi) => {
      if (tops.column + tops.parts + top <= y + 1) at = pi;
    });
    s.setCurrent(at);
    updateNear();
  };
  effect(() => {
    const r = s.reveal();
    if (r) untracked(() => void reveal(r.row, r.part, r.seq));
  });
  async function reveal(row: string | null, part: number, seq: number) {
    let asked = false;
    let at: number | null = null;
    let scrolls = 0;
    for (let tries = 0; tries < 40; tries++) {
      await new Promise((done) => setTimeout(done, 50));
      if (s.reveal()?.seq !== seq) return;
      if (!scrollRef) continue;
      const ref = row ? rowRefs.get(row) : cards.get(part);
      if (!ref) {
        // A row not drawn yet: its part's list scrolls the page to it with the sizes it knows, and draws it there.
        // A part far away has no list yet: the page goes to its card first, which brings the list.
        const list = row ? lists.get(part) : undefined;
        const index = row && list ? list.indexOf(row) : -1;
        if (list && index >= 0 && !asked) {
          list.ref.scrollToIndex({ index, viewOffset: 72, animated: false });
          asked = true;
        } else if (row && !list && tops.cards[part] !== undefined) {
          scrollRef.scrollTo({ y: tops.column + tops.parts + tops.cards[part], animated: false });
        }
        continue;
      }
      const [m, area] = await Promise.all([ref.measure(), scrollRef.measure()]);
      if (!m || !area) continue;
      const to = () => Math.max(0, scrollY + m.pagePosition.y - area.pagePosition.y - (row ? 72 : 0));
      if (row) {
        scrollRef.scrollTo({ y: to(), animated: true });
        s.focusRow(row);
        return;
      }
      // A part: the parts that were open fold as this one opens and its own rows are still being laid out, so the card
      // keeps moving after the first measure (seen in the iOS Simulator: the page stopped below the fold, or in the middle
      // of the card). Scroll only once it rests, and again while it rests away from the top.
      if (at === null || Math.abs(m.pagePosition.y - at) > 0.5) {
        at = m.pagePosition.y;
        continue;
      }
      if (Math.abs(m.pagePosition.y - area.pagePosition.y) < 1 || scrolls === 3) return;
      scrollRef.scrollTo({ y: to(), animated: scrolls === 0 });
      scrolls += 1;
      at = null;
    }
  }

  // A box that takes the keyboard under the floating pill (Tab into it) is scrolled clear of it: the pill sits 16 pt above
  // the pane's bottom and covers what lies behind it.
  effect(() => {
    const f = s.focusedRow();
    if (f) untracked(() => void clearOfPill(f.row, f.seq));
  });
  async function clearOfPill(row: string, seq: number) {
    const ref = rowRefs.get(row);
    if (!ref || !scrollRef) return;
    const [m, area] = await Promise.all([ref.measure(), scrollRef.measure()]);
    if (!m || !area || s.focusedRow()?.seq !== seq) return;
    const covered = m.pagePosition.y + m.bounds.size.height - (area.pagePosition.y + area.bounds.size.height - PILL_CLEAR);
    if (covered > 0) scrollRef.scrollTo({ y: scrollY + covered, animated: false });
  }

  const schema = s.schema;
  return (
    <View testID="form-page" class="min-h-0 flex-1 flex-row" onLayout={(e) => width.set(e.bounds.size.width)}>
      <If when={wide() && schema && !s.header().error && !s.header().loading}>
        <Navigator s={s} edge={edge()} />
      </If>
      <View class="min-h-0 min-w-0 flex-1">
        <Scroll
          ref={(r: ScrollRef) => (scrollRef = r)}
          testID="form-scroll"
          keyboardShouldPersistTaps="handled"
          class="min-h-0 flex-1"
          contentClass={wide() ? "pt-7 pr-7.5 pb-28 pl-5.5" : narrow() ? "px-4 pt-5 pb-32" : "px-8 pt-7 pb-28"}
          onScroll={(e) => followScroll(e.offset.y)}
          onLayout={(e) => {
            viewH = e.bounds.size.height;
            updateNear();
          }}
          scrollEventThrottle={50}
        >
          <View class="w-full" classList={{ "self-center": !wide() }} style={{ maxWidth: wide() ? formW() : column() }} onLayout={(e) => (tops.column = e.bounds.origin.y)}>
            <Header s={s} narrow={narrow} />
            <Notes s={s} />
            <If when={schema && !s.header().loading && !s.header().error}>
              <View class="gap-4" onLayout={(e) => (tops.parts = e.bounds.origin.y)}>
                {(schema?.parts || []).map((_p, pi) => (
                  <PartCard
                    s={s}
                    pi={pi}
                    narrow={narrow}
                    onRef={(id, ref) => rowRefs.set(id, ref)}
                    onUnref={(id, ref) => rowRefs.get(id) === ref && rowRefs.delete(id)}
                    onList={(list) => (list ? lists.set(pi, list) : lists.delete(pi))}
                    onCard={(ref) => cards.set(pi, ref)}
                    near={() => isNear(pi)}
                    onTop={(y, h) => {
                      tops.cards[pi] = y;
                      tops.heights[pi] = h;
                      updateNear();
                    }}
                  />
                ))}
              </View>
            </If>
            <If when={s.header().loading && schema}>
              <View class="items-center py-10">
                <Text tone="fg3">{`Opening ${props.formCode}…`}</Text>
              </View>
            </If>
            <If when={s.header().error}>
              <View testID="fp-error" class="rounded-(--radius-card) bg-danger-soft p-4">
                <Text tone="danger">{s.header().error}</Text>
              </View>
            </If>
            <If when={narrow()}>
              <View testID="fp-pager-clearance" class="shrink-0" style={{ height: NARROW_PILL_HEIGHT + 16 + 16 }} />
            </If>
          </View>
        </Scroll>
        <If when={schema && !s.header().error && !s.header().loading}>
          <Pill s={s} wide={wide} narrow={narrow} box={() => (wide() ? { left: 22, width: formW() } : null)} />
        </If>
      </View>
      <Dialogs s={s} />
    </View>
  );
}

// ---------------------------------------------------------------- header

function SaveLine(props: { s: FormSession }) {
  const st = () => props.s.saveState();
  const text = () => {
    const v = st();
    if (v.state === "saving") return "Saving…";
    if (v.state === "saved") return "✓ Saved";
    if (v.state === "unsaved") return "Unsaved changes";
    if (v.state === "failed") return v.message;
    if (v.state === "manual") return v.message;
    return "";
  };
  return (
    <Text testID="fp-save-state" variant="meta" tone={st().state === "failed" ? "danger" : st().state === "saved" ? "success" : "fg3"} lines={2} class="max-w-80">
      {text()}
    </Text>
  );
}

function Header(props: { s: FormSession; narrow: Accessor<boolean> }) {
  const s = props.s;
  const f = s.schema;
  const h = () => s.header();
  if (!f) {
    return (
      <View class="mb-5 gap-1">
        <Text variant="page">{`Form ${s.target().formCode}`}</Text>
      </View>
    );
  }
  const statusLabel = () => (h().editable ? h().statusWord : `${h().statusWord} · read-only`);
  return (
    <View testID="fp-header" class="mb-5 flex-row flex-wrap items-start gap-3.5">
      <View class="min-w-0 flex-1" style={{ minWidth: props.narrow() ? 0 : 420 }}>
        <MetaLine parts={[<Code>{`BIR Form ${f.c}`}</Code>, `Revision ${f.rev}`]} />
        <Text variant="page" class="mt-1">{f.t}</Text>
        <View class="mt-2 flex-row flex-wrap items-center gap-2">
          <Chip label={f.freq} />
          <Chip label={`${f.pages} page${f.pages > 1 ? "s" : ""}`} />
          <Chip label={`${s.fieldCount()} fields`} />
          <If when={h().status !== "Draft"}>
            <Chip testID="fp-status" label={statusLabel()} tone={h().status === "Queued" ? "accent" : "success"} />
          </If>
          <If when={h().amends}>
            <Chip label="Amended return" tone="warning" />
          </If>
          <If when={h().filedOriginalView}>
            <Chip label="Filed original · read-only" tone="success" />
          </If>
          <If when={h().sample}>
            <Chip label="Sample" tone="accent" />
          </If>
          <If when={h().loading}>
            <Chip label="Opening…" tone="accent" />
          </If>
          <If when={formUi.reviewing()}>
            <Chip testID="fp-agent-review" label="Prepared by the assistant — review, then Queue" tone="accent" />
          </If>
        </View>
      </View>
      <View class="items-end gap-1.5" classList={{ "w-full items-start": props.narrow() }}>
        <View class="flex-row flex-wrap items-center gap-2">
          <AppButton testID="fp-print" size="sm" icon="printer" label="Print preview" disabled={h().loading} onPress={() => s.print()} />
          <If when={h().filedOriginal}>
            <AppButton testID="fp-filed-original" size="sm" icon="history" label="View the filed original" onPress={() => s.openFiledOriginal()} />
          </If>
          <If when={h().filedOriginalView}>
            <AppButton testID="fp-back-to-amendment" size="sm" label="Back to the amendment" onPress={() => s.backToAmendment()} />
          </If>
          <If when={h().editable && !h().filedOriginalView}>
            <AppButton
              testID="fp-check"
              size="sm"
              label="Check"
              accessibilityLabel={s.checking() ? "Check — validation in progress" : "Check"}
              leading={
                <View class="size-4 items-center justify-center">
                  <If when={s.checking()} else={<Icon name="circle-check" />}>
                    <Spinner size={14} tone="fg2" label="Checking" />
                  </If>
                </View>
              }
              onPress={() => s.check()}
            />
            <AppButton testID="fp-queue" size="sm" icon="send" label="Queue" variant={formUi.reviewing() ? "primary" : undefined} disabled={h().loading} onPress={() => s.queue()} />
          </If>
          <If when={h().status === "Queued" && !h().sample && !h().canResolve && !h().checkingSend}>
            <AppButton testID="fp-cancel-queue" size="sm" label="Cancel queue" onPress={() => s.cancelQueue()} />
          </If>
          <If when={h().editable && h().sample && !h().periodSet}>
            <AppButton testID="fp-save" variant="primary" size="sm" label="Save as return…" onPress={() => s.saveNow()} />
          </If>
          <If when={h().amendUnsaved}>
            <AppButton testID="fp-save" variant="primary" size="sm" label="Save amendment" onPress={() => s.saveNow()} />
          </If>
        </View>
        <SaveLine s={s} />
      </View>
    </View>
  );
}

/** The lines under the header: the sample note, profile changes, one-time notes, amendment and filed strips. */
function Notes(props: { s: FormSession }) {
  const s = props.s;
  const h = () => s.header();
  return (
    <View class="gap-4" classList={{ "mb-4": Boolean(h()) }}>
      <If when={h().sample}>
        <Text testID="fp-sample-note" variant="meta" tone="fg2">{TOUCH ? SAMPLE_NOTE.replace("the + button or Cmd/Ctrl+N", "the + button") : SAMPLE_NOTE}</Text>
      </If>
      <If when={h().profileUpdates.length}>
        <Text variant="meta" tone="fg2">{profileUpdatesNote(h().profileUpdates)}</Text>
      </If>
      <Each of={h().notes}>{(note) => <Text variant="meta" tone="fg2">{note()}</Text>}</Each>
      <Each of={h().profilePrerequisites} key={(item) => item.field}>
        {(item) => (
          <View testID={`fp-profile-prerequisite-${item().field}`} class="gap-1 rounded-(--radius-row) border border-warning bg-warning-soft p-3">
            <Text variant="meta" tone="fg2">{item().message}</Text>
            <View class="flex-row">
              <AppButton testID={`fp-set-profile-${item().field}`} size="sm" variant="ghost" label={`Set ${item().label.toLowerCase()}`} onPress={() => s.openProfilePrerequisite(item().field)} />
            </View>
          </View>
        )}
      </Each>
      <If when={h().profileDiffers.length}>
        <View class="flex-row flex-wrap items-center gap-2">
          <Text variant="meta" tone="fg2">{profileDiffersNote(h().profileDiffers)}</Text>
          <AppButton testID="fp-use-profile" size="sm" variant="ghost" label="Use profile values" onPress={() => s.useProfileValues()} />
        </View>
      </If>
      <If when={h().amends}>
        <Text variant="meta" tone="fg2">{`This return amends a filed one. ${FILED_KEPT}`}</Text>
      </If>
      <If when={h().filedOriginalView}>{(v) => <Text variant="meta" tone="fg2">{filedOriginalStrip(v())}</Text>}</If>
      <If when={h().profileChanged.length}>
        <View testID="fp-profile-changed" class="gap-0.5">
          <Text variant="meta" tone="fg2">{profileChangedNote(h().statusWord === DRY_RUN_WORD)}</Text>
          <Each of={h().profileChanged}>{(c) => <Text variant="meta" tone="fg3">{`· ${c()}`}</Text>}</Each>
        </View>
      </If>
      <If when={h().sendNotice}>
        <View testID="fp-send-notice" class="gap-1 rounded-(--radius-row) border border-warning bg-warning-soft p-3">
          <Text variant="meta" tone="fg2">{h().sendNotice}</Text>
          <If when={h().accountFix}>
            <Text testID="fp-account-fix" variant="meta" tone="fg2">{ACCOUNT_FIX}</Text>
          </If>
        </View>
      </If>
      <If when={h().submittedLine}>
        <Text testID="fp-send-line" variant="meta" tone="fg2">{h().submittedLine}</Text>
      </If>
      <If when={h().canResolve}>
        <View class="flex-row flex-wrap items-center gap-2">
          <AppButton testID="fp-email-came" size="sm" label="The email came" onPress={() => s.askResolve("received")} />
          <AppButton testID="fp-email-missing" size="sm" label="It did not arrive" onPress={() => s.askResolve("not_received")} />
        </View>
      </If>
      <Each of={h().formErrors}>{(e) => <Text variant="meta" tone="danger">{e()}</Text>}</Each>
    </View>
  );
}

// ---------------------------------------------------------------- parts

/** What a row is taken to be tall before it is drawn (points). */
const ROW_ESTIMATE = 104;

/** An open part's list, for a step to a row it has not drawn: its ref and where the row is in it. */
interface PartList {
  ref: ListRef;
  indexOf: (row: string) => number;
}

/** One row of a part's list: the "From tax profile" fold, or item ii. One object per row for the page's life (a list compares items by identity). */
interface PartRow {
  key: string;
  fold: boolean;
  ii: number;
}

interface PartCardProps {
  s: FormSession;
  pi: number;
  narrow: Accessor<boolean>;
  onRef: (id: string, ref: ViewRef) => void;
  onUnref: (id: string, ref: ViewRef) => void;
  onList: (list: PartList | null) => void;
  onCard: (ref: ViewRef) => void;
  onTop: (y: number, height: number) => void;
  /** Its rows are drawn now (the card is near the viewport). */
  near: Accessor<boolean>;
}

/** An item row of a part's list: it hands the page its view while it is drawn. */
function PartItem(props: { card: PartCardProps; ii: number; first: Accessor<boolean>; last: Accessor<boolean> }) {
  const c = props.card;
  const id = `${c.pi}:${props.ii}`;
  let mine: ViewRef | undefined;
  cleanup(() => mine && c.onUnref(id, mine));
  return (
    <ItemRow
      s={c.s}
      id={id}
      it={c.s.schema!.parts[c.pi].items[props.ii]}
      narrow={c.narrow}
      first={props.first}
      last={props.last}
      onRef={(rid, ref) => {
        mine = ref;
        c.onRef(rid, ref);
      }}
    />
  );
}

/** An open part's rows: a list in the page's viewport (only the rows near it exist). */
function PartRows(props: { card: PartCardProps; rows: Accessor<PartRow[]>; estimate: number }) {
  const c = props.card;
  const pi = c.pi;
  // Read once: rows drawn before give this part's own row height, so the list comes back as long as it went.
  const estimate = untracked(() => props.estimate);
  cleanup(() => c.onList(null));
  return (
    <List
      viewport="parent"
      each={props.rows()}
      keyExtractor={(r: PartRow) => r.key}
      estimatedItemSize={estimate}
      ref={(ref: ListRef) =>
        c.onList({ ref, indexOf: (row) => props.rows().findIndex((r) => !r.fold && `${pi}:${r.ii}` === row) })
      }
    >
      {(row, index) => (
        // Keyed by the row: a recycled cell given another row draws that row anew, with its own state.
        <Each of={[row()]}>
          {(r) =>
            r().fold ? (
              <ProfileFold s={c.s} pi={pi} first={() => index() === 0} />
            ) : (
              <PartItem card={c} ii={r().ii} first={() => index() === 0} last={() => index() === props.rows().length - 1} />
            )
          }
        </Each>
      )}
    </List>
  );
}

function PartCard(props: PartCardProps) {
  const s = props.s;
  const p = s.schema!.parts[props.pi];
  const info = s.part(props.pi);
  const pi = props.pi;
  // The rows' height when they were last drawn: the empty box that stands in for them while the part is far away.
  const rowsHeight = signal(0);
  const made = new Map<string, PartRow>();
  const rowOf = (fold: boolean, ii: number) => {
    const key = `${fold ? "f" : "i"}${ii}`;
    let r = made.get(key);
    if (!r) made.set(key, (r = { key, fold, ii }));
    return r;
  };
  // The part's rows: the fold before its first profile item, then every item not folded into it and not gated away (G1).
  const rows = computed<PartRow[]>(
    () => {
      const fold = info().fold;
      const hidden = fold && !fold.open ? new Set(fold.items) : null;
      const out: PartRow[] = [];
      p.items.forEach((_it, ii) => {
        if (fold?.first === ii) out.push(rowOf(true, ii));
        // A gated-away row is not a row of the list: it is not drawn and not counted.
        if (hidden?.has(ii) || s.row(`${pi}:${ii}`)().hidden) return;
        out.push(rowOf(false, ii));
      });
      return out;
    },
    { equals: (a, b) => a.length === b.length && a.every((r, i) => r === b[i]) },
  );
  return (
    <View
      ref={(r: ViewRef) => props.onCard(r)}
      testID={`fp-part-${pi}`}
      class="rounded-(--radius-card) border border-border bg-card"
      onLayout={(e) => props.onTop(e.bounds.origin.y, e.bounds.size.height)}
    >
      <View class="sticky top-0 z-10 rounded-t-(--radius-card) bg-card" classList={{ "rounded-b-(--radius-card)": !info().open, "border-b border-border": info().open }}>
        <ListRow testID={`fp-part-${pi}-toggle`} accessibilityLabel={partName(p, pi)} onPress={() => s.togglePart(pi)}>
          <Icon name={info().open ? "chevron-down" : "chevron-right"} tone="fg3" />
          <Text variant="control" class="min-w-0 flex-1 font-semibold">{partName(p, pi)}</Text>
          <Text variant="meta" tone="fg3">{`${info().items} item${info().items === 1 ? "" : "s"}`}</Text>
        </ListRow>
      </View>
      <If when={info().open}>
        <If when={props.near()} else={<View style={{ height: rowsHeight() || rows().length * ROW_ESTIMATE }} />}>
          <View onLayout={(e) => rowsHeight.set(e.bounds.size.height)}>
            <PartRows card={props} rows={rows} estimate={rowsHeight() && rows().length ? rowsHeight() / rows().length : ROW_ESTIMATE} />
          </View>
        </If>
      </If>
    </View>
  );
}

// ---------------------------------------------------------------- navigator (wide)

function Badge(props: { text: string; danger?: boolean }) {
  return (
    <View class="h-4.5 min-w-4.5 items-center justify-center rounded-full px-1.25" classList={{ "bg-danger": Boolean(props.danger), "bg-hover": !props.danger }}>
      <Text variant="cap" tone={props.danger ? "primaryFg" : "fg2"} class={props.danger ? "text-card" : undefined}>{props.text}</Text>
    </View>
  );
}

/**
 * A part name for the compact navigator's one line (old form_page.js compactPartName): the filing instructions in
 * parentheses go ("(attach additional sheet/s, if necessary)") and "Schedule" becomes "Sch.".
 */
export function compactPartName(name: string): string {
  const short = name
    .replace(/\s*\((?:attach|if graduated|except)[^)]*\)/gi, "")
    .replace(/^Schedule\b/, "Sch.")
    .trim();
  return short || name;
}

function Navigator(props: { s: FormSession; edge: number }) {
  const s = props.s;
  const parts = s.schema!.parts;
  const infos = parts.map((_p, i) => s.part(i));
  const openOne = () => {
    const open = infos.map((f, i) => (f().open ? i : -1)).filter((i) => i >= 0);
    return open.length === 1 ? open[0] : -1;
  };
  const needing = () => infos.filter((f) => f().errors > 0).length;
  const compact = () => app.prefs().compactFormNav;
  return (
    <View testID="fp-nav" class="shrink-0 pt-7 pb-4" style={{ width: props.edge + 208 + 6, paddingLeft: props.edge }}>
      <View class="flex-row items-center gap-1 px-2 pb-1.5" style={{ width: 208 }}>
        <Text variant="metaStrong" tone="fg3">On this form</Text>
        <View class="flex-1" />
        <Text variant="meta" tone="fg3" testID="fp-nav-count">{openOne() >= 0 ? `${openOne() + 1} of ${parts.length}` : `${parts.length} parts`}</Text>
        <IconButton
          testID="fp-parts-toggle"
          icon={s.allPartsOpen() ? "chevrons-down-up" : "chevrons-up-down"}
          tooltip={s.allPartsOpen() ? "Collapse all parts" : "Expand all parts"}
          onPress={() => s.toggleAllParts()}
        />
      </View>
      <If when={needing() > 0}>
        <PressCell
          testID="fp-nav-attention"
          label="Go to the next part that needs attention"
          class="mb-1.5 rounded-(--radius-row) px-2 py-0.5"
          style={{ width: 208 }}
          onPress={() => {
            const next = s.nextNeedingPart();
            if (next !== null) s.jumpTo(next);
          }}
        >
          <Text variant="meta" tone="danger">{`${needing()} part${needing() > 1 ? "s" : ""} need${needing() > 1 ? "" : "s"} attention · next ↓`}</Text>
        </PressCell>
      </If>
      <Scroll testID="fp-nav-list" class="min-h-0 flex-1" contentClass="gap-0.5 pr-1.5" style={{ width: 208 + 6 }}>
        {parts.map((p, i) => (
          // Old form_page.js navRows: with "Compact form sections" off (the default) the full part name wraps over as
          // many lines as it needs (rows at least 32 tall, 6 above and below); on, one 28-point line of the shortened
          // name, cut with an ellipsis, the full name on hover.
          <PressCell
            testID={`fp-nav-${i}`}
            label={partName(p, i)}
            tooltip={compact() ? partName(p, i) : undefined}
            selected={openOne() === i}
            class={compact() ? "group h-7 flex-row items-center gap-2 rounded-(--radius-row) px-2" : "group flex-row items-start gap-2 rounded-(--radius-row) px-2 py-1.5"}
            style={compact() ? { width: 208 } : { width: 208, minHeight: 32 }}
            onPress={() => s.jumpTo(i)}
          >
            <If
              when={compact()}
              else={
                <Text variant="control" tone="fg2" class="min-w-0 flex-1 leading-[18px] group-selected:text-foreground">
                  {partName(p, i)}
                </Text>
              }
            >
              <Line variant="control" tone="fg2" class="flex-1 group-selected:text-foreground">
                {compactPartName(partName(p, i))}
              </Line>
            </If>
            <View class="flex-row gap-1">
              <If when={infos[i]().errors > 0}>
                <Badge text={String(infos[i]().errors)} danger />
              </If>
              <Badge text={String(infos[i]().items)} />
            </View>
          </PressCell>
        ))}
      </Scroll>
    </View>
  );
}

// ---------------------------------------------------------------- the pill

function SectionsList(props: { s: FormSession; onPick: () => void; maxHeight: number }) {
  const s = props.s;
  const parts = s.schema!.parts;
  const needing = () => parts.filter((_p, i) => s.part(i)().errors > 0).length;
  return (
    <View testID="fp-sections" class="gap-1">
      <View class="flex-row items-start gap-2 pr-1 pb-2 pl-2">
        <View class="min-w-0 flex-1">
          <Text variant="meta" tone="fg2">On this form</Text>
          <Text variant="title">{`${parts.length} section${parts.length > 1 ? "s" : ""}${needing() ? ` · ${needing()} need${needing() > 1 ? "" : "s"} attention` : ""}`}</Text>
        </View>
        <IconButton
          testID="fp-sections-toggle"
          icon={s.allPartsOpen() ? "chevrons-down-up" : "chevrons-up-down"}
          tooltip={s.allPartsOpen() ? "Collapse all parts" : "Expand all parts"}
          onPress={() => s.toggleAllParts()}
        />
      </View>
      <Scroll testID="fp-sections-list" style={{ maxHeight: props.maxHeight }} contentClass="gap-0.5">
        {parts.map((p, i) => {
          const names = sectionNames(p, i);
          const info = s.part(i);
          return (
            <ListRow
              testID={`fp-section-${i}`}
              accessibilityLabel={partName(p, i)}
              onPress={() => {
                props.onPick();
                s.jumpTo(i);
              }}
            >
              <View class="h-7 w-0.75 rounded-full" classList={{ "bg-accent-blue": s.current() === i && info().open }} />
              <View class="size-4.5 items-center justify-center rounded-full" classList={{ "bg-danger": info().errors > 0, "border-2 border-border-strong": info().errors === 0 }}>
                <If when={info().errors > 0}>
                  <Text variant="cap" class="text-card">!</Text>
                </If>
              </View>
              <View class="min-w-0 flex-1">
                <Text variant="emph" lines={1}>{names.short}</Text>
                <Text variant="meta" tone="fg2" lines={1}>{names.tag}</Text>
              </View>
              <Text variant="metaStrong" tone={info().errors > 0 ? "danger" : "fg2"}>
                {info().errors > 0 ? `Fix · ${info().errors}` : info().todo > 0 ? `${info().todo} to do` : `${info().items} item${info().items === 1 ? "" : "s"}`}
              </Text>
            </ListRow>
          );
        })}
      </Scroll>
    </View>
  );
}

function Pill(props: { s: FormSession; wide: Accessor<boolean>; narrow: Accessor<boolean>; box: Accessor<{ left: number; width: number } | null> }) {
  const s = props.s;
  const parts = s.schema!.parts;
  const listOpen = signal(false);
  const cur = () => Math.min(Math.max(s.current(), 0), parts.length - 1);
  const todo = () => s.todo();
  const small = () => {
    const ids = todo().map((r) => r.id);
    const here = sectionNames(parts[cur()], cur());
    if (!ids.length) return `${here.tag} · ${cur() + 1} of ${parts.length}`;
    const at = s.stepAt() ? ids.indexOf(s.stepAt()!) : -1;
    if (at >= 0) {
      const pi = Number(s.stepAt()!.split(":")[0]);
      return `${at + 1} of ${ids.length} to do · ${sectionNames(parts[pi], pi).tag}`;
    }
    return `${ids.length} to do · ${here.tag}`;
  };
  const big = () => {
    const ids = todo().map((r) => r.id);
    const at = s.stepAt() && ids.includes(s.stepAt()!) ? s.stepAt()! : null;
    if (at) {
      const it = s.item(at)!;
      return it.no ? `Item ${it.no} · ${it.l}` : it.l;
    }
    return sectionNames(parts[cur()], cur()).short;
  };
  // Red (old drawPill): the to-do rows with a message; with nothing to do, the parts that have one. Shown or not.
  const red = () => s.attention();
  const redLabel = () => {
    const n = red();
    return `${n} item${n > 1 ? "s" : ""} need${n > 1 ? "" : "s"} attention`;
  };
  // The row is one button to assistive tech and to the agent tree, so its label carries what the pill shows.
  const openLabel = () => `All sections · ${small()} · ${big()}${red() > 0 ? ` · ${redLabel()}` : ""}`;
  const target = (dir: number) => {
    todo();
    s.stepAt();
    s.current();
    return s.stepTarget(dir);
  };
  const stepLabel = (dir: number) => {
    const t = target(dir);
    if (t && "check" in t) return "Check";
    return todo().length ? (dir < 0 ? "Previous to do" : "Next to do") : dir < 0 ? "Previous section" : "Next section";
  };
  const main = (
    <View testID="fp-pill-main" class="h-full min-w-0 flex-1 flex-row items-center gap-2.5 rounded-[14px] pr-2.5 pl-1.5" accessibilityLabel={`All sections${listOpen() ? ", shown" : ""}`}>
      <View class="min-w-0 flex-1">
        <Text variant="cap" tone="fg2" lines={1} testID="fp-pill-small">{small()}</Text>
        <Text variant="emph" lines={1} testID="fp-pill-big">{big()}</Text>
      </View>
      <If when={red() > 0}>
        <View testID="fp-pill-count" accessibilityLabel={redLabel()} class="h-5.5 min-w-5.5 items-center justify-center rounded-full bg-danger px-1.5">
          <Text variant="num" class="text-card">{String(red())}</Text>
        </View>
      </If>
    </View>
  );
  return (
    // Centred over the form's own column when wide (old formColumnBox), over the whole pane otherwise.
    <View
      style={props.box() ? { pointerEvents: "box-none", left: props.box()!.left, width: props.box()!.width } : { pointerEvents: "box-none" }}
      class="absolute bottom-4 items-center px-4"
      classList={{ "inset-x-0": !props.box() }}
    >
      <View testID="fp-pill" class="w-full flex-row items-center gap-1.5 rounded-[22px] border border-border-strong bg-raised p-1.5 shadow-lg" style={{ maxWidth: PILL_W, height: props.narrow() ? NARROW_PILL_HEIGHT : 56 }}>
        <IconButton testID="fp-pill-prev" icon="chevron-left" tooltip={stepLabel(-1)} disabled={!target(-1)} onPress={() => s.step(-1)} />
        <If
          when={props.wide()}
          else={
            <View class="h-full min-w-0 flex-1 flex-row items-center">
              <IconButton testID="fp-pill-list" icon="list" tooltip="All sections" pressed={listOpen()} onPress={() => listOpen.set(!listOpen())} />
              <View class="min-w-0 flex-1">
                <ListRow testID="fp-pill-open" accessibilityLabel={openLabel()} onPress={() => listOpen.set(true)}>
                  {main}
                </ListRow>
              </View>
              <Sheet testID="fp-sections-sheet" open={listOpen()} onOpenChange={(o) => listOpen.set(o)}>
                <View class="px-2 pb-4">
                  <SectionsList s={s} onPick={() => listOpen.set(false)} maxHeight={Math.round(windowSize().height * 0.6)} />
                </View>
              </Sheet>
            </View>
          }
        >
          <View class="h-full min-w-0 flex-1 flex-row items-center">
            <Popover
              testID="fp-sections-popover"
              placement="top"
              open={listOpen()}
              onOpenChange={(o) => listOpen.set(o)}
              width={PILL_W - 12}
              trigger={<IconButton testID="fp-pill-list" icon="list" tooltip="All sections" pressed={listOpen()} />}
            >
              <SectionsList s={s} onPick={() => listOpen.set(false)} maxHeight={Math.min(440, windowSize().height - 200)} />
            </Popover>
            <View class="min-w-0 flex-1">
              <ListRow testID="fp-pill-open" accessibilityLabel={openLabel()} onPress={() => listOpen.set(!listOpen())}>
                {main}
              </ListRow>
            </View>
          </View>
        </If>
        <IconButton
          testID="fp-pill-next"
          icon={(() => {
            const t = target(1);
            return t && "check" in t ? "circle-check" : "chevron-right";
          })()}
          tooltip={stepLabel(1)}
          disabled={!target(1)}
          onPress={() => s.step(1)}
        />
      </View>
    </View>
  );
}

// ---------------------------------------------------------------- dialogs

function Dialogs(props: { s: FormSession }) {
  const s = props.s;
  const o = () => s.overlay();
  const kind = (k: NonNullable<Overlay>["kind"]) => o()?.kind === k;
  const removal = () => {
    const v = o();
    return v?.kind === "removal" ? v : null;
  };
  const existing = () => {
    const v = o();
    return v?.kind === "existing" ? existingOfferFor(v.existing) : null;
  };
  const warns = () => {
    const v = o();
    return v?.kind === "queue-confirm" ? v.warns : [];
  };
  const blocked = () => {
    const v = o();
    return v?.kind === "tin-blocked" ? v.row : null;
  };
  const resolving = () => {
    const v = o();
    return v?.kind === "resolve-confirm" ? v : null;
  };
  const period = () => {
    const v = o();
    return v?.kind === "sample-period" ? v : null;
  };
  const profileAsk = () => {
    const v = o();
    return v?.kind === "profile-ask" ? v.ask : null;
  };
  // In the phone layout (every touch screen, and a desktop window under 600 points) the ask is a bottom sheet; else a dialog (Q1 P1b).
  const phone = () => phoneLayout();
  const askTitle = () => {
    const a = profileAsk();
    return a ? `Save ${a.label.toLowerCase()} to the tax profile?` : "";
  };
  const askBody = () => {
    const a = profileAsk();
    if (!a) return "";
    const who = a.personal ? a.name : `${a.name} for ${a.year}`;
    return `“${a.value}” is saved for ${who}. Later returns use it. You can change it later under Profile.`;
  };
  const fast = signal(false);
  // The period dialog's boxes and quarter, filled when it opens.
  const parts = signal<Record<string, string>>({});
  effect(() => {
    const p = period();
    if (p) untracked(() => parts.set(Object.fromEntries(p.parts.map((x) => [x.key, x.value]))));
  });
  const savePeriod = () => {
    const p = period();
    if (!p) return;
    const v = parts();
    s.takeSamplePeriod(Object.fromEntries(p.parts.map((x) => [x.key, x.part === "quarter" ? String(v[x.key] === "true") : (v[x.key] ?? "").trim()])));
  };
  return (
    <>
      <Dialog testID="removal-confirm" width={420} open={kind("removal")} onOpenChange={(open) => !open && s.cancelRemoval()}>
        <View class="gap-3 p-5">
          <Text variant="title">{removal()?.message ?? ""}</Text>
          <AppButton testID="removal-continue" variant="primary" block label="Continue" onPress={() => s.confirmRemoval()} />
          <AppButton testID="removal-cancel" block label="Cancel" onPress={() => s.cancelRemoval()} />
        </View>
      </Dialog>
      <Dialog testID="existing-offer" width={420} open={kind("existing")} onOpenChange={(open) => !open && s.takeExisting("keep")}>
        <If when={existing()}>
          {(offer) => (
            <View class="gap-3 p-5">
              <View class="gap-0.5">
                <Text variant="title">{offer().title}</Text>
                <If when={offer().subtitle}>
                  <Text variant="meta" tone="fg2">{offer().subtitle}</Text>
                </If>
              </View>
              <Each of={offer().choices}>
                {(c) => <AppButton testID={`existing-${c().id}`} block variant={c().variant} label={c().label} hint={c().hint} onPress={() => s.takeExisting(c().id)} />}
              </Each>
              <If when={offer().fastLane}>
                <CheckboxLabel
                  testID="existing-fast-lane"
                  label="Don't ask again for drafts (this session)"
                  checked={fast()}
                  onChange={(on) => {
                    fast.set(on);
                    s.setFastLane(on);
                  }}
                />
              </If>
            </View>
          )}
        </If>
      </Dialog>
      <Dialog testID="tin-queue-blocked" width={420} open={kind("tin-blocked")} onOpenChange={(open) => !open && s.dismissTinBlocked()}>
        <If when={blocked()}>
          {(row) => (
            <View class="gap-3 p-5">
              <Text variant="title">{row().state === "pending" ? "This TIN is waiting to be verified" : row().state === "fix" ? "This TIN needs a step on your account" : "This TIN is not activated"}</Text>
              <If when={row().message}>
                <Text testID="tin-blocked-message" variant="control">{row().message}</Text>
              </If>
              <If when={row().test_only}>
                <Text variant="meta" tone="fg2">A test TIN sends nothing to the BIR.</Text>
              </If>
              <If when={row().state === "fix" && row().action_url}>
                <AppButton
                  testID="tin-blocked-action"
                  variant="primary"
                  block
                  label="Fix on your account"
                  onPress={() => {
                    void openUrl(row().action_url!);
                    void returnedFromAccountSite();
                  }}
                />
              </If>
              <AppButton testID="tin-blocked-close" block label="Close" onPress={() => s.dismissTinBlocked()} />
            </View>
          )}
        </If>
      </Dialog>
      <Dialog testID="queue-confirm" width={420} open={kind("queue-confirm")} onOpenChange={(open) => !open && s.cancelQueueConfirm()}>
        <View class="gap-3 p-5">
          <Text variant="title">Queue this return?</Text>
          <Each of={warns()}>{(w, n) => <Text variant="control" testID={`queue-warn-${n()}`}>{w()}</Text>}</Each>
          <AppButton testID="queue-continue" variant="primary" block label="Continue" onPress={() => s.confirmQueue()} />
          <AppButton testID="queue-cancel" block label="Cancel" onPress={() => s.cancelQueueConfirm()} />
        </View>
      </Dialog>
      <Dialog testID="resolve-confirm" width={420} open={kind("resolve-confirm")} onOpenChange={(open) => !open && s.cancelResolve()}>
        <If when={resolving()}>
          {(r) => (
            <View class="gap-3 p-5">
              <Text variant="title">{r().question}</Text>
              <AppButton testID="resolve-continue" variant="primary" block label={r().button} onPress={() => s.confirmResolve()} />
              <AppButton testID="resolve-cancel" block label="Cancel" onPress={() => s.cancelResolve()} />
            </View>
          )}
        </If>
      </Dialog>
      <Dialog testID="sample-period" width={420} open={kind("sample-period")} onOpenChange={(open) => !open && s.closeSamplePeriod()}>
        <If when={period()}>
          {(p) => (
            <View class="gap-3 p-5">
              <View class="gap-0.5">
                <Text variant="title">Which period is this return for?</Text>
                <Text variant="meta" tone="fg2">Save makes this sample your return for that period.</Text>
              </View>
              <Each of={p().parts.filter((x) => x.part !== "quarter")}>
                {(x) => (
                  <TextField
                    testID={`sample-period-${x().part}`}
                    label={PERIOD_PART_LABELS[x().part] || x().part}
                    value={parts()[x().key] ?? ""}
                    onChange={(v) => parts.set({ ...parts(), [x().key]: v })}
                  />
                )}
              </Each>
              <If when={p().parts.some((x) => x.part === "quarter")}>
                <View class="gap-1">
                  <Text variant="meta" tone="fg2">{PERIOD_PART_LABELS.quarter}</Text>
                  <View class="flex-row flex-wrap gap-2">
                    <Each of={p().parts.filter((x) => x.part === "quarter")}>
                      {(x) => (
                        <AppButton
                          testID={`sample-period-q${x().quarter}`}
                          size="sm"
                          variant={parts()[x().key] === "true" ? "primary" : "secondary"}
                          label={`Q${x().quarter}`}
                          onPress={() => {
                            const next: Record<string, string> = { ...parts() };
                            for (const q of p().parts) if (q.part === "quarter") next[q.key] = q.key === x().key ? "true" : "false";
                            parts.set(next);
                          }}
                        />
                      )}
                    </Each>
                  </View>
                </View>
              </If>
              <If when={p().error}>
                <Text variant="meta" tone="danger">{p().error}</Text>
              </If>
              <View class="flex-row justify-end gap-2">
                <AppButton testID="sample-period-cancel" variant="ghost" label="Cancel" onPress={() => s.closeSamplePeriod()} />
                <AppButton testID="sample-period-save" variant="primary" label="Save" onPress={savePeriod} />
              </View>
            </View>
          )}
        </If>
      </Dialog>
      {/* Q1 P1b: "Save to profile?" — a dialog from 600 points up, a bottom sheet under it. Escape, the
          scrim and a swipe-down are no answer: nothing saved, nothing remembered (asks again on a changed leave). */}
      <Dialog testID="profile-ask" width={420} open={kind("profile-ask") && !phone()} onOpenChange={(open) => !open && s.dismissProfileAsk()}>
        <If when={profileAsk()}>
          {() => (
            <View class="gap-3 p-5">
              <View class="gap-0.5">
                <Text variant="title">{askTitle()}</Text>
                <Text variant="meta" tone="fg2">{askBody()}</Text>
              </View>
              <View class="flex-row justify-end gap-2">
                <AppButton testID="profile-ask-only" label="Only this return" onPress={() => s.profileAskOnly()} />
                <AppButton testID="profile-ask-save" variant="primary" label="Save to profile" onPress={() => s.profileAskSave()} />
              </View>
            </View>
          )}
        </If>
      </Dialog>
      <Sheet testID="profile-ask-sheet" open={kind("profile-ask") && phone()} onOpenChange={(open) => !open && s.dismissProfileAsk()}>
        <If when={profileAsk()}>
          {() => (
            <View class="gap-3">
              <View class="gap-0.5">
                <Text variant="title">{askTitle()}</Text>
                <Text variant="meta" tone="fg2">{askBody()}</Text>
              </View>
              <View class="gap-2">
                <AppButton testID="profile-ask-save" variant="primary" block label="Save to profile" onPress={() => s.profileAskSave()} />
                <AppButton testID="profile-ask-only" block label="Only this return" onPress={() => s.profileAskOnly()} />
              </View>
            </View>
          )}
        </If>
      </Sheet>
    </>
  );
}
