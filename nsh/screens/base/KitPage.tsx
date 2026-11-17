// The UI kit's gallery: every component of nsh/ui in its states (default, selected, disabled, error), the masks with
// their live values, and the layers behind buttons. A preview for people and screenshots (shots/kit.test.tsx), not a
// screen of the app; built only from nsh/ui, as screens are. Data is generated or the demo taxpayer's.
import type { JSX } from "nsh";
import { colorScheme, platform, Scroll, signal, View, windowSize } from "nsh";
import { SIZE } from "../../theme/tokens";
import {
  AppButton, Avatar, Banner, Card, Checkbox, CheckboxLabel, Chip, Code, ComboField, ContextPill, DateField, Dialog, Disclosure,
  DisclosureGroup, Dropdown, EmptyRow, Field, FieldCell, FieldGrid, FormActions, GroupLabel, Hairline, Icon, IconButton, type IconName,
  ICONS, IntInput, Kbd, Line, ListGroup, ListRow, Menu, MenuItem, MenuSeparator, MetaLine, MoneyInput, MultiCombo, NameCell, NavRow,
  PageHeader, PageTitle, PercentInput, PinCells, Popover, PressCard, PressCell, RadioGroup, SectionTitle, Segmented, SettingRow, Sheet, ShortcutRecorder, SidePanel, Spinner,
  StatusChip, StepCircle, Switch, Tabs, Text, TextField, TinInput, toast, Toaster, undoToast,
} from "../../ui";

// The demo taxpayer (Juana Santos, TIN 000-123-453-00000) and generated rows only.
const DEMO = { name: "Juana Santos", tin: "000-123-453-00000", root: "000123453" };

/** Revenue district offices: a few named, the rest generated, so the list is as long as the real one. */
const RDO = [
  { value: "050", label: "050 - South Makati" },
  { value: "047", label: "047 - East Makati" },
  { value: "039", label: "039 - South Quezon City" },
  ...Array.from({ length: 120 }, (_, i) => {
    const code = String(60 + i).padStart(3, "0");
    return { value: code, label: `${code} - District ${i + 1}` };
  }),
];

const TAXPAYERS = [
  { value: "000123453", label: DEMO.name, sub: DEMO.tin, lead: () => <Avatar name={DEMO.name} size={20} hueKey={DEMO.root} /> },
  ...Array.from({ length: 5 }, (_, i) => {
    const name = `Sample Taxpayer ${i + 2}`;
    const root = `00099900${i}`;
    return { value: root, label: name, sub: `000-999-00${i}-00000`, lead: () => <Avatar name={name} size={20} hueKey={root} /> };
  }),
];

const ICON_NAMES = Object.keys(ICONS) as IconName[];

function Section(props: { title: string; testID?: string; children?: JSX.Element }) {
  return (
    <View testID={props.testID} class="gap-4 rounded-(--radius-card) border border-border bg-card p-4">
      <Text variant="title">{props.title}</Text>
      {props.children}
    </View>
  );
}

function Demo(props: { label: string; children?: JSX.Element; column?: boolean }) {
  return (
    <View class="gap-2">
      <Text variant="cap" tone="fg3">{props.label}</Text>
      <View class={props.column ? "items-start gap-2" : "flex-row flex-wrap items-center gap-2"}>{props.children}</View>
    </View>
  );
}

function TextSection() {
  return (
    <Section title="Text and marks" testID="kit-text">
      <Demo label="Type scale" column>
        <PageTitle>Page title 20/600</PageTitle>
        <Text variant="title">Title 16/600</Text>
        <Text variant="emph">Emphasis 14/600</Text>
        <Text>Body 13/400, the default</Text>
        <Text variant="control">Control 13/500</Text>
        <Text variant="meta" tone="fg2">Meta 12/400 in fg2</Text>
        <Text variant="metaStrong" tone="fg3">Meta strong 12/500 in fg3</Text>
        <Text variant="cap" tone="fg3">Caption 11/500</Text>
      </Demo>
      <Demo label="Tones">
        <Text tone="fg">fg</Text>
        <Text tone="fg2">fg2</Text>
        <Text tone="fg3">fg3</Text>
        <Text tone="accent">accent</Text>
        <Text tone="success">success</Text>
        <Text tone="warning">warning</Text>
        <Text tone="danger">danger</Text>
      </Demo>
      <Demo label="Line truncates, Code is tabular">
        <View class="w-40 rounded-(--radius-ctl) border border-border px-2 py-1">
          <Line>Annual income tax return for individuals</Line>
        </View>
        <Code>{DEMO.tin}</Code>
        <Code tone="fg2">2550Q</Code>
      </Demo>
      <View>
        <SectionTitle first>Section title</SectionTitle>
        <GroupLabel>Group label</GroupLabel>
        <MetaLine parts={["2550Q", "Q3 2026", <StatusChip status="due_soon" days={3} />]} />
      </View>
      <Demo label="Kbd, spinner, hairline">
        <Kbd>⌘K</Kbd>
        <Kbd>Ctrl Shift B</Kbd>
        <Spinner />
        <View class="w-24">
          <Hairline />
        </View>
      </Demo>
      <Demo label={`Icons (${ICON_NAMES.length})`}>
        <Each of={ICON_NAMES}>{(n) => <Icon name={n()} />}</Each>
      </Demo>
      <Demo label="Icon tones and sizes">
        <Icon name="circle-check" tone="success" />
        <Icon name="triangle-alert" tone="warning" />
        <Icon name="info" tone="accent" />
        <Icon name="trash" tone="danger" />
        <Icon name="search" size={12} />
        <Icon name="search" size={14} />
        <Icon name="search" size={16} tone="fg" />
      </Demo>
    </Section>
  );
}

function ButtonSection() {
  const presses = signal(0);
  const sidebar = signal(true);
  const page = signal("home");
  const press = () => presses.update((n) => n + 1);
  return (
    <Section title="Buttons and rows" testID="kit-buttons">
      <Text tone="fg2">{`Presses: ${presses()}`}</Text>
      <Demo label="Variants">
        <AppButton label="Primary" variant="primary" onPress={press} testID="kit-primary" />
        <AppButton label="Secondary" onPress={press} />
        <AppButton label="Ghost" variant="ghost" onPress={press} />
        <AppButton label="Danger" variant="danger" icon="trash" onPress={press} />
      </Demo>
      <Demo label="Small, icon, trailing, selected">
        <AppButton label="New return" variant="primary" size="sm" icon="plus" onPress={press} />
        <AppButton label="Print" size="sm" icon="printer" onPress={press} />
        <AppButton label="Search" size="sm" icon="search" trailing={<Kbd>⌘K</Kbd>} onPress={press} />
        <AppButton label="Filters" size="sm" variant="ghost" selected onPress={press} />
      </Demo>
      <Demo label="Disabled">
        <AppButton label="Primary" variant="primary" disabled testID="kit-primary-off" onPress={press} />
        <AppButton label="Secondary" disabled onPress={press} />
        <AppButton label="Danger" variant="danger" disabled onPress={press} />
      </Demo>
      <Demo label="Block and hint" column>
        <AppButton label="Continue" variant="primary" block onPress={press} />
        <AppButton label="Start a blank return" hint="Every box empty; nothing carried over from your profile." block onPress={press} />
      </Demo>
      <Demo label="Icon buttons (tooltips), pressed, disabled, dot">
        <IconButton icon="panel-left" tooltip="Hide sidebar" shortcut="⌘B" pressed={sidebar()} onPress={() => sidebar.update((v) => !v)} testID="kit-sidebar" />
        <IconButton icon="bell" tooltip="Notifications" dot onPress={press} />
        <IconButton icon="settings" tooltip="Settings" onPress={press} />
        <IconButton icon="trash" tooltip="Delete" disabled onPress={press} />
      </Demo>
      <Demo label="Nav rows" column>
        <View class="w-full gap-0.5" style={{ maxWidth: SIZE.sidebar }}>
          <NavRow label="Home" leading="house" current={page() === "home"} onPress={() => page.set("home")} testID="kit-nav-home" />
          <NavRow label="Form library" leading="file-text" kbd="⌘L" current={page() === "forms"} onPress={() => page.set("forms")} testID="kit-nav-forms" />
          <NavRow label="Calendar" leading="calendar" count={4} current={page() === "calendar"} onPress={() => page.set("calendar")} />
          <NavRow label={DEMO.name} leading={<Avatar name={DEMO.name} size={20} hueKey={DEMO.root} />} onPress={() => page.set("tp")} />
        </View>
      </Demo>
    </Section>
  );
}

function RowsSection() {
  const tab = signal("profile");
  const sound = signal(true);
  const theme = signal("system");
  return (
    <Section title="Lists, cards, headers" testID="kit-rows">
      <PageHeader
        title={DEMO.name}
        avatar={<Avatar name={DEMO.name} size={40} hueKey={DEMO.root} />}
        meta={<MetaLine parts={[DEMO.tin, "Individual", "RDO 050"]} />}
        actions={<AppButton label="Edit" size="sm" icon="pencil" onPress={() => {}} />}
      />
      <Tabs
        tabs={[
          { value: "profile", label: "Profile" },
          { value: "forms", label: "Forms", count: 12 },
          { value: "templates", label: "Templates" },
        ]}
        value={tab()}
        onChange={(v) => tab.set(v)}
        testID="kit-tabs"
      />
      <Text tone="fg2">{`Tab: ${tab()}`}</Text>
      <ListGroup>
        <ListRow onPress={() => toast("Opened 2550Q")} accessibilityLabel="2550Q Quarterly VAT">
          <NameCell title="2550Q  Quarterly VAT" sub="Q3 2026" />
          <StatusChip status="filed" />
          <Text class="w-24 text-right tabular-nums">12,480.50</Text>
        </ListRow>
        <ListRow onPress={() => {}}>
          <NameCell title="1701Q  Quarterly income tax" sub="Q3 2026" />
          <StatusChip status="due_soon" days={3} />
          <Text class="w-24 text-right tabular-nums">1,111.11</Text>
        </ListRow>
        <ListRow>
          <NameCell title="0619-E  Monthly withholding" sub="October 2026" dim />
          <StatusChip status="overdue" days={-2} />
          <Text class="w-24 text-right tabular-nums">8,888.88</Text>
        </ListRow>
      </ListGroup>
      <ListGroup>
        <EmptyRow text="No returns filed yet." action={<AppButton label="New return" size="sm" icon="plus" onPress={() => {}} />} />
      </ListGroup>
      <Card>
        <SettingRow title="Sounds" hint="A soft click on save and send.">
          <Switch checked={sound()} onChange={(v) => sound.set(v)} label="Sounds" />
        </SettingRow>
        <SettingRow title="Appearance" hint="Follows the system unless set.">
          <Dropdown
            label="Appearance"
            width={140}
            value={theme()}
            onChange={(v) => theme.set(v)}
            options={[
              { value: "system", label: "System" },
              { value: "light", label: "Light" },
              { value: "dark", label: "Dark" },
            ]}
          />
        </SettingRow>
      </Card>
    </Section>
  );
}

function PressSection() {
  const presses = signal(0);
  const picked = signal(false);
  const day = signal("11");
  return (
    <Section title="Pressable cards and cells" testID="kit-press">
      <Text tone="fg2" testID="kit-press-count">{`Card presses: ${presses()}`}</Text>
      <Demo label="Tile, lift, selected, disabled">
        <PressCard testID="kit-press-tile" label="Form 2551Q" lift class="w-48 gap-2" onPress={() => presses.update((n) => n + 1)}>
          <Text variant="title">2551Q</Text>
          <Text variant="control" lines={2}>Quarterly Percentage Tax Return</Text>
        </PressCard>
        <PressCard testID="kit-press-pick" label="Draft: 3" selected={picked()} align="start" class="w-36 gap-0.5" onPress={() => picked.update((v) => !v)}>
          <Text variant="meta" tone="fg2">Draft</Text>
          <Text variant="page" class="tabular-nums">3</Text>
          <Text variant="meta" tone="fg3">{picked() ? "Picked" : "Not picked"}</Text>
        </PressCard>
        <PressCard testID="kit-press-off" label="Not available" disabled class="w-36 gap-0.5">
          <Text variant="control">Not available</Text>
        </PressCard>
      </Demo>
      <Demo label="Row card" column>
        <PressCard testID="kit-press-row" label="2551Q Q3 2026" size="row" class="w-full gap-1" onPress={() => presses.update((n) => n + 1)}>
          <Text variant="emph">2551Q Q3 2026</Text>
          <Text variant="meta" tone="fg2">Juana Santos</Text>
        </PressCard>
      </Demo>
      <Demo label="Cells: fill and outline">
        <View class="h-14 w-40 flex-row gap-1">
          <PressCell testID="kit-cell-11" label="November 11" selected={day() === "11"} class="flex-1 items-center justify-center rounded-(--radius-row)" onPress={() => day.set("11")}>
            <Text variant="control">11</Text>
          </PressCell>
          <PressCell testID="kit-cell-12" label="November 12" selected={day() === "12"} class="flex-1 items-center justify-center rounded-(--radius-row)" onPress={() => day.set("12")}>
            <Text variant="control">12</Text>
          </PressCell>
        </View>
        <View class="h-14 w-40 flex-row gap-1">
          <PressCell testID="kit-strip-13" label="November 13" tone="outline" selected={day() === "13"} class="flex-1 items-center justify-center rounded-(--radius-ctl)" onPress={() => day.set("13")}>
            <Text variant="control">13</Text>
          </PressCell>
          <PressCell testID="kit-strip-14" label="November 14" tone="outline" selected={day() === "14"} class="flex-1 items-center justify-center rounded-(--radius-ctl)" onPress={() => day.set("14")}>
            <Text variant="control">14</Text>
          </PressCell>
        </View>
        <Text tone="fg2" testID="kit-press-day">{`Day: ${day()}`}</Text>
      </Demo>
    </Section>
  );
}

function StatusSection() {
  const open = signal(false);
  return (
    <Section title="Identity and status" testID="kit-status">
      <Demo label="Avatars 20, 28, 40, 56">
        <Avatar name={DEMO.name} size={20} hueKey={DEMO.root} />
        <Avatar name={DEMO.name} size={28} hueKey={DEMO.root} />
        <Avatar name={DEMO.name} size={40} hueKey={DEMO.root} />
        <Avatar name={DEMO.name} size={56} hueKey={DEMO.root} />
        <Avatar name="Sample Taxpayer Two" size={40} />
        <Avatar name="dela cruz" size={40} />
      </Demo>
      <Demo label="Context pill: closed, open, compact, empty" column>
        <ContextPill name={DEMO.name} tin={DEMO.tin} hueKey={DEMO.root} open={open()} onPress={() => open.update((v) => !v)} testID="kit-pill" />
        <ContextPill name={DEMO.name} tin={DEMO.tin} hueKey={DEMO.root} open />
        <ContextPill name={DEMO.name} hueKey={DEMO.root} branchCode="00000" compact />
        <ContextPill empty />
      </Demo>
      <Demo label="Step circles">
        <StepCircle label="1" />
        <StepCircle label="2" />
        <StepCircle>
          <Icon name="check" size={14} />
        </StepCircle>
      </Demo>
      <Demo label="Chips">
        <Chip label="Neutral" />
        <Chip label="Accent" tone="accent" />
        <Chip label="Success" tone="success" icon="check" />
        <Chip label="Warning" tone="warning" />
        <Chip label="Danger" tone="danger" />
        <Chip label="Head office" icon="git-branch" />
      </Demo>
      <Demo label="Status chips">
        <StatusChip status="not_started" />
        <StatusChip status="draft" />
        <StatusChip status="queued" />
        <StatusChip status="submitted" />
        <StatusChip status="confirmed" />
        <StatusChip status="paid" />
        <StatusChip status="filed" />
        <StatusChip status="due_soon" days={0} />
        <StatusChip status="due_soon" days={5} />
        <StatusChip status="overdue" days={-1} />
      </Demo>
      <Banner>
        <Icon name="calendar" tone="accent" />
        <Text class="flex-1">Tax year 2027 is open. Add it to start next year's returns.</Text>
        <AppButton label="Add 2027" variant="primary" size="sm" onPress={() => {}} />
      </Banner>
    </Section>
  );
}

function ChoiceSection() {
  const on = signal(true);
  const checked = signal(true);
  const agree = signal(false);
  const regime = signal<string | null>("graduated");
  const view = signal("list");
  const month = signal("oct");
  const form = signal<string | null>(null);
  const rdo = signal<string | null>("050");
  const who = signal<Set<string> | null>(null);
  return (
    <Section title="Choice controls" testID="kit-choice">
      <Demo label="Switch: on, off, disabled">
        <Switch checked={on()} onChange={(v) => on.set(v)} label="Autosave" testID="kit-switch">
          <Text>{on() ? "Autosave on" : "Autosave off"}</Text>
        </Switch>
        <Switch checked={false} onChange={() => {}} label="Off" />
        <Switch checked onChange={() => {}} label="Disabled" disabled />
      </Demo>
      <Demo label="Checkbox: on, off, mixed, disabled; with a label">
        <Checkbox checked={checked()} onChange={(v) => checked.set(v)} label="Include" testID="kit-check" />
        <Checkbox checked={false} onChange={() => {}} label="Off" />
        <Checkbox checked={false} indeterminate onChange={() => {}} label="Some" />
        <Checkbox checked disabled onChange={() => {}} label="Disabled" />
        <CheckboxLabel checked={agree()} onChange={(v) => agree.set(v)} label="I declare this return true and correct" testID="kit-agree" />
      </Demo>
      <Text tone="fg2">{`Include: ${checked() ? "yes" : "no"} | Declared: ${agree() ? "yes" : "no"}`}</Text>
      <Demo label="Radio group" column>
        <RadioGroup
          testID="kit-radio"
          value={regime()}
          onChange={(v) => regime.set(v)}
          options={[
            { value: "graduated", label: "Graduated rates", hint: "Income tax by the table, after deductions" },
            { value: "eight_percent", label: "8% flat rate", hint: "8% of gross sales or receipts" },
            { value: "none", label: "Not chosen yet", disabled: true },
          ]}
        />
      </Demo>
      <Demo label="Segmented">
        <Segmented
          label="View"
          testID="kit-seg"
          value={view()}
          onChange={(v) => view.set(v)}
          options={[
            { value: "list", label: "List", icon: "list" },
            { value: "month", label: "Month", icon: "calendar" },
          ]}
        />
        <Segmented
          label="Month"
          wrap
          value={month()}
          onChange={(v) => month.set(v)}
          options={["jul", "aug", "sep", "oct", "nov", "dec"].map((m) => ({ value: m, label: m[0].toUpperCase() + m.slice(1) }))}
        />
      </Demo>
      <Text tone="fg2">{`View: ${view()} | Regime: ${regime()}`}</Text>
      <Demo label="Dropdown: placeholder, error, disabled">
        <Dropdown
          label="Form"
          testID="kit-dropdown"
          width={220}
          placeholder="Choose a form"
          value={form()}
          onChange={(v) => form.set(v)}
          options={[
            { value: "2550Q", label: "Form 2550Q" },
            { value: "1701Q", label: "Form 1701Q" },
            { value: "0619E", label: "Form 0619-E" },
          ]}
        />
        <Dropdown label="Error" width={160} error value={null} onChange={() => {}} options={[{ value: "a", label: "A" }]} placeholder="Required" />
        <Dropdown label="Disabled" width={160} disabled value="a" onChange={() => {}} options={[{ value: "a", label: "Locked choice" }]} />
      </Demo>
      <Text tone="fg2">{`Form: ${form() ?? "none"}`}</Text>
      <Demo label={`Combo field (${RDO.length} RDOs; type a code or a place)`} column>
        <ComboField label="RDO" testID="kit-rdo" options={RDO} value={rdo()} onChange={(v) => rdo.set(v)} cleanable width={320} />
        <ComboField label="RDO with error" options={RDO} value={null} onChange={() => {}} error width={320} />
      </Demo>
      <Text tone="fg2">{`RDO: ${rdo() ?? "none"}`}</Text>
      <Demo label="Multi combo">
        <MultiCombo
          testID="kit-who"
          all="All taxpayers"
          nouns="taxpayers"
          icon="users"
          placeholder="Find a taxpayer"
          options={TAXPAYERS}
          selected={who()}
          onChange={(v) => who.set(v)}
        />
      </Demo>
      <Text tone="fg2">{`Taxpayers: ${who() === null ? "all" : [...who()!].join(", ") || "none"}`}</Text>
    </Section>
  );
}

function EntrySection() {
  const name = signal(DEMO.name);
  const tin = signal("000123453");
  const amount = signal("1250000");
  const pct = signal("12");
  const qty = signal("4");
  const date = signal("12/01/2020");
  const pin = signal("");
  const code = signal("");
  const done = signal("");
  const shortcut = signal("ctrl+alt+e");
  const bare = signal("");
  const recording = signal(false);
  return (
    <Section title="Text entry" testID="kit-entry">
      <FieldGrid>
        <FieldCell>
          <TextField label="Registered name" value={name()} onChange={(v) => name.set(v)} testID="kit-name" />
        </FieldCell>
        <FieldCell>
          <TextField label="Email" value="" onChange={() => {}} placeholder="name@example.com" hint="For the filing receipt." />
        </FieldCell>
        <FieldCell>
          <TextField label="Trade name" value="Santos Store" onChange={() => {}} error="Use the name on your COR." />
        </FieldCell>
        <FieldCell>
          <TextField label="Disabled" value="Not editable" onChange={() => {}} disabled />
        </FieldCell>
        <FieldCell>
          <TextField label="From tax profile (locked)" value={DEMO.tin} onChange={() => {}} locked tabular />
        </FieldCell>
        <FieldCell>
          <TextField label="Search" icon="search" size="sm" value="" onChange={() => {}} placeholder="Find a form" />
        </FieldCell>
      </FieldGrid>
      <Hairline />
      <FieldGrid>
        <FieldCell>
          <TinInput label="TIN" value={tin()} onChange={(v) => tin.set(v)} testID="kit-tin" />
          <Text variant="meta" tone="fg3" class="mt-1 tabular-nums">{`Digits: ${tin()}`}</Text>
        </FieldCell>
        <FieldCell>
          <MoneyInput label="Gross sales" value={amount()} onChange={(v) => amount.set(v)} testID="kit-money" />
          <Text variant="meta" tone="fg3" class="mt-1 tabular-nums">{`Value: ${amount() || "empty"}`}</Text>
        </FieldCell>
        <FieldCell>
          <PercentInput label="Rate" value={pct()} onChange={(v) => pct.set(v)} width={120} testID="kit-pct" />
        </FieldCell>
        <FieldCell>
          <IntInput label="Branches" value={qty()} onChange={(v) => qty.set(v)} maxDigits={4} width={120} testID="kit-int" />
        </FieldCell>
        <FieldCell>
          <DateField label="Date of birth" value={date()} onChange={(v) => date.set(v)} width={200} testID="kit-date" />
          <Text variant="meta" tone="fg3" class="mt-1 tabular-nums">{`Stored: ${date() || "empty"}`}</Text>
        </FieldCell>
        <FieldCell>
          <MoneyInput label="Tax due (computed)" value="18480.5" onChange={() => {}} locked />
        </FieldCell>
      </FieldGrid>
      <Hairline />
      <FieldGrid>
        <FieldCell>
          <Field label="Registered address" value="123 Mabini St, Makati City" />
        </FieldCell>
        <FieldCell>
          <Field label="ZIP code" value="" />
        </FieldCell>
        <FieldCell>
          <Field label="RDO" value="050 - South Makati" inherited />
        </FieldCell>
        <FieldCell>
          <Field label="Phone" inheritSwitch={<Switch checked onChange={() => {}} label="Same as head office" />}>
            <TextField accessibilityLabel="Phone" value="0917 000 0000" onChange={() => {}} locked />
          </Field>
        </FieldCell>
      </FieldGrid>
      <Demo label="PIN (4 digits, masked)">
        <PinCells length={4} masked label="PIN" value={pin()} onChange={(v) => pin.set(v)} testID="kit-pin" />
      </Demo>
      <Demo label="Authenticator code (6 digits)">
        <PinCells length={6} label="Code" value={code()} onChange={(v) => code.set(v)} onComplete={(v) => done.set(v)} testID="kit-code" />
      </Demo>
      <Text tone="fg2" class="tabular-nums">{`PIN digits: ${pin().length} | Code complete: ${done() || "no"}`}</Text>
      <Demo label="Shortcut recorder: with a shortcut, none, needs Ctrl, Alt or Cmd, with an error, disabled">
        <ShortcutRecorder
          testID="kit-shortcut"
          label="Show shortcut"
          width={200}
          value={shortcut()}
          onChange={(v) => shortcut.set(v)}
          onRecordingChange={(on) => recording.set(on)}
        />
        <ShortcutRecorder
          testID="kit-shortcut-bare"
          label="Needs a modifier"
          width={200}
          value={bare()}
          onChange={(v) => bare.set(v)}
          accepts={(k) => /(^|\+)(ctrl|alt|cmd)\+/.test(k)}
          unacceptable="Add Control, Option or Command."
        />
        <ShortcutRecorder testID="kit-shortcut-taken" label="Taken" width={200} value="ctrl+alt+b" onChange={() => {}} error="Another app is using this shortcut. Pick another one." />
        <ShortcutRecorder testID="kit-shortcut-locked" label="Locked" width={200} value="ctrl+alt+h" onChange={() => {}} disabled />
      </Demo>
      <Text tone="fg2">{`Shortcut: ${shortcut() || "none"} | Recording: ${recording() ? "yes" : "no"} | Needs a modifier: ${bare() || "none"}`}</Text>
      <FormActions onCancel={() => toast("Cancelled")} onSave={() => toast("Saved")} testID="kit-form" />
    </Section>
  );
}

function LayerSection() {
  const dialog = signal(false);
  const palette = signal(false);
  const sheet = signal(false);
  const undone = signal(0);
  const opened = signal<string[]>(["income"]);
  const flip = (id: string) => opened.update((l) => (l.includes(id) ? l.filter((x) => x !== id) : [...l, id]));
  return (
    <Section title="Layers" testID="kit-layers">
      <Demo label="Dialog, palette, sheet">
        <AppButton label="Open dialog" onPress={() => dialog.set(true)} testID="kit-open-dialog" />
        <AppButton label="Open palette" icon="search" onPress={() => palette.set(true)} />
        <AppButton label="Open sheet" onPress={() => sheet.set(true)} testID="kit-open-sheet" />
      </Demo>
      <Demo label="Popover, menu, tooltip">
        <Popover trigger={<AppButton label="Popover" icon="info" />} width={260} testID="kit-popover">
          <Text variant="emph">Where this number comes from</Text>
          <Text tone="fg2">Line 15 of Part II, less the credits you claimed in Part III.</Text>
        </Popover>
        <Menu
          testID="kit-menu"
          trigger={(open) => <AppButton label="More" icon="chevrons-up-down" selected={open()} />}
        >
          <MenuItem label="Duplicate" icon="copy" shortcut="⌘D" onSelect={() => toast("Duplicated")} />
          <MenuItem label="Print" icon="printer" shortcut="⌘P" onSelect={() => toast("Printing")} />
          <MenuItem label="Archived" checked onSelect={() => {}} />
          <MenuSeparator />
          <MenuItem label="Delete return" icon="trash" danger onSelect={() => undoToast("Return deleted", () => undone.update((n) => n + 1))} />
          <MenuItem label="Send to BIR" icon="send" disabled />
        </Menu>
        <IconButton icon="info" tooltip="Hover or focus shows a tooltip" />
      </Demo>
      <Demo label="Toasts">
        <AppButton label="Show toast" onPress={() => toast("Draft saved")} testID="kit-toast" />
        <AppButton label="Delete with undo" variant="danger" onPress={() => undoToast("Return deleted", () => undone.update((n) => n + 1))} testID="kit-undo" />
        <Text tone="fg2">{`Undone: ${undone()}`}</Text>
      </Demo>
      <Demo label="Disclosures" column>
        <DisclosureGroup>
          <Disclosure title="Income" meta="2 sources" open={opened().includes("income")} onToggle={() => flip("income")} onEdit={() => toast("Edit income")} testID="kit-disc-income">
            <Text>Business income, 1,250,000.00</Text>
          </Disclosure>
          <Disclosure title="Deductions" meta="Optional standard deduction" open={opened().includes("ded")} onToggle={() => flip("ded")} testID="kit-disc-ded">
            <Text>40% of gross sales</Text>
          </Disclosure>
        </DisclosureGroup>
      </Demo>
      <Demo label="Side panel" column>
        <View class="h-60 w-full overflow-hidden rounded-(--radius-row) border border-border">
          <SidePanel title="Notifications" action={<AppButton label="Clear" variant="ghost" size="sm" onPress={() => {}} />}>
            <NavRow label="2550Q is due in 3 days" leading="bell" onPress={() => {}} />
            <NavRow label="1701Q filed" leading="circle-check" onPress={() => {}} />
          </SidePanel>
        </View>
      </Demo>
      <Dialog open={dialog()} onOpenChange={(o) => dialog.set(o)} width={440} testID="kit-dialog">
        <View class="gap-3 p-5">
          <Text variant="title">Delete this return?</Text>
          <Text tone="fg2">The 2550Q for Q3 2026 and its draft are removed. You can undo it for five seconds.</Text>
          <View class="mt-2 flex-row justify-end gap-2">
            <AppButton label="Cancel" variant="ghost" onPress={() => dialog.set(false)} testID="kit-dialog-cancel" />
            <AppButton label="Delete" variant="danger" onPress={() => dialog.set(false)} />
          </View>
        </View>
      </Dialog>
      <Dialog open={palette()} onOpenChange={(o) => palette.set(o)} top width={600}>
        <View class="gap-2 p-3">
          <TextField icon="search" value="" onChange={() => {}} placeholder="Search forms, taxpayers, commands" accessibilityLabel="Search" autoFocus />
          <NavRow label="New return" leading="plus" kbd="⌘N" onPress={() => palette.set(false)} />
          <NavRow label="Open Juana Santos" leading="user" onPress={() => palette.set(false)} />
        </View>
      </Dialog>
      <Sheet
        open={sheet()}
        onOpenChange={(o) => sheet.set(o)}
        title="Schedule 1, row 3"
        testID="kit-sheet"
        footer={<FormActions onCancel={() => sheet.set(false)} onSave={() => sheet.set(false)} saveLabel="Save row" />}
      >
        <TextField label="Description" value="Office rent" onChange={() => {}} />
        <MoneyInput label="Amount" value="36000" onChange={() => {}} />
      </Sheet>
    </Section>
  );
}

/** Every kit component on one scrolling page. */
export function KitPage() {
  const env = () => `${platform.os} | ${colorScheme()} | ${Math.round(windowSize().width)} x ${Math.round(windowSize().height)} pt`;
  return (
    <View class="flex-1 bg-background">
      <Scroll testID="kit-scroll" class="flex-1" contentClass="items-center p-4 wide:p-6">
        <View class="w-full gap-4" style={{ maxWidth: SIZE.contentMax }}>
          <View class="gap-1">
            <PageTitle>UI kit</PageTitle>
            <Text tone="fg2">{env()}</Text>
          </View>
          <TextSection />
          <ButtonSection />
          <RowsSection />
          <PressSection />
          <StatusSection />
          <ChoiceSection />
          <EntrySection />
          <LayerSection />
        </View>
      </Scroll>
      <Toaster />
    </View>
  );
}
