// The base wave's only screen: the theme on one page, so the colors, the type scale, tabular figures and the
// Rust bridge can be looked at (nsh run, or the light/dark screenshots in shots/). B3 replaces it with the frame.
//
// Everything here follows the rules the next screens follow: colors are classes naming theme tokens (bg-card,
// text-muted-foreground), sizes come from the type scale (text-body) and the 4 pt grid (p-4), nothing is a raw
// value, props are read in JSX or closures and never destructured.
import { Bridge } from "ebirforms/native";
import type { JSX } from "nsh";
import { colorScheme, platform, Scroll, signal, Text, View, windowSize } from "nsh";
import { Button } from "nsh/base";
import { BREAK, SIZE, TYPE } from "../../theme/tokens";

interface Swatch {
  name: string;
  bg: string;
}

const SURFACES: Swatch[] = [
  { name: "background", bg: "bg-background" },
  { name: "card", bg: "bg-card" },
  { name: "raised", bg: "bg-raised" },
  { name: "hover", bg: "bg-hover" },
  { name: "selected", bg: "bg-selected" },
];
const LINES_AND_ACTIONS: Swatch[] = [
  { name: "border", bg: "bg-border" },
  { name: "border-strong", bg: "bg-border-strong" },
  { name: "primary", bg: "bg-primary" },
  { name: "accent-blue", bg: "bg-accent-blue" },
  { name: "accent-blue-soft", bg: "bg-accent-blue-soft" },
];
const STATUS: Swatch[] = [
  { name: "success", bg: "bg-success" },
  { name: "warning", bg: "bg-warning" },
  { name: "danger", bg: "bg-danger" },
  { name: "success-soft", bg: "bg-success-soft" },
  { name: "warning-soft", bg: "bg-warning-soft" },
  { name: "danger-soft", bg: "bg-danger-soft" },
];

interface TextTone {
  name: string;
  cls: string;
}

const TEXT_TONES: TextTone[] = [
  { name: "foreground", cls: "text-foreground" },
  { name: "muted-foreground (fg2)", cls: "text-muted-foreground" },
  { name: "subtle-foreground (fg3, 4.5:1 or better)", cls: "text-subtle-foreground" },
];

interface TypeRow {
  name: string;
  size: number;
  weight: number;
  cls: string;
}

const TYPE_ROWS: TypeRow[] = Object.entries(TYPE).map(([name, t]) => ({ name, size: t.size, weight: t.weight, cls: t.cls }));

interface SampleRow {
  id: string;
  title: string;
  sub: string;
  amount: string;
  status: "Filed" | "Due soon" | "Overdue";
}

// The demo taxpayer's TIN only (Juana Santos, 000-123-453-00000); never real data.
const SAMPLE_ROWS: SampleRow[] = [
  { id: "r1", title: "2550Q  Quarterly VAT", sub: "TIN 000-123-453-00000", amount: "12,480.50", status: "Filed" },
  { id: "r2", title: "1701Q  Quarterly income tax", sub: "TIN 000-123-453-00000", amount: "1,111.11", status: "Due soon" },
  { id: "r3", title: "0619-E  Monthly withholding", sub: "TIN 000-123-453-00000", amount: "8,888.88", status: "Overdue" },
];

const STATUS_CLASSES: Record<SampleRow["status"], string> = {
  Filed: "bg-success-soft text-success",
  "Due soon": "bg-warning-soft text-warning",
  Overdue: "bg-danger-soft text-danger",
};

function SwatchTile(props: Swatch) {
  return (
    <View class="w-28 gap-1">
      <View class={`h-10 rounded-(--radius-ctl) border border-border ${props.bg}`} />
      <Text class="text-cap font-medium text-muted-foreground">{props.name}</Text>
    </View>
  );
}

function SwatchGroup(props: { title: string; items: Swatch[] }) {
  return (
    <View class="gap-2">
      <Text class="text-meta font-medium text-muted-foreground">{props.title}</Text>
      <View class="flex-row flex-wrap gap-3">
        <Each of={props.items} key={(s) => s.name}>
          {(s) => <SwatchTile name={s().name} bg={s().bg} />}
        </Each>
      </View>
    </View>
  );
}

function Section(props: { title: string; children?: JSX.Element }) {
  return (
    <View class="gap-3 rounded-(--radius-card) border border-border bg-card p-4">
      <Text class="text-title font-semibold text-foreground">{props.title}</Text>
      {props.children}
    </View>
  );
}

function RowItem(props: SampleRow) {
  return (
    <View class="min-h-13 flex-row items-center gap-3 border-b border-border py-2">
      <View class="flex-1 gap-0.5">
        <Text class="text-body font-medium text-foreground">{props.title}</Text>
        <Text class="text-meta tabular-nums text-subtle-foreground">{props.sub}</Text>
      </View>
      <Text class={`rounded-full px-2 py-0.5 text-cap font-medium ${STATUS_CLASSES[props.status]}`}>{props.status}</Text>
      <Text class="w-24 text-right text-body font-medium tabular-nums text-foreground">{props.amount}</Text>
    </View>
  );
}

export function ThemePage() {
  // The Rust bridge: called once at start, and again by the button. The count is state of this page.
  const reply = signal(Bridge.ping());
  const pings = signal(1);
  const ping = () => {
    reply.set(Bridge.ping());
    pings.update((n) => n + 1);
  };
  const layout = () => (windowSize().width >= BREAK.phone ? "wide" : "narrow");
  const env = () =>
    `${platform.os} | ${colorScheme()} | ${Math.round(windowSize().width)} x ${Math.round(windowSize().height)} pt | ${layout()}`;

  return (
    <View class="flex-1 bg-background">
      <Scroll testID="page-scroll" class="flex-1" contentClass="items-center p-4 wide:p-6">
        <View class="w-full gap-4" style={{ maxWidth: SIZE.contentMax }}>
          <View class="gap-1">
            <Text class="text-page font-semibold text-foreground">eBIRForms base</Text>
            <Text class="text-body text-muted-foreground">{env()}</Text>
          </View>

          <Section title="Rust bridge">
            <Text class="text-body text-foreground">{`Native module says: ${reply()}`}</Text>
            <Text class="text-meta tabular-nums text-subtle-foreground">{`Pings: ${pings()}`}</Text>
            <Button
              onPress={ping}
              class="h-8 items-center justify-center self-start rounded-(--radius-ctl) bg-primary px-3 active:opacity-80"
            >
              <Text class="text-body font-medium text-primary-foreground">Ping the Rust bridge</Text>
            </Button>
          </Section>

          <Section title="Colors">
            <SwatchGroup title="Surfaces" items={SURFACES} />
            <SwatchGroup title="Lines, ink, accent" items={LINES_AND_ACTIONS} />
            <SwatchGroup title="Status" items={STATUS} />
            <View class="gap-1">
              <Text class="text-meta font-medium text-muted-foreground">Text</Text>
              <Each of={TEXT_TONES} key={(t) => t.name}>
                {(t) => <Text class={`text-body ${t().cls}`}>{t().name}</Text>}
              </Each>
            </View>
          </Section>

          <View class="gap-4 wide:flex-row">
            <View class="gap-4 wide:flex-1">
              <Section title="Type scale (Inter)">
                <Each of={TYPE_ROWS} key={(r) => r.name}>
                  {(r) => (
                    <View class="flex-row items-baseline justify-between gap-3">
                      <Text class={`flex-1 text-foreground ${r().cls}`}>Form 2550Q, Juana Santos</Text>
                      <Text class="text-cap tabular-nums text-subtle-foreground">
                        {`${r().name} ${r().size}/${r().weight}`}
                      </Text>
                    </View>
                  )}
                </Each>
              </Section>
              <Section title="Tabular figures">
                <View class="gap-1">
                  <Text class="text-meta text-subtle-foreground">Proportional (default)</Text>
                  <Text class="w-28 text-right text-body text-foreground">{"1,111.11\n0,000.00\n8,888.88"}</Text>
                </View>
                <View class="gap-1">
                  <Text class="text-meta text-subtle-foreground">tabular-nums</Text>
                  <Text class="w-28 text-right text-body tabular-nums text-foreground">{"1,111.11\n0,000.00\n8,888.88"}</Text>
                </View>
              </Section>
            </View>
            <View class="gap-4 wide:flex-1">
              <Section title="Sample rows">
                <View>
                  <Each of={SAMPLE_ROWS} key={(r) => r.id}>
                    {(r) => <RowItem id={r().id} title={r().title} sub={r().sub} amount={r().amount} status={r().status} />}
                  </Each>
                </View>
              </Section>
            </View>
          </View>
        </View>
      </Scroll>
    </View>
  );
}
