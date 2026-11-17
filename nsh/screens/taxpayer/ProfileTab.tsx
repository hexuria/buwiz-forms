// The Profile tab (old profileTab): the tax-year segments over one group of seven sections laid out like the Certificate
// of Registration (Taxpayer, Registered activity, Tax types, Facts your forms use, Address and contact, Confirmation
// inbox, Agent filing), each editable one with an inline Edit; then Archive or delete for the admin. Narrow: the sections
// stack full width and an edit opens in a sheet with Save pinned at its foot.
import type { JSX } from "nsh";
import { View } from "nsh";
import type { BranchFieldName } from "../../bir";
import { shortDate } from "../../lib/dates";
import { joinNames, readerChips, type FactName } from "../../lib/facts";
import { formatTin } from "../../lib/format";
import { app, go, store } from "../../shell";
import {
  AppButton, Card, Chip, Code, DigitCells, Disclosure, DisclosureGroup, Dropdown, Field, FieldCell, FieldGrid, FormActions, Line,
  SectionTitle, Segmented, SettingRow, Sheet, Switch, Text, TextField,
} from "../../ui";
import { SearchPick } from "../form/controls";
import { canRemove, deleteTaxpayer, setGrant, setSectionOpen, shared, toggleArchive } from "./actions";
import { ReplaceBanner } from "./Banners";
import { FactsBody, factsSummary } from "./Facts";
import { InboxBody } from "./Inbox";
import { ADDRESS_FIELDS, addressMeta, agentMeta, inboxMeta, plural, TAXPAYER_TYPES, type EditableSection, type SectionId } from "./logic";
import type { PageState } from "./page";

const TITLES: Record<EditableSection, string> = { taxpayer: "Taxpayer", branch: "Registered activity", address: "Address and contact" };

/** An RDO or ZIP pick over the reference lists, the empty choice first. */
export function pickOptions(field: "rdo_code" | "zip_code") {
  const list = field === "rdo_code" ? store.rdoOptions() : store.zipOptions();
  return [{ value: "", label: "Choose…" }, ...list];
}

/** The year segments over the page ("Tax year"). */
export function YearSegments(props: { testID: string }) {
  return (
    <Segmented
      testID={props.testID}
      label="Tax year"
      wrap
      options={app.years().map((y) => ({ value: String(y), label: String(y) }))}
      value={String(app.year() ?? "")}
      onChange={(v) => app.selectYear(Number(v))}
    />
  );
}

/** A value in read mode, with a "Needed by" chip when a form on the list reads it and it is not set. */
function ReadCell(props: { label: string; value: string | null | undefined; full?: boolean; inherited?: boolean; flag?: string | null; testID?: string }) {
  return (
    <FieldCell full={props.full}>
      <View testID={props.testID}>
        <Field label={props.label} value={props.value} inherited={props.inherited} />
        <If when={props.flag}>
          <View class="mt-1 flex-row">
            <Chip label={props.flag as string} tone="warning" icon="triangle-alert" />
          </View>
        </If>
      </View>
    </FieldCell>
  );
}

/** The fact a date or line of business row flags: "Needed by 1701Q" while not set. */
function useFlag(s: PageState) {
  return (fact: FactName): string | null => {
    const view = s.yearFacts();
    return view && (view.not_set || []).includes(fact) ? `Needed by ${joinNames(readerChips(fact, view.used_by[fact]))}` : null;
  };
}

const headOffice = () => app.taxpayer()?.branches.find((x) => x.is_head_office) ?? null;
const inherits = (f: BranchFieldName) => {
  const b = app.branch();
  return Boolean(b && !b.is_head_office && (b.inherited || []).includes(f));
};
const inheritable = () => Boolean(app.branch() && !app.branch()?.is_head_office && headOffice());
const startLabel = () => (app.taxpayer()?.taxpayer_type === "Individual" ? "Business start date" : "Date of incorporation");

// ---------------------------------------------------------------- read mode

function TaxpayerRead(props: { s: PageState }) {
  const flag = useFlag(props.s);
  const t = () => app.taxpayer();
  return (
    <FieldGrid>
      <ReadCell full label="Registered name" value={t()?.name} testID="tp-taxpayer-name-value" />
      <ReadCell label="TIN" value={formatTin(t()?.tin_root)} />
      <ReadCell label="Taxpayer type" value={t()?.taxpayer_type} />
      <If when={t()?.taxpayer_type === "Individual"}>
        <ReadCell label="Birth date" value={t()?.birth_date ? shortDate(t()?.birth_date) : null} flag={flag("birth_date")} />
      </If>
    </FieldGrid>
  );
}

function ActivityRead(props: { s: PageState }) {
  const flag = useFlag(props.s);
  const b = () => app.branch();
  const view = () => props.s.yearFacts();
  return (
    <View class="w-full">
      <FieldGrid>
        <ReadCell label="Branch code" value={b()?.code} />
        <ReadCell label="Branch name" value={b()?.name} />
        <ReadCell label="RDO" value={b()?.facts.rdo_code} inherited={inherits("rdo_code")} />
        <If when={view()}>
          <ReadCell full label="Line of business" value={view()?.facts.line_of_business} flag={flag("line_of_business")} />
          <ReadCell
            label={startLabel()}
            value={view()?.facts.business_start_date ? shortDate(view()?.facts.business_start_date) : null}
            flag={flag("business_start_date")}
          />
        </If>
      </FieldGrid>
      <If when={!view()}>
        <Text variant="meta" tone="fg3" class="mt-3">Add a tax year to record the line of business and the start date.</Text>
      </If>
    </View>
  );
}

function AddressRead() {
  const b = () => app.branch();
  return (
    <FieldGrid>
      <Each of={ADDRESS_FIELDS} key={(f) => f[0]}>
        {(f) => <ReadCell full={f()[2]} label={f()[1]} value={b()?.facts[f()[0]]} inherited={inherits(f()[0])} />}
      </Each>
    </FieldGrid>
  );
}

// ---------------------------------------------------------------- edit mode

/** A control in a section being edited: the label, the "Same as head office" switch, the error once shown. */
function EditCell(props: { s: PageState; field: string; label: string; full?: boolean; inherit?: boolean; children?: JSX.Element }) {
  const f = props.field as BranchFieldName;
  return (
    <FieldCell full={props.full}>
      <Field
        label={props.label}
        error={props.s.errors()[props.field]}
        inheritSwitch={
          props.inherit && inheritable() ? (
            <Switch
              testID={`tp-inherit-${props.field}`}
              checked={Boolean(props.s.edit()?.inherit[f])}
              onChange={(on) => props.s.setInherit(f, on)}
              label={`${props.label}: same as head office`}
            />
          ) : undefined
        }
      >
        {props.children}
      </Field>
    </FieldCell>
  );
}

/** A text box of the section being edited. */
function EditText(props: { s: PageState; field: string; label: string; full?: boolean; inherit?: boolean; placeholder?: string; tabular?: boolean }) {
  const e = () => props.s.edit();
  const off = () => Boolean(props.inherit && inheritable() && e()?.inherit[props.field as BranchFieldName]);
  return (
    <EditCell s={props.s} field={props.field} label={props.label} full={props.full} inherit={props.inherit}>
      <TextField
        testID={`tp-${e()?.sec}-${props.field}`}
        accessibilityLabel={props.label}
        value={e()?.values[props.field] ?? ""}
        onChange={(v) => props.s.setValue(props.field, v)}
        onBlur={() => props.s.touch(props.field)}
        placeholder={props.placeholder}
        tabular={props.tabular}
        disabled={off()}
        error={Boolean(props.s.errors()[props.field])}
      />
    </EditCell>
  );
}

/** An RDO or ZIP pick of the section being edited. */
function EditPick(props: { s: PageState; field: "rdo_code" | "zip_code"; label: string }) {
  const e = () => props.s.edit();
  return (
    <EditCell s={props.s} field={props.field} label={props.label} inherit>
      <SearchPick
        testID={`tp-${e()?.sec}-${props.field}`}
        label={props.label}
        options={pickOptions(props.field)}
        value={e()?.values[props.field] ?? ""}
        onPick={(v) => {
          props.s.setValue(props.field, v);
          props.s.touch(props.field);
        }}
        disabled={Boolean(inheritable() && e()?.inherit[props.field])}
        error={Boolean(props.s.errors()[props.field])}
      />
    </EditCell>
  );
}

/** The fields of the section being edited (inline when wide, in the sheet when narrow). */
function SectionEditor(props: { s: PageState }) {
  const s = props.s;
  const e = () => s.edit();
  const sec = () => e()?.sec;
  return (
    <View class="w-full">
      <If when={sec() === "taxpayer"}>
        <FieldGrid>
          <EditText s={s} field="name" label="Registered name" full />
          <EditCell s={s} field="tin" label="TIN">
            <DigitCells
              testID="tp-taxpayer-tin"
              label="TIN"
              length={9}
              groups={3}
              value={e()?.values.tin ?? ""}
              onChange={(d) => s.setValue("tin", d)}
              onBlur={() => s.touch("tin")}
              error={Boolean(s.errors().tin)}
            />
          </EditCell>
          <EditCell s={s} field="type" label="Taxpayer type">
            <View class="w-full flex-row">
              <Dropdown
                testID="tp-type"
                label="Taxpayer type"
                width={260}
                options={TAXPAYER_TYPES.map((x) => ({ value: x, label: x }))}
                value={e()?.type ?? null}
                onChange={(v) => s.setType(v as (typeof TAXPAYER_TYPES)[number])}
              />
            </View>
          </EditCell>
          <If when={e()?.values.birth_date !== undefined}>
            <EditText s={s} field="birth_date" label="Birth date" placeholder="YYYY-MM-DD" tabular />
          </If>
        </FieldGrid>
        <Text variant="meta" tone="fg3" class="mt-4">Changing the TIN moves every branch, tax year, return and template to the new number.</Text>
      </If>
      <If when={sec() === "branch"}>
        <FieldGrid>
          <If when={e()?.values.code !== undefined} else={<ReadCell label="Branch code" value={app.branch()?.code} />}>
            <EditCell s={s} field="code" label="Branch code">
              <DigitCells
                testID="tp-branch-code"
                label="Branch code"
                length={5}
                value={e()?.values.code ?? ""}
                onChange={(d) => s.setValue("code", d)}
                onBlur={() => s.touch("code")}
                error={Boolean(s.errors().code)}
              />
            </EditCell>
          </If>
          <EditText s={s} field="name" label="Branch name" />
          <EditPick s={s} field="rdo_code" label="RDO" />
          <If when={e()?.yearFields}>
            <EditText s={s} field="line_of_business" label="Line of business" full placeholder="As on the Certificate of Registration" />
            <EditText s={s} field="business_start_date" label={startLabel()} placeholder="YYYY-MM-DD" tabular />
          </If>
        </FieldGrid>
      </If>
      <If when={sec() === "address"}>
        <FieldGrid>
          <EditText s={s} field="registered_address" label="Registered address" full inherit />
          <EditPick s={s} field="zip_code" label="ZIP code" />
          <EditText s={s} field="phone" label="Phone" inherit />
          <EditText s={s} field="email" label="Email" full inherit placeholder="name@example.com" />
        </FieldGrid>
      </If>
    </View>
  );
}

/** Cancel and Save of the section being edited. */
function EditActions(props: { s: PageState }) {
  return <FormActions testID={`tp-${props.s.edit()?.sec ?? "edit"}`} onCancel={props.s.cancelEdit} onSave={props.s.saveSection} />;
}

/** One editable section's body: the editor inline while it is edited on a wide window, else the values. */
function EditableBody(props: { s: PageState; sec: EditableSection; read: JSX.Element }) {
  return (
    <If when={props.s.edit()?.sec === props.sec && !app.layout.narrow()} else={props.read}>
      <SectionEditor s={props.s} />
      <EditActions s={props.s} />
    </If>
  );
}

// ---------------------------------------------------------------- tax types and agent filing

/** Tax types · your forms for the year: the forms with their frequency, Change, the tax types as chips, Save as a setup. */
function TaxTypesBody(props: { s: PageState }) {
  const s = props.s;
  const b = () => app.branch();
  const forms = () => app.yearForms();
  const entry = (code: string) => store.screenIndex().find((f) => f.c === code) ?? { c: code, t: code, freq: "" };
  const chips = () => store.taxTypes(b()?.tin, app.year());
  return (
    <If
      when={app.year()}
      else={
        <View class="gap-3">
          <Text variant="meta" tone="fg2">{`${b()?.name ?? ""} has no tax year yet. Add one to choose the forms it files.`}</Text>
          <View class="flex-row">
            <AppButton testID="tp-taxtypes-add-year" label="Add year" size="sm" icon="plus" onPress={() => go("forms", { addYear: true })} />
          </View>
        </View>
      }
    >
      <View class="gap-3">
        <View testID="tp-taxtypes-forms" class="gap-1">
          <If when={forms().length === 0}>
            <Text tone="fg3">{`No forms for ${app.year()} yet.`}</Text>
          </If>
          <Each of={forms()} key={(c) => c}>
            {(code) => (
              <View testID={`tp-taxtypes-form-${code()}`} class="min-h-6 flex-row items-center gap-3">
                <View class="shrink-0" style={{ width: 68 }}>
                  <Code>{code()}</Code>
                </View>
                <Line class="min-w-0 flex-1">{entry(code()).t}</Line>
                <If when={entry(code()).freq}>
                  <Chip label={entry(code()).freq} />
                </If>
              </View>
            )}
          </Each>
        </View>
        <If when={chips().length > 0}>
          <View testID="tp-taxtypes-chips" class="flex-row flex-wrap gap-1.5">
            <Each of={chips()} key={(c) => c.type}>
              {(c) => <Chip label={`${c().label} · from ${c().forms.join(", ")}`} tone="accent" />}
            </Each>
          </View>
        </If>
        <View class="flex-row flex-wrap items-center gap-2">
          <AppButton
            testID="tp-taxtypes-change"
            label={forms().length ? "Change" : "Choose forms"}
            size="sm"
            icon="pencil"
            onPress={() => go("forms", { editForms: true })}
          />
          <If when={forms().length > 0 && !s.saveSetup()}>
            <AppButton testID="tp-save-setup-open" label="Save as a setup" variant="ghost" size="sm" icon="layout-template" onPress={s.openSaveSetup} />
          </If>
        </View>
        <If when={s.saveSetup()}>
          <View class="w-full" style={{ maxWidth: 360 }}>
            <Field label="Setup name" error={s.saveSetup()?.error ?? undefined}>
              <TextField
                testID="tp-save-setup-name"
                accessibilityLabel="Setup name"
                placeholder="Like: Bookkeeping clients"
                value={s.saveSetup()?.name ?? ""}
                onChange={(v) => s.saveSetup.set({ name: v, error: null })}
                error={Boolean(s.saveSetup()?.error)}
              />
            </Field>
            <Text variant="meta" tone="fg3" class="mt-2">
              {`Keeps the ${app.year()} forms and the facts they use. Never a name, TIN, address, date or line of business.`}
            </Text>
            <FormActions testID="tp-save-setup" saveLabel="Save setup" onCancel={() => s.saveSetup.set(null)} onSave={s.saveSetupNow} />
          </View>
        </If>
      </View>
    </If>
  );
}

/** Agent filing: a standing OK per form for an assistant to file it for this branch. */
function AgentBody() {
  const b = () => app.branch();
  const forms = () => {
    const list = app.yearForms();
    return list.length ? list : ["2551Q"];
  };
  const granted = () => new Set(store.sendGrants(b()?.tin).map((g) => g.form));
  return (
    <View class="gap-3">
      <Text variant="meta" tone="fg2">
        A standing OK lets an assistant file that form for this branch without asking each time. The Queue button does not need it. You can turn it off here.
      </Text>
      <Each of={forms()} key={(c) => c}>
        {(code) => (
          <View class="flex-row items-center gap-3">
            <Switch
              testID={`tp-agent-${code()}`}
              checked={granted().has(code())}
              onChange={(on) => {
                const tin = b()?.tin;
                if (tin) setGrant(tin, code(), on);
              }}
              label={`Allow the assistant to file ${code()}`}
            />
            <Text variant="control">{code()}</Text>
          </View>
        )}
      </Each>
    </View>
  );
}

// ---------------------------------------------------------------- archive or delete

function RemoveCard(props: { s: PageState }) {
  const t = () => app.taxpayer();
  const armed = () => props.s.deleteArmed();
  return (
    <View class="w-full">
      <SectionTitle>Archive or delete</SectionTitle>
      <Card testID="tp-remove">
        <SettingRow
          testID="tp-archive"
          title={t()?.archived ? "Archived" : "Archive this taxpayer"}
          hint={t()?.archived ? "Kept with its returns, out of new work. Unarchive to file for it again" : "Keeps every return; it just leaves new work"}
        >
          <AppButton testID="tp-archive-btn" label={t()?.archived ? "Unarchive" : "Archive"} size="sm" icon="archive" onPress={toggleArchive} />
        </SettingRow>
        <SettingRow
          testID="tp-delete"
          title="Delete this taxpayer"
          hint={armed() ? "Every branch and return goes. This cannot be undone" : "Removes it and every branch and return"}
        >
          <View class="flex-row gap-2">
            <If when={armed()}>
              <AppButton testID="tp-delete-cancel" label="Keep it" variant="ghost" size="sm" onPress={() => props.s.deleteArmed.set(false)} />
            </If>
            <AppButton
              testID="tp-delete-btn"
              label={armed() ? "Delete for good" : "Delete"}
              variant="danger"
              size="sm"
              icon="trash"
              onPress={() => {
                if (!armed()) return void props.s.deleteArmed.set(true);
                if (deleteTaxpayer()) props.s.deleteArmed.set(false);
              }}
            />
          </View>
        </SettingRow>
      </Card>
    </View>
  );
}

// ---------------------------------------------------------------- the tab

export function ProfileTab(props: { s: PageState }) {
  const s = props.s;
  const b = () => app.branch();
  const year = () => app.year();
  const forms = () => app.yearForms();
  const toggle = (id: SectionId) => {
    const on = !shared.open(id);
    setSectionOpen(id, on);
    if (!on && s.edit()?.sec === id) s.cancelEdit();
    if (!on && id === "facts") s.cancelFactEdit();
    if (!on && id === "inbox") s.inbox.set(null);
  };
  const inheritedCount = () => ADDRESS_FIELDS.filter(([f]) => inherits(f)).length;
  return (
    <View testID="tp-profile-tab" class="w-full">
      <If when={app.years().length > 0}>
        <View class="mb-4 w-full flex-row">
          <YearSegments testID="tp-profile-years" />
        </View>
      </If>
      <ReplaceBanner />
      <DisclosureGroup testID="tp-profile">
        <Disclosure
          testID="tp-sec-taxpayer"
          title="Taxpayer"
          meta="Shared by every branch"
          open={shared.open("taxpayer")}
          onToggle={() => toggle("taxpayer")}
          onEdit={() => s.startEdit("taxpayer")}
          editing={s.edit()?.sec === "taxpayer"}
        >
          <EditableBody s={s} sec="taxpayer" read={<TaxpayerRead s={s} />} />
        </Disclosure>
        <Disclosure
          testID="tp-sec-branch"
          title="Registered activity"
          meta={`${b()?.is_head_office ? "Head office" : `Branch ${b()?.code}`}${year() ? ` · ${year()}` : ""}`}
          open={shared.open("branch")}
          onToggle={() => toggle("branch")}
          onEdit={() => s.startEdit("branch")}
          editing={s.edit()?.sec === "branch"}
        >
          <EditableBody s={s} sec="branch" read={<ActivityRead s={s} />} />
        </Disclosure>
        <Disclosure
          testID="tp-sec-taxtypes"
          title={year() ? `Tax types · your forms for ${year()}` : "Tax types · your forms"}
          meta={year() ? plural(forms().length, "form") : "Add a tax year first"}
          open={shared.open("taxtypes")}
          onToggle={() => toggle("taxtypes")}
        >
          <TaxTypesBody s={s} />
        </Disclosure>
        <Disclosure testID="tp-sec-facts" title="Facts your forms use" meta={factsSummary(s)} open={shared.open("facts")} onToggle={() => toggle("facts")}>
          <FactsBody s={s} />
        </Disclosure>
        <Disclosure
          testID="tp-sec-address"
          title="Address and contact"
          meta={addressMeta(Boolean(b()?.is_head_office), inheritedCount())}
          open={shared.open("address")}
          onToggle={() => toggle("address")}
          onEdit={() => s.startEdit("address")}
          editing={s.edit()?.sec === "address"}
        >
          <EditableBody s={s} sec="address" read={<AddressRead />} />
        </Disclosure>
        <Disclosure testID="tp-sec-inbox" title="Confirmation inbox" meta={inboxMeta(store.inboxState(b()?.tin))} open={shared.open("inbox")} onToggle={() => toggle("inbox")}>
          <InboxBody s={s} />
        </Disclosure>
        <Disclosure testID="tp-sec-agent" title="Agent filing" meta={agentMeta(store.sendGrants(b()?.tin))} open={shared.open("agent")} onToggle={() => toggle("agent")}>
          <AgentBody />
        </Disclosure>
      </DisclosureGroup>
      <If when={canRemove()}>
        <RemoveCard s={s} />
      </If>
      <If when={app.layout.narrow()}>
        <Sheet
          testID="tp-edit-sheet"
          open={s.edit() !== null}
          onOpenChange={(o) => {
            if (!o) s.cancelEdit();
          }}
          title={TITLES[s.edit()?.sec ?? "taxpayer"]}
          footer={<EditActions s={s} />}
        >
          <If when={s.edit()}>
            <View testID="tp-edit-sheet-body" class="w-full">
              <SectionEditor s={s} />
            </View>
          </If>
        </Sheet>
      </If>
    </View>
  );
}
