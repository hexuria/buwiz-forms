// Facts your forms use (old factsBody, factRow): one row per fact a form on the year's list reads, with who reads it,
// where the rate was chosen, why it cannot change; Set or Change opens the row's choices; a quiet line counts the rest.
import { View, type ViewRef } from "nsh";
import { ELECTION_CHOICES, EOPT_TIERS, electionSourceText, FACT_LABELS, factValue, incomeTypeChoices, readerChips, ROW_FACTS, type FactName } from "../../lib/facts";
import { app, store } from "../../shell";
import { AppButton, Chip, Field, FormActions, Segmented, Text, TextField } from "../../ui";
import { revealAndFocus } from "../../ui/focus";
import { factsMeta, hiddenFactsText } from "./logic";
import type { PageState } from "./page";

/** The facts the year's forms read. */
function shownFacts(s: PageState): FactName[] {
  const view = s.yearFacts();
  return view ? ROW_FACTS.filter((f) => (view.used_by[f] || []).length || s.factEdit()?.fact === f) : [];
}

/** Whether a row is flagged "Not set": the rate only when the taxpayer chooses one. */
function rowFlagged(s: PageState, fact: FactName): boolean {
  const view = s.yearFacts();
  if (!view || !(view.not_set || []).includes(fact)) return false;
  if (fact !== "income_tax_election") return true;
  return store.electionReadiness(view.tin, view.year)?.gap === "ready";
}

/** The section's summary: "2 not set", "All set", ... */
export function factsSummary(s: PageState): string {
  const shown = shownFacts(s);
  return factsMeta(Boolean(s.yearFacts()), shown.length, shown.filter((f) => rowFlagged(s, f)).length);
}

/** A fact's choices as [value, label], the value as text (true and false become "true" and "false"). */
function choices(fact: FactName): [string, string][] {
  if (fact === "tax_classification") return incomeTypeChoices(app.taxpayer()?.taxpayer_type);
  if (fact === "income_tax_election") return ELECTION_CHOICES.map(([v, l]) => [v, l]);
  if (fact === "eopt_tier") return EOPT_TIERS.map(([v, l]) => [v, l]);
  return [
    ["true", "Yes"],
    ["false", "No"],
  ];
}

function FactRow(props: { s: PageState; fact: FactName }) {
  const s = props.s;
  const fact = props.fact;
  let editorRef: ViewRef | undefined;
  let choiceRef: ViewRef | undefined;
  const view = () => s.yearFacts();
  const year = () => view()?.year ?? 0;
  const edit = () => (s.factEdit()?.fact === fact ? s.factEdit() : null);
  const readiness = () => (fact === "income_tax_election" && view() ? store.electionReadiness(view()?.tin, year()) : null);
  const chooses = () => fact !== "income_tax_election" || readiness()?.gap === "ready";
  const locked = () => fact === "income_tax_election" && Boolean(view()?.election_locked);
  const value = () => {
    const v = factValue(fact, view()?.facts, year());
    if (v || chooses()) return v;
    return readiness()?.gap === "not_individual" ? "Does not apply" : "Not chosen";
  };
  const selected = () => {
    const v = edit()?.value;
    if (fact === "income_tax_election" && /^Graduated/.test(String(v || ""))) return "GraduatedUnspecified";
    return v === null || v === undefined ? "" : String(v);
  };
  const source = () => (fact === "income_tax_election" && value() ? electionSourceText(view()?.election_source) : null);
  const why = () =>
    readiness()?.gap === "not_individual"
      ? "Only an individual chooses between 8% and graduated rates."
      : "Only a self-employed or mixed-income individual chooses between 8% and graduated rates. Set the income type first.";
  return (
    <View testID={`tp-fact-${fact}`} class="w-full gap-1 border-t border-border py-3 first:border-t-0">
      <View class="flex-row items-center gap-2">
        <View class="min-w-0 flex-1">
          <Field label={FACT_LABELS[fact]} value={value()} />
        </View>
        <If when={rowFlagged(s, fact) && !edit()}>
          <Chip label="Not set" tone="warning" icon="triangle-alert" />
        </If>
        <If when={!edit() && chooses() && !locked()}>
          <AppButton
            testID={`tp-fact-${fact}-edit`}
            label={value() ? "Change" : "Set"}
            variant="ghost"
            size="sm"
            icon="pencil"
            onPress={() => s.startFactEdit(fact)}
          />
        </If>
      </View>
      <View testID={`tp-fact-${fact}-forms`} class="flex-row flex-wrap items-center gap-1.5">
        <Text variant="cap" tone="fg3">Read by</Text>
        <Each of={[...readerChips(fact, view()?.used_by[fact])]} key={(c) => c}>
          {(c) => <Chip label={c()} />}
        </Each>
      </View>
      <If when={source()}>
        <Text variant="meta" tone="fg3" testID="tp-fact-election-source">{source() as string}</Text>
      </If>
      <If when={fact === "income_tax_election" && locked()}>
        <Text variant="meta" tone="fg3" testID="tp-fact-election-locked">{`A ${year()} return that carries it was filed, so it cannot change for ${year()}.`}</Text>
      </If>
      <If when={fact === "income_tax_election" && !locked() && !chooses()}>
        <Text variant="meta" tone="fg3" testID="tp-fact-election-why">{why()}</Text>
      </If>
      <If when={view()?.owner[fact as Exclude<FactName, "birth_date">] === "head_office" && !app.branch()?.is_head_office}>
        <Text variant="meta" tone="fg3" testID={`tp-fact-${fact}-owner`}>Held by the head office. A change here changes it for every branch.</Text>
      </If>
      <If when={edit()}>
        <View
          class="mt-2 w-full"
          ref={(r) => { editorRef = r; }}
          onLayout={() => {
            if (editorRef && choiceRef) {
              revealAndFocus(choiceRef, editorRef);
              editorRef = undefined;
            }
          }}
        >
          <If
            when={fact === "atc_codes"}
            else={
              <Segmented
                testID={`tp-fact-${fact}-choice`}
                label={FACT_LABELS[fact]}
                focusRef={(r) => { choiceRef = r; }}
                wrap
                options={choices(fact).map(([v, label]) => ({ value: v, label }))}
                value={selected()}
                onChange={(v) => s.pickFact(fact === "is_government_withholding_entity" ? v === "true" : v)}
              />
            }
          >
            <View class="w-full" style={{ maxWidth: 360 }}>
              <TextField
                testID="tp-fact-atc_codes-input"
                accessibilityLabel={FACT_LABELS[fact]}
                placeholder="WC160, WI010"
                value={edit()?.atc ?? ""}
                onChange={(v) => s.setAtc(v)}
                error={Boolean(edit()?.error)}
              />
            </View>
          </If>
          <If when={edit()?.error}>
            <Text variant="meta" tone="danger" class="mt-1" testID={`tp-fact-${fact}-error`}>{edit()?.error ?? ""}</Text>
          </If>
          <FormActions testID={`tp-fact-${fact}`} saveLabel={`Save for ${year()}`} onCancel={s.cancelFactEdit} onSave={s.saveFactEdit} />
        </View>
      </If>
    </View>
  );
}

/** The rows, or why there are none. */
export function FactsBody(props: { s: PageState }) {
  const s = props.s;
  const hidden = () => hiddenFactsText(ROW_FACTS.length - shownFacts(s).length);
  return (
    <If when={s.yearFacts()} else={<Text variant="meta" tone="fg3">Add a tax year to see the facts its forms use.</Text>}>
      <View class="w-full">
        <Each of={shownFacts(s)} key={(f) => f}>
          {(f) => <FactRow s={s} fact={f()} />}
        </Each>
        <If when={hidden()}>
          <Text variant="meta" tone="fg3" testID="tp-facts-hidden" class={shownFacts(s).length ? "mt-3" : ""}>{hidden() as string}</Text>
        </If>
      </View>
    </If>
  );
}
