// The taxpayer page (old app/shell/taxpayer.js renderTaxpayerPage): the header with Copy TIN and New return, and the
// Profile, Forms and Templates tabs. Each tab is its own view (profile, forms, templates); the page is made once per move,
// with its edits, and watches the branch and year so a switch drops what no longer applies (the old reset and leaveYear).
import type { JSX } from "nsh";
import { Clipboard, effect, untracked, View } from "nsh";
import { formatTin } from "../../lib/format";
import { app, go, toast, type ViewParams } from "../../shell";
import { AppButton, Avatar, Code, MetaLine, PageHeader, Tabs } from "../../ui";
import { FormsTab } from "./FormsTab";
import type { EditableSection } from "./logic";
import { createPageState, type PageState } from "./page";
import { ProfileTab } from "./ProfileTab";
import { TemplatesTab } from "./TemplatesTab";
import type { FactName } from "../../lib/facts";

/** The view this page shows. */
export type TaxpayerTab = "profile" | "forms" | "templates";

const FACTS: readonly string[] = ["tax_classification", "income_tax_election", "eopt_tier", "is_government_withholding_entity", "atc_codes"];
const EDITABLE: readonly string[] = ["taxpayer", "branch", "address"];

/** The header: avatar, name, "TIN · branch · type", Copy TIN, New return; then the three tabs with their counts. */
function Header(props: { view: TaxpayerTab }) {
  const tin = () => formatTin(app.taxpayer()?.tin_root, app.branch()?.code);
  return (
    <View testID="tp-header" class="w-full">
      <PageHeader
        title={app.taxpayer()?.name ?? ""}
        avatar={<Avatar name={app.taxpayer()?.name ?? ""} size={40} hueKey={app.taxpayer()?.tin_root} />}
        meta={
          <MetaLine
            parts={[
              <Code tone="fg2" class="font-normal">{tin()}</Code>,
              app.branch()?.name,
              app.taxpayer()?.taxpayer_type,
            ]}
          />
        }
        actions={
          <>
            <AppButton
              testID="tp-copy-tin"
              label="Copy TIN"
              size="sm"
              icon="copy"
              onPress={() => {
                const text = tin();
                Clipboard.writeText(text).then(
                  () => toast(`Copied ${text}`, "copy"),
                  () => toast("The TIN could not be copied.", "triangle-alert"),
                );
              }}
            />
            <AppButton testID="tp-new-return" label="New return" variant="primary" size="sm" icon="plus" onPress={() => app.startNewReturn()} />
          </>
        }
      />
      <Tabs
        testID="tp-tabs"
        tabs={[
          { value: "profile", label: "Profile" },
          { value: "forms", label: "Forms", count: app.yearForms().length },
          { value: "templates", label: "Templates", count: app.templates().length },
        ]}
        value={props.view}
        onChange={(v) => go(v as TaxpayerTab)}
      />
    </View>
  );
}

/**
 * The page for one of its three views. Params: `edit` (a section to edit, or a fact row: "tax_classification" with
 * `fromForm` goes back to the return after a save), `open` (a section to open), `year` (select that tax year first),
 * `editForms` (the Forms tab opens with the list in edit), `addYear` (the Add year band open).
 */
export function TaxpayerPage(props: { view: TaxpayerTab; params?: ViewParams }): JSX.Element {
  const s: PageState = createPageState();
  const params = props.params ?? {};
  untracked(() => {
    if (typeof params.year === "number" && app.years().includes(params.year)) app.selectYear(params.year);
    const edit = typeof params.edit === "string" ? params.edit : null;
    if (edit === "business_start_date") s.startEdit("branch", { focusField: edit, fromForm: params.fromForm === true });
    else if (edit && EDITABLE.includes(edit)) s.startEdit(edit as EditableSection, { fromForm: params.fromForm === true });
    else if (edit && FACTS.includes(edit)) s.startFactEdit(edit as FactName, { fromForm: params.fromForm === true });
    if (params.editForms === true) s.editForms();
    if (params.addYear === true) s.addYear.set(true);
  });
  // A taxpayer or branch switch drops every edit; a year switch the year's (old reset and leaveYear).
  let lastTin = untracked(() => app.branch()?.tin);
  let lastYear = untracked(() => app.year());
  effect(() => {
    const tin = app.branch()?.tin;
    const year = app.year();
    untracked(() => {
      if (tin !== lastTin) s.reset();
      else if (year !== lastYear) s.leaveYear();
      lastTin = tin;
      lastYear = year;
    });
  });
  return (
    <View testID={`tp-page-${props.view}`} class="w-full">
      <If when={app.taxpayer() && app.branch()}>
        <Header view={props.view} />
        {props.view === "forms" ? <FormsTab s={s} /> : props.view === "templates" ? <TemplatesTab s={s} /> : <ProfileTab s={s} />}
      </If>
    </View>
  );
}
