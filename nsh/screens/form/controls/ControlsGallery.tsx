// The field controls in every kind and state, wide and narrow: a preview for people and screenshots (shots/controls.test.tsx), and
// the page the controls' tests drive. Not a screen of the app and not wired into App.tsx (entry: nsh/ControlsShot.tsx). The controls
// get mock boxes (./mock.ts); under each box a readout says what it reported, so a test reads what the page would have been sent.
// Demo taxpayer (Juana Santos, TIN 000-123-453-00000) and generated data only.
import type { JSX } from "nsh";
import { Scroll, signal, View } from "nsh";
import { AppButton, Text } from "../../../ui";
import type { ChoiceOption, SchemaField, SchemaItem } from "../contract";
import { FieldControl, FieldRow, RadioGroupControl, ScheduleControl } from "./index";
import { mockField, mockRadios, mockSchedule, narrowNow, reports, type Reports } from "./mock";

const ATC: ChoiceOption[] = [
  { value: "WI010", label: "WI010 — Professional fees paid to individuals, gross income of P3M or less" },
  { value: "WI011", label: "WI011 — Professional fees paid to individuals, gross income over P3M" },
  { value: "WC010", label: "WC010 — Professional fees paid to juridical persons, gross income of P720,000 or less" },
  { value: "WC011", label: "WC011 — Professional fees paid to juridical persons, gross income over P720,000" },
  { value: "WI100", label: "WI100 — Rentals of real or personal properties" },
  ...Array.from({ length: 70 }, (_, i) => ({ value: `WX${String(100 + i)}`, label: `WX${100 + i} — Sample withholding category ${i + 1}` })),
];
const MONTHS = ["01 - January", "02 - February", "03 - March", "04 - April"];

const f = (k: string, l: string, t: SchemaField["t"], more: Partial<SchemaField> = {}): SchemaField => ({ k, l, t, ...more });

function Section(props: { title: string; testID?: string; children?: JSX.Element }) {
  return (
    <View testID={props.testID} class="gap-4 rounded-(--radius-card) border border-border bg-card p-4">
      <Text variant="title">{props.title}</Text>
      {props.children}
    </View>
  );
}

/** A box, its label and what it reported. */
function Row(props: { log: Reports; field: SchemaField; hint?: string; title?: string; children?: JSX.Element }) {
  return (
    <FieldRow label={props.title ?? props.field.l} hint={props.hint} narrow={narrowNow}>
      {props.children}
      <Text variant="cap" tone="fg3" class="mt-1">
        {`${props.field.k} edit: ${props.log.edits()[props.field.k] ?? ""} | blur: ${props.log.blurs()[props.field.k] ?? 0} | focus: ${props.log.focuses()[props.field.k] ?? 0}`}
      </Text>
    </FieldRow>
  );
}

/** What a radio group reported so far: "regime2=true regime1=true". */
function picked(log: Reports, name: string): string {
  return Object.entries(log.edits())
    .filter(([k]) => k.startsWith(name))
    .map(([k, v]) => `${k}=${v}`)
    .join(" ");
}

function TextSection(props: { log: Reports }) {
  const name = mockField(props.log, f("name", "Registered name", "text", { m: 40 }), { value: "Juana Santos" }, { width: 320 });
  const email = mockField(props.log, f("email", "Email", "email"), { value: "juana@example.test" }, { width: 320 });
  const phone = mockField(props.log, f("phone", "Phone", "phone"), { value: "" }, { width: 200 });
  const zip = mockField(props.log, f("zip", "ZIP code", "int", { m: 4 }), { value: "1200" }, { width: 88 });
  return (
    <Section title="Text, email, phone, whole number" testID="controls-text">
      <Row log={props.log} field={name.field}><FieldControl {...name} /></Row>
      <Row log={props.log} field={email.field}><FieldControl {...email} /></Row>
      <Row log={props.log} field={phone.field}><FieldControl {...phone} /></Row>
      <Row log={props.log} field={zip.field}><FieldControl {...zip} /></Row>
    </Section>
  );
}

function AmountSection(props: { log: Reports }) {
  const money = mockField(props.log, f("money", "Gross receipts", "money", { r: 1 }), { value: "1250000.50" });
  const whole = mockField(props.log, f("whole", "Whole pesos", "money", { w: 1 }), { value: "48000" });
  const sum = mockField(props.log, f("sum", "Total tax due", "money", { c: 1 }), { value: "12,500.00", locked: true, zeroWhenBlank: true });
  const zero = mockField(props.log, f("zero", "Penalties", "money", { c: 1 }), { value: "", locked: true, zeroWhenBlank: true });
  const locked = mockField(props.log, f("lockedAmt", "Creditable tax", "money", { a: 1 }), { value: "3,000.00", locked: true }, { reason: "From tax profile" });
  const waits = mockField(props.log, f("waits", "Tax rate amount", "money"), { value: "", locked: true, hint: "Needs the ATC" });
  const pct = mockField(props.log, f("pct", "Tax rate", "pct"), { value: "8" }, { width: 88 });
  const bad = mockField(props.log, f("bad", "With an error", "money"), { value: "12", invalid: true });
  const off = mockField(props.log, f("off", "Turned off by another answer", "money"), { value: "", off: "Needs item 13 = Yes" });
  const disabled = mockField(props.log, f("disabled", "Disabled", "text"), { value: "Not now" }, { disabled: true, width: 230 });
  return (
    <Section title="Amounts and rates" testID="controls-amounts">
      <Row log={props.log} field={money.field}><FieldControl {...money} /></Row>
      <Row log={props.log} field={whole.field}><FieldControl {...whole} /></Row>
      <Row log={props.log} field={sum.field} hint="Computed"><FieldControl {...sum} /></Row>
      <Row log={props.log} field={zero.field} hint="Computed, blank"><FieldControl {...zero} /></Row>
      <Row log={props.log} field={locked.field} hint="From the tax profile"><FieldControl {...locked} /></Row>
      <Row log={props.log} field={waits.field} hint="Locked and blank"><FieldControl {...waits} /></Row>
      <Row log={props.log} field={pct.field}><FieldControl {...pct} /></Row>
      <Row log={props.log} field={bad.field} hint="Error state"><FieldControl {...bad} /></Row>
      <Row log={props.log} field={off.field} hint="Faded"><FieldControl {...off} /></Row>
      <Row log={props.log} field={disabled.field} hint="Disabled"><FieldControl {...disabled} /></Row>
    </Section>
  );
}

function TinSection(props: { log: Reports }) {
  const full = mockField(props.log, f("tin", "Signatory TIN", "text", { m: 14 }), { value: "00012345300000" }, { width: 230 });
  const parts = [
    mockField(props.log, f("tin1", "TIN1", "text", { m: 3 }), { value: "000" }, { width: 58 }),
    mockField(props.log, f("tin2", "TIN2", "text", { m: 3 }), { value: "123" }, { width: 58 }),
    mockField(props.log, f("tin3", "TIN3", "text", { m: 3 }), { value: "453" }, { width: 58 }),
    mockField(props.log, f("branch", "Branch code", "text", { m: 5 }), { value: "00000" }, { width: 76 }),
  ];
  const own = mockField(props.log, f("ownTin", "Taxpayer TIN", "text", { m: 3, a: 1 }), { value: "000", locked: true }, { width: 58 });
  // The phone grid's cells (PR 8): tinCell only under 600 pt, as the form page passes it; centred digits, a locked
  // cell without a lock of its own.
  const cell = (k: string, l: string, m: number, value: string, locked = false) =>
    mockField(props.log, f(k, l, "text", { m, ...(locked ? { a: 1 } : {}) }), { value, locked }, { tinCell: narrowNow() });
  const grid = [
    cell("gtin1", "TIN1", 3, "000"),
    cell("gtin2", "TIN2", 3, "123"),
    cell("gtin3", "TIN3", 3, "453"),
    cell("gbranch", "Branch code", 5, "00000"),
    cell("gtinL", "Taxpayer TIN", 3, "000", true),
  ];
  return (
    <Section title="TIN" testID="controls-tin">
      <Row log={props.log} field={full.field} hint="One box, 14 digits"><FieldControl {...full} /></Row>
      <FieldRow label="TIN in four boxes" hint="3 - 3 - 3 - 5 digits" narrow={narrowNow}>
        <View class="flex-row flex-wrap items-center gap-1.5">
          <FieldControl {...parts[0]} />
          <Text tone="fg3">-</Text>
          <FieldControl {...parts[1]} />
          <Text tone="fg3">-</Text>
          <FieldControl {...parts[2]} />
          <Text tone="fg3">-</Text>
          <FieldControl {...parts[3]} />
        </View>
        <Text variant="cap" tone="fg3" class="mt-1">{`tin2 edit: ${props.log.edits().tin2 ?? ""} | blur: ${props.log.blurs().tin2 ?? 0} | focus: ${props.log.focuses().tin2 ?? 0}`}</Text>
      </FieldRow>
      <FieldRow label="Locked TIN box" narrow={narrowNow}>
        <View class="flex-row"><FieldControl {...own} /></View>
      </FieldRow>
      {/* The phone grid exists only under 600 pt (the page passes tinCell then); a desktop window shows the dashed row above. */}
      <If when={narrowNow()}>
        <FieldRow label="TIN grid cells" hint="The phone row's grid: equal columns, the branch code's 1.4" narrow={narrowNow}>
          <View class="flex-row gap-1.5">
            {grid.map((c, i) => (
              <View class="min-w-0" style={{ flexGrow: i === 3 ? 1.4 : 1, flexBasis: 0 }}>
                <FieldControl {...c} />
              </View>
            ))}
          </View>
        </FieldRow>
      </If>
    </Section>
  );
}

function DateSection(props: { log: Reports }) {
  const date = mockField(props.log, f("date", "Date of birth", "date"), { value: "Dec 10, 2020" }, { width: 140 });
  const fixed = mockField(props.log, f("fixedDate", "Date filed", "date", { c: 1 }), { value: "Mar 31, 2026", locked: true }, { width: 140 });
  return (
    <Section title="Dates" testID="controls-dates">
      <Row log={props.log} field={date.field} hint="Typed or picked"><FieldControl {...date} /></Row>
      <Row log={props.log} field={fixed.field} hint="Locked"><FieldControl {...fixed} /></Row>
    </Section>
  );
}

function ChoiceSection(props: { log: Reports }) {
  // What a pill step does: the page keeps each box's focus function and calls it. The buttons stand for the step.
  let focusAtc: (() => void) | undefined;
  let focusRegime: (() => void) | undefined;
  const month = mockField(props.log, f("month", "Fiscal start month", "sel", { o: MONTHS }), { value: "03" }, { width: 260 });
  const atc = mockField(props.log, f("atc", "ATC", "sel"), { value: "", options: ATC }, { width: 360, focusRef: (focus) => (focusAtc = focus) });
  const waits = mockField(props.log, f("atcWait", "ATC (locked)", "sel"), { value: "", locked: true, hint: "Needs the item above" }, { width: 260 });
  const chk = mockField(props.log, f("chk", "I confirm the amounts are true and correct", "chk"), { value: "false" });
  const lockedChk = mockField(props.log, f("lockedChk", "Amended return", "chk"), { value: "false", locked: true, hint: "Set by the period" });
  const regime = mockRadios(props.log, "regime", ["Graduated income tax rate", "8% flat rate", "Mixed income"]);
  const pair = mockRadios(props.log, "amended", ["Yes", "No"], { selected: "amended2" });
  const shut = mockRadios(props.log, "taxType", ["Individual", "Corporation"], { locked: true, hint: "Set by the taxpayer type" });
  // Five to twelve choices draw as a dropdown, more as a search list, and a bar whose labels cannot hold one line stacks.
  const payment = mockRadios(props.log, "payment", [
    "Self-Assessment",
    "Penalties",
    "Tax Deposit/Advance Payment",
    "Income Tax Second Installment",
    "Preliminary/Final Assessment/Deficiency Tax",
    "Accounts Receivable/Delinquent Account",
    "Others (Specify)",
  ]);
  const codes = mockRadios(props.log, "codes", Array.from({ length: 13 }, (_, i) => `Code ${i + 1} — a longer descriptive label`));
  const longrate = mockRadios(props.log, "longrate", [
    "Graduated Rates per Tax Table",
    "8% on gross sales/receipts and other non-operating income",
    "Special/Preferential Rate under a special law or international tax treaty",
  ]);
  const taxpayer = mockRadios(props.log, "taxpayer", ["Itemized Deduction", "Optional Standard Deduction (OSD)"]);
  const spouse = mockRadios(props.log, "spouse", ["Itemized Deduction", "Optional Standard Deduction (OSD)"]);
  return (
    <Section title="Choices" testID="controls-choices">
      <Row log={props.log} field={month.field} hint="A few choices"><FieldControl {...month} /></Row>
      <Row log={props.log} field={atc.field} hint="A long list, searched by words">
        <FieldControl {...atc} />
        <AppButton testID="gallery-focus-atc" size="sm" label="Focus the ATC list" onPress={() => focusAtc?.()} />
      </Row>
      <Row log={props.log} field={waits.field} hint="Locked and blank"><FieldControl {...waits} /></Row>
      <Row log={props.log} field={chk.field} title="Declaration"><FieldControl {...chk} /></Row>
      <Row log={props.log} field={lockedChk.field} title="Return type" hint="Locked, with its reason"><FieldControl {...lockedChk} /></Row>
      <FieldRow label="Income tax rate" hint="A group of three: Clear returns it to unanswered" narrow={narrowNow}>
        <RadioGroupControl {...regime} focusRef={(focus) => (focusRegime = focus)} />
        <AppButton testID="gallery-focus-regime" size="sm" label="Focus the rate group" onPress={() => focusRegime?.()} />
        <Text variant="cap" tone="fg3" class="mt-1">{`edit regime: ${picked(props.log, "regime")}`}</Text>
      </FieldRow>
      <FieldRow label="Amended return" hint="A pair keeps one answer" narrow={narrowNow}>
        <RadioGroupControl {...pair} />
        <Text variant="cap" tone="fg3" class="mt-1">{`edit amended: ${picked(props.log, "amended")}`}</Text>
      </FieldRow>
      <FieldRow label="Taxpayer type" hint="Locked group" narrow={narrowNow}>
        <RadioGroupControl {...shut} />
        <Text variant="cap" tone="fg3" class="mt-1">{`edit taxType: ${picked(props.log, "taxType")}`}</Text>
      </FieldRow>
      <FieldRow label="Manner of payment" hint="Seven choices: a dropdown, None clears it" narrow={narrowNow}>
        <RadioGroupControl {...payment} />
        <Text variant="cap" tone="fg3" class="mt-1">{`edit payment: ${picked(props.log, "payment")}`}</Text>
      </FieldRow>
      <FieldRow label="Tax codes" hint="Thirteen choices: a search list" narrow={narrowNow}>
        <RadioGroupControl {...codes} />
        <Text variant="cap" tone="fg3" class="mt-1">{`edit codes: ${picked(props.log, "codes")}`}</Text>
      </FieldRow>
      <FieldRow label="Tax rate" hint="Long labels stack one under another" narrow={narrowNow}>
        <RadioGroupControl {...longrate} />
        <Text variant="cap" tone="fg3" class="mt-1">{`edit longrate: ${picked(props.log, "longrate")}`}</Text>
      </FieldRow>
      <FieldRow label="Method of deduction" hint="Two one-answer groups in one item" narrow={narrowNow}>
        <View class="flex-row flex-wrap gap-x-6 gap-y-3">
          <View class="gap-1">
            <Text variant="meta" tone="fg2">Taxpayer</Text>
            <RadioGroupControl {...taxpayer} />
          </View>
          <View class="gap-1">
            <Text variant="meta" tone="fg2">Spouse</Text>
            <RadioGroupControl {...spouse} />
          </View>
        </View>
        <Text variant="cap" tone="fg3" class="mt-1">{`edit taxpayer: ${picked(props.log, "taxpayer")} | spouse: ${picked(props.log, "spouse")}`}</Text>
      </FieldRow>
    </Section>
  );
}

const PAYEES: SchemaItem = {
  no: "9",
  l: "Schedule of payees",
  p: 1,
  rows: 4,
  cols: [
    { l: "Payee", t: "text" },
    { l: "ATC", t: "sel", o: ["WI010 - Professional fees", "WI100 - Rentals", "WC010 - Professional fees, juridical"] },
    { l: "Amount", t: "money" },
    { l: "Withheld", t: "money", c: 1 },
    { l: "Resident", t: "chk" },
  ],
  keys: [0, 1, 2, 3].map((r) => ["pay", "atc", "amt", "tax", "res"].map((c) => `sch${r + 1}${c}`)),
};

function ScheduleSection(props: { log: Reports }) {
  const table = mockSchedule(props.log, PAYEES, { sch1pay: "Santos Rentals", sch1atc: "WI100", sch1amt: "12,000.00", sch1tax: "600.00", sch2pay: "Cruz Law", sch2atc: "WI010", sch2amt: "30,000.00" });
  return (
    <Section title="Schedule table" testID="controls-schedule">
      <FieldRow label="Payees" hint="Each row opens in a drawer" narrow={narrowNow}>
        <ScheduleControl {...table} />
        <Text variant="cap" tone="fg3" class="mt-1">{`cleared rows: ${table.cleared().join(",")}`}</Text>
      </FieldRow>
    </Section>
  );
}

/** Every control, in sections down one scrolling page. */
export function ControlsGallery() {
  const log = reports();
  const tab = signal(0);
  void tab;
  return (
    <View class="flex-1 bg-background">
      <Scroll testID="controls-scroll" class="flex-1" contentClass="mx-auto w-full max-w-[880px] gap-4 p-4">
        <Text variant="page">Field controls</Text>
        <TextSection log={log} />
        <AmountSection log={log} />
        <TinSection log={log} />
        <DateSection log={log} />
        <ChoiceSection log={log} />
        <ScheduleSection log={log} />
      </Scroll>
    </View>
  );
}
