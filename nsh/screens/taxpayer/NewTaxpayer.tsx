// New taxpayer (old renderNewTaxpayer): the 9-digit TIN, the type, the registered name, "Start from" (nothing, a setup
// for the type, or a copy of a taxpayer I have), the head office's address and contact. A field shows its error once
// it was left (the TIN once all 9 digits are in), every field after the first Create, and what the host answered.
import type { JSX } from "nsh";
import { batch, signal, untracked, View } from "nsh";
import type { Taxpayer } from "../../bir";
import { errorText } from "../../lib/errors";
import { app, back, go, replace, store, toast } from "../../shell";
import { DigitCells, Dropdown, Field, FieldCell, FieldGrid, FormActions, PageHeader, Text, TextField } from "../../ui";
import { SearchPick } from "../form/controls";
import { startFromChoice } from "./actions";
import { emptyNewTaxpayer, newTaxpayerErrors, newTaxpayerProblems, startHint, startYear, TAXPAYER_TYPES, type NewTaxpayerDraft } from "./logic";
import { knownRdo } from "./page";
import { pickOptions } from "./ProfileTab";

type TextKey = "name" | "registered_address" | "phone" | "email";

/** The card the New taxpayer and New branch forms sit in. */
export function FormCard(props: { testID: string; error?: string | null; children?: JSX.Element; actions?: JSX.Element }) {
  return (
    <View testID={props.testID} class="mt-7 w-full rounded-(--radius-card) border border-border p-4 wide:p-5">
      {props.children}
      <If when={props.error}>
        <Text variant="meta" tone="danger" class="mt-4" testID={`${props.testID}-error`}>{props.error as string}</Text>
      </If>
      {props.actions}
    </View>
  );
}

export function NewTaxpayerPage() {
  const n = signal<NewTaxpayerDraft>(emptyNewTaxpayer());
  untracked(() => app.closePanel());
  // A type change resets "Start from": a setup or a taxpayer of the old type does not fit the new one.
  const set = (patch: Partial<NewTaxpayerDraft>, key?: string) => {
    const cur = n();
    const hostErrors = { ...cur.hostErrors };
    if (key) delete hostErrors[key];
    n.set({ ...cur, ...patch, hostErrors, error: null });
  };
  const touch = (key: string) => {
    const cur = n();
    if (!cur.touched.includes(key)) n.set({ ...cur, touched: [...cur.touched, key] });
  };
  const problems = () => newTaxpayerProblems(n(), store.taxpayers(), knownRdo);
  const errors = () => newTaxpayerErrors(n(), problems());
  const setups = () => store.setups(n().type);
  const branches = () => {
    const out: { value: string; label: string }[] = [];
    for (const t of app.taxpayers()) {
      if (t.taxpayer_type !== n().type) continue;
      for (const b of t.branches) out.push({ value: b.tin, label: t.branches.length > 1 ? `${t.name} · ${b.name}` : t.name });
    }
    return out;
  };
  const startOptions = () => {
    const options = [
      { value: "none", label: "Nothing yet: I choose the forms" },
      ...setups().map((x) => ({ value: `setup:${x.id}`, label: x.built_in ? x.name : `${x.name} (yours)` })),
    ];
    if (branches().length) options.push({ value: "copy", label: "Same as a taxpayer I have…" });
    return options;
  };
  const start = () => (startOptions().some((o) => o.value === n().start) ? n().start : "none");
  const year = () => startYear(start(), n().from ? store.taxYears(n().from) : null);
  const chosen = () => (start().startsWith("setup:") ? setups().find((x) => `setup:${x.id}` === start()) ?? null : null);

  function create(): void {
    const cur = { ...n(), start: start(), submitted: true, error: null };
    n.set(cur);
    if (Object.keys(newTaxpayerProblems(cur, store.taxpayers(), knownRdo)).length) return;
    const name = cur.name.trim();
    try {
      store.write("create_taxpayer", {
        tin_root: cur.tin,
        name,
        taxpayer_type: cur.type,
        head_office: {
          registered_address: cur.registered_address.trim(),
          zip_code: cur.zip_code.trim(),
          rdo_code: cur.rdo_code.trim(),
          phone: cur.phone.trim(),
          email: cur.email.trim(),
        },
      });
    } catch (e) {
      const text = errorText(e);
      if (/\bTIN\b/i.test(text)) n.set({ ...cur, hostErrors: { ...cur.hostErrors, tin: text } });
      else n.set({ ...cur, error: text });
      return;
    }
    app.selectTaxpayer(cur.tin, { quiet: true });
    const headTin = `${cur.tin}00000`;
    if (cur.start !== "none") {
      const y = startYear(cur.start, cur.from ? store.taxYears(cur.from) : null);
      try {
        if (cur.start === "copy") {
          const source = app.taxpayers().find((t) => t.tin_root === String(cur.from).slice(0, 9));
          startFromChoice("copy", headTin, y, { from: cur.from as string, label: `Same as ${source ? source.name : "the taxpayer"}` });
        } else {
          const id = cur.start.replace(/^setup:/, "");
          const setup = store.setups(cur.type).find((x) => x.id === id);
          startFromChoice("setup", headTin, y, { id, label: setup ? setup.name : "The setup" });
        }
        app.selectYear(y);
        replace("forms");
        return;
      } catch (e) {
        toast(`${name} added, but it did not start from your choice: ${errorText(e)}`, "triangle-alert");
      }
    }
    // In place of this form, not after it: the taxpayer exists now, and Back must not return to the form that made it.
    replace("forms", { addYear: true });
    toast(`${name} added. Add a tax year to choose its forms.`);
  }

  const text = (key: TextKey, label: string, full?: boolean, placeholder?: string) => (
    <FieldCell full={full}>
      <Field label={label} error={errors()[key]}>
        <TextField
          testID={`new-tp-${key}`}
          accessibilityLabel={label}
          placeholder={placeholder}
          value={n()[key]}
          onChange={(v) => set({ [key]: v } as Partial<NewTaxpayerDraft>, key)}
          onBlur={() => touch(key)}
          error={Boolean(errors()[key])}
        />
      </Field>
    </FieldCell>
  );
  const pick = (key: "zip_code" | "rdo_code", label: string) => (
    <FieldCell>
      <Field label={label} error={errors()[key]}>
        <SearchPick
          testID={`new-tp-${key}`}
          label={label}
          options={pickOptions(key)}
          value={n()[key]}
          onPick={(v) => {
            batch(() => {
              set({ [key]: v } as Partial<NewTaxpayerDraft>, key);
              touch(key);
            });
          }}
          error={Boolean(errors()[key])}
        />
      </Field>
    </FieldCell>
  );
  return (
    <View testID="new-tp" class="w-full">
      <PageHeader
        title="New taxpayer"
        lede="The 9-digit TIN, the registered name and the head office's address. Branches inherit the address unless you change it."
      />
      <FormCard
        testID="new-tp-card"
        error={n().error}
        actions={
          <FormActions
            testID="new-tp"
            saveLabel="Create taxpayer"
            onCancel={() => {
              if (!back()) go("home");
            }}
            onSave={create}
          />
        }
      >
        <FieldGrid>
          <FieldCell>
            <Field label="TIN" error={errors().tin}>
              <DigitCells
                testID="new-tp-tin"
                label="TIN"
                length={9}
                groups={3}
                value={n().tin}
                onChange={(d) => set({ tin: d }, "tin")}
                onBlur={() => touch("tin")}
                error={Boolean(errors().tin)}
              />
            </Field>
          </FieldCell>
          <FieldCell>
            <Field label="Taxpayer type">
              <View class="w-full flex-row">
                <Dropdown
                  testID="new-tp-type"
                  label="Taxpayer type"
                  width={220}
                  options={TAXPAYER_TYPES.map((x) => ({ value: x, label: x }))}
                  value={n().type}
                  onChange={(v) => set({ type: v as Taxpayer["taxpayer_type"], start: "none", from: null })}
                />
              </View>
            </Field>
          </FieldCell>
          {text("name", "Registered name", true, "As registered with BIR")}
          <FieldCell full>
            <Field label="Start from">
              <View class="w-full flex-row">
                <Dropdown
                  testID="new-tp-start"
                  label="Start from"
                  width={300}
                  options={startOptions()}
                  value={start()}
                  onChange={(v) => set({ start: v, from: v === "copy" ? n().from : null })}
                />
              </View>
            </Field>
            <Text variant="meta" tone="fg3" class="mt-1" testID="new-tp-start-hint">{startHint(chosen(), start(), year())}</Text>
          </FieldCell>
          <If when={start() === "copy"}>
            <FieldCell full>
              <Field label="Copy from" error={errors().from}>
                <View class="w-full flex-row">
                  <Dropdown testID="new-tp-from" label="Copy from" width={300} options={branches()} value={n().from} onChange={(v) => set({ from: v }, "from")} error={Boolean(errors().from)} />
                </View>
              </Field>
            </FieldCell>
          </If>
          {text("registered_address", "Head-office address", true)}
          {pick("zip_code", "ZIP code")}
          {pick("rdo_code", "RDO")}
          {text("phone", "Phone")}
          {text("email", "Email", false, "name@example.com")}
        </FieldGrid>
      </FormCard>
    </View>
  );
}
