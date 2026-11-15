// The Forms tab (old formsTab): the year segments, Add year (the band with the year and "copy N forms from"), the year's
// forms with their next period, due date and template; Edit forms ticks forms from every form there is, with a search,
// the frequency filter and the setups (apply, delete one of yours). Two annual income tax returns of one kind: a warning.
import { View } from "nsh";
import type { ScreenFormIndexEntry } from "../../bir";
import { daysUntil, shortDate } from "../../lib/dates";
import { errorText } from "../../lib/errors";
import { app, store, toast, today } from "../../shell";
import { AppButton, Banner, Checkbox, CheckboxLabel, Chip, Code, Dropdown, EmptyRow, IconButton, Line, ListGroup, ListRow, NameCell, Segmented, Text, TextField } from "../../ui";
import { applySetup, deleteSetup } from "./actions";
import { AnnualConflictBanner, ReplaceBanner } from "./Banners";
import { allowedYears, defaultAddYear, formsSavedText, formSub, FREQS, plural, schedule, searchText, yearAddedText } from "./logic";
import type { PageState } from "./page";
import { YearSegments } from "./ProfileTab";

/** The Add year band: which year, whether to copy the latest year's forms, Add. */
export function AddYearBand(props: { s: PageState }) {
  const s = props.s;
  const ys = () => app.years();
  const bounds = () => store.yearBounds();
  const allowed = () => allowedYears(bounds(), ys());
  const pick = () => {
    const sel = s.addYearSel();
    return sel !== null && allowed().includes(sel) ? sel : defaultAddYear(allowed(), ys());
  };
  const last = () => ys()[ys().length - 1];
  const lastCount = () => (last() ? store.yearlyForms(app.branch()?.tin, last()).length : 0);
  const add = () => {
    const b = app.branch();
    const year = pick();
    if (!b || year === null) return;
    try {
      const r = store.write("add_tax_year", b.tin, year, ys().length > 0 && s.addYearCopy());
      app.selectYear(year);
      s.addYear.set(false);
      s.forms.set(null);
      toast(yearAddedText(year, r));
    } catch (e) {
      toast(errorText(e), "triangle-alert");
    }
  };
  return (
    <View class="mb-4 w-full">
      <Banner testID="tp-add-year">
        <If
          when={allowed().length > 0}
          else={
            <>
              <Text class="min-w-0 flex-1">{`Every tax year from ${bounds().min} to ${bounds().max} is already added.`}</Text>
              <If when={ys().length > 0}>
                <AppButton testID="tp-add-year-cancel" label="Cancel" variant="ghost" size="sm" onPress={s.toggleAddYear} />
              </If>
            </>
          }
        >
          <Text variant="control" class="font-semibold">Add tax year</Text>
          <Dropdown
            testID="tp-add-year-select"
            label="Tax year to add"
            width={96}
            options={allowed().map((y) => ({ value: String(y), label: String(y) }))}
            value={String(pick() ?? "")}
            onChange={(v) => s.addYearSel.set(Number(v))}
          />
          <If when={last()}>
            <CheckboxLabel
              testID="tp-add-year-copy"
              checked={s.addYearCopy()}
              onChange={(v) => s.addYearCopy.set(v)}
              label={`Copy ${plural(lastCount(), "form")} from ${last()}`}
            />
          </If>
          <View class="flex-1" />
          <If when={ys().length > 0}>
            <AppButton testID="tp-add-year-cancel" label="Cancel" variant="ghost" size="sm" onPress={s.toggleAddYear} />
          </If>
          <AppButton testID="tp-add-year-add" label={`Add ${pick() ?? ""}`} variant="primary" size="sm" onPress={add} />
        </If>
      </Banner>
    </View>
  );
}

/** One form's row: the tick (editing), code, name with its next period, frequency, due date, template. */
function FormRow(props: { s: PageState; f: ScreenFormIndexEntry }) {
  const f = props.f;
  const year = () => app.year() ?? 0;
  const editing = () => props.s.forms() !== null;
  const on = () => !editing() || Boolean(props.s.forms()?.includes(f.c));
  const compact = () => app.layout.mainWidth() < 600;
  const tpl = () => app.templates().some((x) => x.form_code === f.c);
  const sc = () => {
    const b = app.branch();
    if (!on() || !b) return { next: null, due: "" };
    const y = year();
    const now = today();
    return schedule(store.formDeadlines(b.tin, f.c, y, `${y}-01-01`, `${y + 1}-12-31`), (iso) => daysUntil(iso, now), shortDate);
  };
  return (
    <ListRow testID={`tp-form-${f.c}`}>
      <If when={editing()}>
        <Checkbox testID={`tp-form-${f.c}-check`} checked={on()} onChange={(v) => props.s.toggleForm(f.c, v)} label={`File ${f.c} in ${year()}`} />
      </If>
      <View class="shrink-0" style={{ width: 68 }}>
        <Code tone={on() ? "fg" : "fg3"}>{f.c}</Code>
      </View>
      <NameCell title={f.t} sub={formSub(on(), editing(), sc(), year())} dim={!on()} />
      <If when={!compact()}>
        <View class="shrink-0 flex-row" style={{ width: 84 }}>
          <If when={f.freq}>
            <Chip label={f.freq} />
          </If>
        </View>
        <View class="shrink-0" style={{ width: 116 }}>
          <Line variant="meta" tone="fg2" class="tabular-nums">{on() && sc().due ? `Due ${sc().due}` : ""}</Line>
        </View>
      </If>
      <View class="shrink-0 flex-row" style={{ width: compact() ? 96 : 112 }}>
        <If when={on()}>
          <If when={tpl()} else={<Chip label="No template" />}>
            <Chip label="Template" tone="success" icon="layout-template" />
          </If>
        </If>
      </View>
    </ListRow>
  );
}

export function FormsTab(props: { s: PageState }) {
  const s = props.s;
  const b = () => app.branch();
  const index = () => store.screenIndex();
  const editing = () => s.forms() !== null;
  const saved = () => {
    const codes = new Set(app.yearForms());
    return index().filter((f) => codes.has(f.c));
  };
  const filtered = () => {
    const q = s.query().trim().toLowerCase();
    return index().filter((f) => (s.freq() === "All" || f.freq === s.freq()) && (!q || searchText(f).includes(q)));
  };
  const setups = () => store.setups(app.taxpayer()?.taxpayer_type);
  const conflicts = () => store.annualConflicts(s.forms() ?? app.yearForms());
  const save = () => {
    const year = app.year();
    const after = s.forms();
    const tin = b()?.tin;
    if (!year || !after || !tin) return;
    const before = [...app.yearForms()];
    try {
      store.write("set_yearly_forms", tin, year, after);
      s.forms.set(null);
      toast(formsSavedText(year, before, after));
    } catch (e) {
      toast(errorText(e), "triangle-alert");
    }
  };
  return (
    <View testID="tp-forms-tab" class="w-full">
      <If
        when={app.years().length > 0}
        else={
          <View class="w-full">
            <Text tone="fg2" class="mb-4">{`${b()?.name ?? ""} has no tax year yet. Add one to choose the forms it files.`}</Text>
            <AddYearBand s={s} />
          </View>
        }
      >
        <View class="mb-4 w-full flex-row flex-wrap items-center gap-x-3 gap-y-2">
          <YearSegments testID="tp-years" />
          <AppButton testID="tp-add-year-open" label="Add year" variant="ghost" size="sm" icon="plus" onPress={s.toggleAddYear} />
          <View class="flex-1" />
          <If
            when={editing()}
            else={<AppButton testID="tp-forms-edit" label="Edit forms" size="sm" icon="pencil" onPress={s.editForms} />}
          >
            <Text variant="meta" tone="fg2" class="tabular-nums" testID="tp-forms-count">{`${s.forms()?.length ?? 0} selected`}</Text>
            <AppButton testID="tp-forms-cancel" label="Cancel" variant="ghost" size="sm" onPress={() => s.forms.set(null)} />
            <AppButton testID="tp-forms-save" label="Save forms" variant="primary" size="sm" onPress={save} />
          </If>
        </View>
        <If when={s.addYear()}>
          <AddYearBand s={s} />
        </If>
        <ReplaceBanner />
        <Each of={conflicts()} key={(g) => g.join("-")}>
          {(g) => <AnnualConflictBanner group={g()} year={app.year() ?? 0} />}
        </Each>
        <If
          when={editing()}
          else={
            <ListGroup testID="tp-forms-list">
              <If
                when={saved().length > 0}
                else={
                  <EmptyRow
                    testID="tp-forms-empty"
                    text={`No forms for ${app.year()} yet.`}
                    action={<AppButton testID="tp-forms-choose" label="Choose forms" size="sm" onPress={s.editForms} />}
                  />
                }
              >
                <Each of={saved()} key={(f) => f.c}>
                  {(f) => <FormRow s={s} f={f()} />}
                </Each>
              </If>
            </ListGroup>
          }
        >
          <View class="mb-4 w-full flex-row flex-wrap items-center gap-x-3 gap-y-2">
            <View style={{ width: Math.min(280, app.layout.mainWidth() - 32) }}>
              <TextField
                testID="tp-forms-search"
                icon="search"
                accessibilityLabel="Find a form"
                placeholder="Find a form, like 1601C or VAT"
                value={s.query()}
                onChange={(v) => s.query.set(v)}
              />
            </View>
            <Segmented
              testID="tp-forms-freq"
              label="Frequency"
              wrap
              options={FREQS.map((x) => ({ value: x, label: x }))}
              value={s.freq()}
              onChange={(v) => s.freq.set(v)}
            />
          </View>
          <If when={setups().length > 0}>
            <View testID="tp-setups" class="mb-4 w-full flex-row flex-wrap items-center gap-1.5">
              <Text variant="meta" tone="fg3" class="mr-0.5">Setups</Text>
              <Each of={setups()} key={(x) => x.id}>
                {(setup) => (
                  <View class="flex-row items-center">
                    <AppButton
                      testID={`tp-setup-${setup().id}`}
                      label={setup().name}
                      size="sm"
                      onPress={() => {
                        if (applySetup(setup())) s.forms.set(null);
                      }}
                    />
                    <If when={!setup().built_in}>
                      <IconButton
                        testID={`tp-setup-${setup().id}-delete`}
                        icon="trash"
                        tooltip={`Delete the setup ${setup().name}`}
                        onPress={() => deleteSetup(setup())}
                      />
                    </If>
                  </View>
                )}
              </Each>
            </View>
          </If>
          <ListGroup testID="tp-forms-list">
            <If when={filtered().length > 0} else={<EmptyRow testID="tp-forms-nomatch" text="No form matches. Try a code like 1601C or a word like VAT." />}>
              <Each of={filtered()} key={(f) => f.c}>
                {(f) => <FormRow s={s} f={f()} />}
              </Each>
            </If>
          </ListGroup>
        </If>
      </If>
    </View>
  );
}
