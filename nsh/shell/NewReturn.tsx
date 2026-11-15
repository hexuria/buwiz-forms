// The start-a-return dialog (the "+" and Cmd/Ctrl+N): the forms the branch files this year, a period, whether a template
// exists, and the facts the form reads that are not set (named, never blocking). Opens the form page.
import { View, Scroll } from "nsh";
import { frequencyOf, periodOptions, type Frequency } from "../lib/periods";
import { notSetForForm, notSetSentence } from "../lib/facts";
import { AppButton, Chip, Code, Dialog, Dropdown, Icon, NavRow, Text } from "../ui";
import { app } from "./app";
import { store } from "./store";

export function NewReturn() {
  const tp = () => app.taxpayer();
  const br = () => app.branch();
  const forms = () => app.yearForms();
  const entry = (code: string) => store.screenIndex().find((f) => f.c === code);
  /** The picked form, kept in the list. */
  const code = () => {
    const picked = app.newReturn().code;
    return picked && forms().includes(picked) ? picked : (forms()[0] ?? "");
  };
  const options = () => periodOptions(code(), (entry(code())?.freq ?? frequencyOf(store.screenIndex() as { c: string; freq: Frequency }[], code())) as Frequency, app.year() ?? new Date().getFullYear());
  /** The picked period, kept among the options. */
  const period = () => {
    const picked = app.newReturn().period;
    return options().some((o) => o.value === picked) ? (picked as number) : (options()[0]?.value ?? 0);
  };
  const hasTemplate = () => app.templates().some((t) => t.form_code === code());
  // A fact this form reads that is not set is named, never blocks.
  const notSet = () => {
    const b = br();
    const y = app.year();
    if (!b || !y) return null;
    return notSetSentence(notSetForForm(store.yearFacts(b.tin, y), code(), store.electionReadiness(b.tin, y)));
  };
  const open = () => {
    const b = br();
    const y = app.year();
    if (b && y && code()) app.openReturn({ tin: b.tin, code: code(), year: y, period: period() });
  };
  return (
    <Dialog testID="new-return" open={true} onOpenChange={(o) => !o && app.closeOverlay()} width={560}>
      <View class="gap-0.5 px-5 pt-4 pb-1">
        <Text variant="title">Start a return</Text>
        {/* The old header (chrome.js): "name ·", the TIN with its branch code, the branch's name; 4 apart, no dots after the first. */}
        <View testID="new-return-who" class="mt-0.5 flex-row gap-1">
          <If when={tp()?.name}>
            <Text variant="meta" tone="fg2">{`${tp()?.name ?? ""} ·`}</Text>
          </If>
          <Code tone="fg2" class="font-normal">{br()?.tin_display ?? ""}</Code>
          <Text variant="meta" tone="fg2">{br()?.name ?? ""}</Text>
        </View>
      </View>
      <Scroll testID="new-return-list" class="shrink" style={{ maxHeight: 360 }} contentClass="gap-0.5 p-3">
        <Each of={forms()} key={(c) => c}>
          {(c) => (
            <NavRow
              testID={`new-return-${c()}`}
              leading={<Code class="w-15">{c()}</Code>}
              label={entry(c())?.t ?? c()}
              size="list"
              count={entry(c())?.freq ?? ""}
              current={c() === code()}
              onPress={() => app.pickNewReturnForm(c())}
            />
          )}
        </Each>
      </Scroll>
      <If when={notSet()}>
        <View testID="new-return-not-set" class="gap-2 px-5 pb-3">
          <View class="flex-row items-center gap-2">
            <Icon name="triangle-alert" size={14} tone="warning" />
            <Text variant="meta" tone="fg2" class="min-w-0 flex-1">{notSet() ?? ""}</Text>
          </View>
          {/* On its own line under the warning, as the old dialog wraps it (not to the right of the sentence). */}
          <View class="self-start">
            <AppButton
              testID="new-return-profile"
              label="Open the profile"
              variant="ghost"
              size="sm"
              onPress={() => {
                app.closeOverlay();
                app.openProfile();
              }}
            />
          </View>
        </View>
      </If>
      <View class="flex-row flex-wrap items-center justify-between gap-2 border-t border-border px-5 pt-3 pb-4">
        <View class="flex-row items-center gap-2">
          <Dropdown
            testID="new-return-period"
            label="Period"
            width={160}
            options={options().map((o) => ({ value: String(o.value), label: o.label }))}
            value={String(period())}
            onChange={(v) => app.pickNewReturnPeriod(Number(v))}
          />
          <If when={hasTemplate()} else={<Chip label="No template" />}>
            <Chip label="Template" tone="success" icon="layout-template" />
          </If>
        </View>
        <View class="flex-row gap-2">
          <AppButton testID="new-return-cancel" label="Cancel" variant="ghost" onPress={app.closeOverlay} />
          <AppButton testID="new-return-open" label="Open form" variant="primary" onPress={open} />
        </View>
      </View>
    </Dialog>
  );
}
