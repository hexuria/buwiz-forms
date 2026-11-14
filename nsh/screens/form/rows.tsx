// The form page's rows: one item (number chip, label and marks, its controls from ./controls, the reserved 18-point
// message line, fact cards, Item 13's card), and a part's "From tax profile" fold. Each row reads its own RowInfo
// signal and its boxes' signals only, so an edit re-runs the rows it changed and nothing else.
import type { Accessor, JSX, ViewRef } from "nsh";
import { signal, untracked, View } from "nsh";
import { AppButton, Code, Icon, type IconName, Switch, Text, TextField, Tooltip } from "../../ui";
import type { FieldControlProps, RadioGroupProps, ScheduleProps, SchemaField, SchemaItem } from "./contract";
import { FieldControl, RadioGroupControl, ScheduleControl } from "./controls";
import { choiceLabel, controlWidth, isTinItem, needsText, oneDate } from "./schema";
import type { AskCard, ElectionCard, FormSession } from "./session";

export interface RowProps {
  s: FormSession;
  id: string;
  it: SchemaItem;
  narrow: Accessor<boolean>;
  /** The row's view, for a step to scroll to. */
  onRef: (id: string, ref: ViewRef) => void;
}

/** The props of one box's control. */
export function fieldProps(s: FormSession, f: SchemaField, narrow: Accessor<boolean>, label = f.l, width = controlWidth(f, 320)): FieldControlProps {
  return {
    field: f,
    label,
    testID: `fp-${f.k}`,
    box: s.box(f.k),
    width,
    narrow,
    onEdit: (text) => s.edit(f.k, text),
    onFocus: () => s.focus(f.k),
    onBlur: () => s.blur(f.k),
    focusRef: (focus) => s.registerFocus(f.k, focus),
  };
}

function radioProps(s: FormSession, id: string, it: SchemaItem, radios: SchemaField[], narrow: Accessor<boolean>): RadioGroupProps {
  const keys = radios.map((f) => f.k);
  const boxes = keys.map((k) => s.box(k));
  return {
    label: it.l,
    testID: `fp-radio-${id}`,
    choices: radios.map((f) => ({ key: f.k, label: choiceLabel(f), testID: `fp-${f.k}` })),
    selected: s.radioSelected(keys),
    locked: () => boxes.every((b) => b.locked()),
    choiceLocked: (k) => s.box(k).locked(),
    hint: () => (boxes.every((b) => b.locked()) && !boxes.some((b) => b.value() === "true") ? boxes.map((b) => b.hint()).find(Boolean) || "" : ""),
    off: () => boxes.map((b) => b.off()).find(Boolean) || "",
    narrow,
    onPick: (k) => s.pickRadio(k),
    focusRef: (focus) => s.registerFocus(`radio:${id}`, focus),
  };
}

function scheduleProps(s: FormSession, id: string, it: SchemaItem, narrow: Accessor<boolean>): ScheduleProps {
  const keys = it.keys || [];
  const cols = it.cols || [];
  const columns = cols.map((col, index) => ({ index, col })).filter((c) => c.col.t !== "hidden");
  const numbered = !it.row_labels || it.numbered !== false;
  const rowName = (r: number) => (numbered ? String((it.start ?? 1) + r) : it.row_labels?.rows[r] || String((it.start ?? 1) + r));
  const openRow = signal<number | null>(null);
  const filled = (r: number) => columns.some((c) => {
    const v = s.box(keys[r][c.index]).value();
    return v !== "" && v !== "0.00" && v !== "0" && v !== "false";
  });
  return {
    item: it,
    testID: `fp-table-${keys[0]?.[0] ?? id}`,
    narrow,
    columns,
    rowName,
    summary: (r) => () =>
      columns
        .map((c) => {
          const box = s.box(keys[r][c.index]);
          const v = box.value();
          if (!v || v === "0.00" || v === "false") return "";
          if (c.col.t === "sel") return box.options().find((o) => o.value === v)?.label.split(" — ")[0] ?? v;
          return v === "true" ? c.col.l : v;
        })
        .filter(Boolean)
        .join(" · "),
    shownRows: () => {
      const out: number[] = [];
      let empty = false;
      keys.forEach((_row, r) => {
        if (filled(r)) out.push(r);
        else if (!empty) {
          empty = true;
          out.push(r);
        }
      });
      return out;
    },
    cell: (r, c) => {
      const col = cols[c];
      const k = keys[r][c];
      const f: SchemaField = { k, l: col.l, t: col.t === "hidden" ? "text" : col.t, c: col.c, w: col.w, o: col.o };
      return fieldProps(s, f, narrow, `${col.l}, row ${rowName(r)}`, controlWidth(f, 320));
    },
    openRow: () => openRow(),
    onOpenRow: (r) => {
      if (r === null) s.touch(id);
      openRow.set(r);
    },
    onClearRow: (r) => {
      const edits = Object.fromEntries(keys[r].filter((k, c) => cols[c]?.t !== "hidden" && !untracked(() => s.box(k).locked())).map((k) => [k, ""]));
      s.choose(edits);
    },
    locked: () => !s.header().editable,
  };
}

/** What a row draws for its boxes: a schedule, a note, a fold of extra fields, one date, a TIN, or its boxes. */
function Controls(props: { s: FormSession; id: string; it: SchemaItem; narrow: Accessor<boolean> }) {
  const s = props.s;
  const it = props.it;
  const fs = it.f || [];
  if (it.rows) return <ScheduleControl {...scheduleProps(s, props.id, it, props.narrow)} />;
  if (!fs.length && it.note) return <Text variant="meta" tone="fg3" testID={`fp-note-${props.id}`}>{it.note}</Text>;
  const date = oneDate(it);
  if (date) return <FieldControl {...fieldProps(s, { k: date.oneKey, l: it.l, t: "date" }, props.narrow, it.l, controlWidth({ t: "date" }, 320))} />;
  if (isTinItem(it)) {
    return (
      <View class="flex-row flex-wrap items-center gap-1.5">
        {fs.map((f, j) => (
          <View class="flex-row items-center gap-1.5">
            {j ? <Text tone="fg3">-</Text> : null}
            <FieldControl {...fieldProps(s, f, () => false, f.l, j === 3 ? 76 : 58)} />
          </View>
        ))}
      </View>
    );
  }
  const radios = fs.filter((f) => f.t === "radio");
  const boxes = fs.filter((f) => f.t !== "radio");
  const multi = fs.length > 1;
  const body = (
    <View class="flex-row flex-wrap items-end gap-x-4 gap-y-3">
      {radios.length ? <RadioGroupControl {...radioProps(s, props.id, it, radios, props.narrow)} /> : null}
      {boxes.map((f) => {
        const sub = multi && f.l && f.l !== it.l && !/^\d+[A-Z]?$/.test(f.l);
        return (
          <View class="min-w-0 gap-1" style={{ width: props.narrow() ? "100%" : undefined }}>
            {sub && f.t !== "chk" ? <Text variant="meta" tone="fg2">{f.l}</Text> : null}
            <FieldControl {...fieldProps(s, f, props.narrow, f.t === "chk" ? f.l || it.l : f.l)} />
          </View>
        );
      })}
    </View>
  );
  if (it.l !== "Additional details") return body;
  const open = signal(s.moreOpen(props.id));
  return (
    <View class="gap-2.5">
      <AppButton
        variant="ghost"
        size="sm"
        icon={open() ? "chevron-down" : "chevron-right"}
        label={`${fs.length} more field${fs.length > 1 ? "s" : ""} not yet placed on the paper layout`}
        testID={`fp-more-${props.id}`}
        onPress={() => {
          s.toggleMore(props.id);
          open.set(!open());
        }}
      />
      <If when={open()}>{body}</If>
    </View>
  );
}

function Mark(props: { icon: IconName; tip: string; id: string }) {
  return (
    <View testID={`fp-mark-${props.icon}-${props.id}`} class="shrink-0 pt-0.5" accessibilityLabel={props.tip} tooltip={<Tooltip label={props.tip} />}>
      <Icon name={props.icon} size={14} tone="fg3" />
    </View>
  );
}

/** A tax fact the profile has not set, asked where the form uses it. */
function FactCard(props: { s: FormSession; ask: Accessor<AskCard> }) {
  const text = signal("");
  const fact = () => props.ask().fact;
  return (
    <View testID={`fp-ask-${fact()}`} class="mt-2 gap-1 rounded-(--radius-row) border border-border-strong bg-card px-3 py-2.5 shadow-xs">
      <Text variant="control" class="font-semibold">{props.ask().label}</Text>
      <Text variant="meta" tone="fg2">{props.ask().note}</Text>
      <View class="mt-2 flex-row flex-wrap gap-2">
        <If
          when={props.ask().control === "choice"}
          else={
            <TextField
              testID={`fp-ask-${fact()}-input`}
              accessibilityLabel={props.ask().label}
              value={text()}
              width={props.ask().control === "date" ? 140 : 320}
              placeholder={props.ask().control === "date" ? "MM/DD/YYYY" : undefined}
              onChange={(v) => {
                text.set(v);
                props.s.typeAsk(fact(), v);
              }}
            />
          }
        >
          <Each of={props.ask().choices}>
            {(choice, n) => (
              <AppButton
                testID={`fp-ask-${fact()}-${n()}`}
                label={choice().label}
                selected={props.ask().pick === JSON.stringify(choice().value)}
                accessibilityLabel={props.ask().pick === JSON.stringify(choice().value) ? `${choice().label}, chosen` : choice().label}
                onPress={() => props.s.pickAsk(fact(), choice().value)}
              />
            )}
          </Each>
        </If>
      </View>
      <View class="mt-3 flex-row justify-end">
        <AppButton
          testID={`fp-ask-${fact()}-save`}
          variant="primary"
          size="sm"
          label="Save"
          disabled={props.ask().control === "choice" && props.ask().pick === null}
          onPress={() => props.s.saveAsk(fact())}
        />
      </View>
    </View>
  );
}

/** 2551Q Item 13's card: the year's income-tax election, or why the profile cannot take one. */
function ElectionCardView(props: { s: FormSession; card: Accessor<ElectionCard> }) {
  const ask = () => {
    const c = props.card();
    return c.kind === "ask" ? c : null;
  };
  const blocked = () => {
    const c = props.card();
    return c.kind === "blocked" ? c : null;
  };
  const choice = (value: "graduated" | "eight_percent", label: string, hint: string) => (
    <AppButton
      testID={`fp-election-${value}`}
      label={label}
      hint={hint}
      selected={ask()?.pick === value}
      accessibilityLabel={ask()?.pick === value ? `${label}, chosen` : label}
      onPress={() => props.s.pickElection(value)}
    />
  );
  return (
    <View testID="fp-election" class="mt-2 gap-1 rounded-(--radius-row) border border-border-strong bg-card px-3 py-2.5 shadow-xs">
      <If when={ask()}>
        {(a) => (
          <View class="gap-1">
            <Text variant="control" class="font-semibold">{a().question}</Text>
            <Text variant="meta" tone="fg2">We save it on the tax profile, so you answer once a year.</Text>
            <View class="mt-3 flex-row flex-wrap gap-2">
              {choice("graduated", "Graduated rates", "Tax on net taxable income")}
              {choice("eight_percent", "8% flat rate", "8% of gross sales or receipts")}
            </View>
            <View class="mt-3 flex-row justify-end">
              <AppButton testID="fp-election-save" variant="primary" size="sm" label="Save to tax profile" disabled={!a().pick} onPress={() => props.s.recordElection()} />
            </View>
          </View>
        )}
      </If>
      <If when={blocked()}>
        {(b) => (
          <View class="gap-1">
            <View class="flex-row items-start gap-2">
              <Icon name="info" tone="fg2" />
              <Text variant="control" class="min-w-0 flex-1 font-semibold">{b().title}</Text>
            </View>
            <Text variant="meta" tone="fg2">{b().body}</Text>
            <If when={b().action}>
              {(action) => (
                <View class="mt-3 flex-row justify-end">
                  <AppButton testID={action().id} variant="primary" size="sm" label={action().label} onPress={() => props.s.electionAction(action().id)} />
                </View>
              )}
            </If>
          </View>
        )}
      </If>
    </View>
  );
}

function Saved(props: { text: string; testID: string }) {
  return (
    <View testID={props.testID} class="mt-1 flex-row items-center gap-1.5">
      <Icon name="circle-check" size={14} tone="success" />
      <Text variant="meta" tone="success" class="min-w-0 flex-1">{props.text}</Text>
    </View>
  );
}

/** One item of a part. */
export function ItemRow(props: RowProps) {
  const s = props.s;
  const id = props.id;
  const it = props.it;
  const info = s.row(id);
  const fs = it.f || [];
  const allCalc = !it.rows && fs.length > 0 && fs.every((x) => x.c);
  const req = fs.some((x) => x.r);
  const label = req ? `${it.l} *` : it.l;
  return (
    <View
      ref={(r: ViewRef) => props.onRef(id, r)}
      testID={`fp-item-${id}`}
      class="flex-row gap-3 border-t border-border pt-3.5 pr-5 pb-2 pl-3 first:border-t-0 last:rounded-b-(--radius-card)"
      classList={{ "bg-background": allCalc && !info().stepHere, "bg-accent-blue-soft": info().stepHere, "pl-2 gap-2 pr-3": props.narrow() }}
    >
      <View class="shrink-0" style={{ width: props.narrow() ? 32 : 44 }}>
        {it.no ? (
          <View class="h-5.5 min-w-7.5 items-center justify-center self-start rounded-(--radius-ctl) bg-hover px-1.5">
            <Code tone="fg2">{it.no}</Code>
          </View>
        ) : null}
      </View>
      <View class="min-w-0 flex-1">
        <View class="mb-2.5 min-h-5.5 flex-row flex-wrap items-start gap-2">
          <Text variant="control" class="min-w-0 flex-1" testID={`fp-label-${id}`}>{label}</Text>
          {it.rows ? <Text variant="meta" tone="fg3" class="pt-0.5">{`${it.rows} rows`}</Text> : null}
          <If when={info().fromProfile}>
            <Mark icon="user" id={id} tip={info().fromProfile} />
          </If>
          {allCalc ? <Mark icon="lock" id={id} tip="Computed from other items" /> : null}
          <If when={info().kept === "template"}>
            <Mark icon="layout-template" id={id} tip="From your template" />
          </If>
          <If when={info().kept === "last"}>
            <Mark icon="history" id={id} tip="Same as last time" />
          </If>
          <If when={info().penalty}>
            <AppButton
              size="sm"
              variant="ghost"
              icon={info().penalty === "computed" ? "history" : "pencil"}
              label={info().penalty === "computed" ? "Use the computed amounts" : "Edit penalties"}
              testID={`fp-penalty-${id}`}
              onPress={() => (info().penalty === "computed" ? s.usePenaltyComputed(id) : s.openPenalties(id))}
            />
          </If>
          <If when={info().canEdit && !info().editing}>
            <AppButton size="sm" variant="ghost" icon="pencil" label="Edit" tooltip="Change it on this return only" accessibilityLabel={`Edit ${it.l} on this return`} testID={`fp-edit-${id}`} onPress={() => s.startProfileEdit(id)} />
          </If>
          <If when={info().stoppable && info().stepHere}>
            <Switch testID={`fp-stop-${id}`} label="Stop here" checked={info().isStop} onChange={(on) => s.setStop(id, on)}>
              <Text variant="meta" tone="fg2">Stop here</Text>
            </Switch>
          </If>
        </View>
        <Each of={info().needs}>
          {(why) => (
            <View class="mb-1.5" tooltip={<Tooltip label={why()} />}>
              <Text variant="meta" tone="fg3">{needsText(why())}</Text>
            </View>
          )}
        </Each>
        <Controls s={s} id={id} it={it} narrow={props.narrow} />
        <If when={info().toProfile}>
          {(tp) => (
            <View testID={`fp-to-profile-${id}`} class="mt-2 gap-0.5">
              <Switch testID={`fp-to-profile-switch-${id}`} label="Save to profile too" checked={tp().on} onChange={(on) => s.setToProfile(id, on)}>
                <Text variant="meta" tone="fg2">Save to profile too</Text>
              </Switch>
              <Text variant="meta" tone="fg3">{tp().note}</Text>
            </View>
          )}
        </If>
        <If when={info().from}>
          <View testID={`fp-from-${id}`} class="mt-1 flex-row items-center gap-2">
            <Text variant="meta" tone="fg3" class="min-w-0 shrink">{info().from}</Text>
            <AppButton size="sm" variant="ghost" label="Use it" testID={`fp-from-use-${id}`} onPress={() => s.useFrom(id)} />
          </View>
        </If>
        <Each of={info().notes}>{(note) => <Text variant="meta" tone="fg3" class="mt-1">{note()}</Text>}</Each>
        <View class="mt-1 min-h-4.5">
          <Text variant="meta" tone="danger" testID={`fp-msg-${id}`}>{info().message}</Text>
        </View>
        <Each of={info().asks} key={(a: AskCard) => a.fact}>
          {(ask) => <FactCard s={s} ask={ask} />}
        </Each>
        <If when={info().askSaved}>
          <Saved testID={`fp-ask-saved-${id}`} text={info().askSaved} />
        </If>
        <If when={info().election}>{(card) => <ElectionCardView s={s} card={card} />}</If>
        <If when={info().electionSaved}>
          <Saved testID="fp-election-saved" text={info().electionSaved} />
        </If>
        <Each of={info().checkable} key={(c: { token: string; label: string }) => c.token}>
          {(c) => (
            <View class="mt-1 flex-row">
              <AppButton size="sm" variant="ghost" label={c().label} testID={`fp-checked-${c().token}`} onPress={() => s.markChecked(c().token)} />
            </View>
          )}
        </Each>
      </View>
    </View>
  );
}

/** A part's "From tax profile · N filled boxes" fold: profile-filled items fold so what is left to type shows. */
export function ProfileFold(props: { s: FormSession; pi: number }): JSX.Element {
  const part = props.s.part(props.pi);
  return (
    <If when={part().fold}>
      {(fold) => (
        <View class="border-t border-border bg-background px-1 py-1 first:border-t-0">
          <AppButton
            variant="ghost"
            size="sm"
            testID={`fp-profile-${props.pi}`}
            icon={fold().open ? "chevron-down" : "chevron-right"}
            label={fold().label + (fold().flagged ? `  ·  ${fold().flagged} need${fold().flagged === 1 ? "s" : ""} attention` : "")}
            accessibilityLabel={`${fold().label}, ${fold().open ? "shown" : "folded"}`}
            onPress={() => props.s.toggleProfileFold(props.pi)}
            trailing={<Icon name="user" size={14} tone="fg3" />}
          />
        </View>
      )}
    </If>
  );
}
