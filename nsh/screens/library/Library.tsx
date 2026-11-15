// The Form library (old app/shell/library.js renderLibrary): a search box, the scope ("Filed in <year>" or "All forms"),
// the frequency, the "n of 43" count, and a grid of form cards. A card opens its form as a sample: blank, with no period,
// and never a recent return (openReturn keeps it out of Recent). The filters are this screen's signals; they live at
// module level, as the old app kept them on the app object, so they survive a trip to another page.
import { signal, View } from "nsh";
import { app, openReturn, store, toast } from "../../shell";
import { EmptyRow, PageHeader, Segmented, Text, TextField } from "../../ui";
import { FormTile } from "./FormTile";
import { asFrequency, FREQS, type FormScope, type FrequencyFilter, matchingForms } from "./rules";

const query = signal("");
const freq = signal<FrequencyFilter>("All");
/** null until the person picks a scope: the start route's, else with a taxpayer "owned" (the old default), without one "all". */
const scope = signal<FormScope | null>(null);
// The next session starts from the defaults (a lock, a sign-out or another taxpayer signing in).
app.onSessionChange(() => {
  query.set("");
  freq.set("All");
  scope.set(null);
});

const FREQ_OPTIONS = FREQS.map((x) => ({ value: x, label: x }));

const forms = () => store.screenIndex();
const owned = () => new Set(app.yearForms());
/** The dev start route's `scope=all|owned` (BIR_SHELL_START, as the old app took it); any other value is ignored. */
const startScope = (): FormScope | null => {
  const asked = app.startParams().scope;
  return asked === "all" || asked === "owned" ? asked : null;
};
/** The scope in effect: with no taxpayer every form shows, as in the old app. */
const scopeNow = (): FormScope => (app.taxpayer() ? (scope() ?? startScope() ?? "owned") : "all");
const shown = () => matchingForms(forms(), { scope: scopeNow(), owned: owned(), freq: freq(), query: query() });

const year = () => app.year() ?? "this year";

function lede(): string {
  const tp = app.taxpayer();
  if (!tp) return "Every return eBIRForms can prepare. Add a taxpayer to fill one in.";
  const branch = tp.branches.length > 1 ? ` (${app.branch()?.name ?? ""})` : "";
  return `Forms ${tp.name}${branch} files in ${year()}. Choose All forms to see every return eBIRForms can prepare.`;
}

function emptyText(): string {
  const tp = app.taxpayer();
  if (tp && scopeNow() === "owned" && owned().size === 0) {
    return `${tp.name} has no forms for ${year()} yet. Choose them under Forms, or see All forms.`;
  }
  return "No form matches. Try a code like 1601C or a word like VAT.";
}

function scopeOptions() {
  return [
    { value: "owned", label: `Filed in ${year()} (${owned().size})` },
    { value: "all", label: `All forms (${forms().length})` },
  ];
}

/** A card's press: the form as a sample of the selected branch, for the selected year (this year when none is chosen). */
function openSample(code: string): void {
  const branch = app.branch();
  if (!branch) {
    toast("Add a taxpayer first, then open a form for them.");
    return;
  }
  openReturn({ tin: branch.tin, code, year: app.year() ?? new Date().getFullYear(), period: 0, mode: "sample" });
}

export function Library() {
  return (
    <View testID="library" class="w-full">
      <PageHeader title="Form library" lede={lede()} />
      <View class="mt-9 mb-4 w-full flex-row flex-wrap items-center gap-x-3 gap-y-2">
        <View class="w-full wide:w-80">
          <TextField
            testID="library-search"
            accessibilityLabel="Find a form"
            placeholder="Find a form, like 1601C or VAT"
            icon="search"
            value={query()}
            onChange={(value) => query.set(value)}
          />
        </View>
        <If when={app.taxpayer()}>
          <Segmented
            testID="library-scope"
            label="Which forms"
            options={scopeOptions()}
            value={scopeNow()}
            onChange={(value) => scope.set(value === "all" ? "all" : "owned")}
          />
        </If>
        <Segmented
          testID="library-freq"
          label="Frequency"
          options={FREQ_OPTIONS}
          value={freq()}
          onChange={(value) => freq.set(asFrequency(value))}
        />
        <Text variant="meta" tone="fg2" class="tabular-nums" testID="library-count">{`${shown().length} of ${forms().length}`}</Text>
      </View>
      <If when={shown().length} else={<EmptyRow testID="library-empty" text={emptyText()} />}>
        <View
          testID="library-grid"
          class="grid w-full gap-3"
          style={{ gridTemplateColumns: "repeat(auto-fill, minmax(250px, 1fr))" }}
        >
          <Each of={shown()} key={(f) => f.c}>
            {(f) => <FormTile entry={f} onOpen={() => openSample(f().c)} />}
          </Each>
        </View>
      </If>
    </View>
  );
}
