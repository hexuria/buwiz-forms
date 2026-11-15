// New branch (old renderNewBranch): the 5-digit branch code (the next free one suggested), the name, and the head
// office's RDO, address and contact, each with "Same as head office" on until switched off to set the branch's own.
import { signal, untracked, View } from "nsh";
import type { BranchFieldName } from "../../bir";
import { errorText } from "../../lib/errors";
import { app, back, go, store, toast } from "../../shell";
import { DigitCells, Field, FieldCell, FieldGrid, FormActions, PageHeader, Switch, TextField } from "../../ui";
import { SearchPick } from "../form/controls";
import { BRANCH_CODE_ERROR, BRANCH_FIELDS, factError, shownErrors } from "./logic";
import { knownRdo } from "./page";
import { FormCard } from "./NewTaxpayer";
import { pickOptions } from "./ProfileTab";

interface Draft {
  code: string;
  name: string;
  values: Record<BranchFieldName, string>;
  inherit: Record<BranchFieldName, boolean>;
  touched: string[];
  submitted: boolean;
  hostErrors: Record<string, string>;
  error: string | null;
}

/** Every problem with the branch as typed, by field. */
function problems(d: Draft): Record<string, string> {
  const e: Record<string, string> = {};
  if (!/^\d{5}$/.test(d.code)) e.code = BRANCH_CODE_ERROR;
  if (!d.name.trim()) e.name = "Type a branch name, like Cebu branch.";
  for (const [f] of BRANCH_FIELDS) {
    const error = d.inherit[f] ? null : factError(f, d.values[f], knownRdo);
    if (error) e[f] = error;
  }
  return e;
}

export function NewBranchPage() {
  const first = untracked(() => {
    app.closePanel();
    const t = app.taxpayer();
    let suggestion = "";
    try {
      if (t) suggestion = store.call("suggest_next_branch_code", t.tin_root);
    } catch {
      /* no suggestion: the box starts empty */
    }
    const head = t?.branches.find((b) => b.is_head_office) ?? t?.branches[0];
    const values = {} as Record<BranchFieldName, string>;
    const inherit = {} as Record<BranchFieldName, boolean>;
    for (const [f] of BRANCH_FIELDS) {
      values[f] = head ? head.facts[f] || "" : "";
      inherit[f] = Boolean(head);
    }
    return { code: suggestion, name: "", values, inherit, touched: [], submitted: false, hostErrors: {}, error: null } as Draft;
  });
  const d = signal<Draft>(first);
  const head = () => app.taxpayer()?.branches.find((b) => b.is_head_office) ?? null;
  const errors = () => shownErrors(problems(d()), d().touched, d().submitted, d().hostErrors);
  const patch = (p: Partial<Draft>, key: string) => {
    const cur = d();
    const hostErrors = { ...cur.hostErrors };
    delete hostErrors[key];
    d.set({ ...cur, ...p, hostErrors, error: null });
  };
  const touch = (key: string) => {
    const cur = d();
    if (!cur.touched.includes(key)) d.set({ ...cur, touched: [...cur.touched, key] });
  };
  const setField = (f: BranchFieldName, v: string) => patch({ values: { ...d().values, [f]: v } }, f);
  const setInherit = (f: BranchFieldName, on: boolean) => {
    const cur = d();
    const values = on ? { ...cur.values, [f]: head()?.facts[f] || "" } : cur.values;
    d.set({ ...cur, values, inherit: { ...cur.inherit, [f]: on } });
  };

  function create(): void {
    const t = app.taxpayer();
    const cur = { ...d(), submitted: true, error: null };
    d.set(cur);
    if (!t || Object.keys(problems(cur)).length) return;
    const name = cur.name.trim();
    const fields: Partial<Record<BranchFieldName, string | null>> = {};
    for (const [f] of BRANCH_FIELDS) fields[f] = cur.inherit[f] ? null : cur.values[f].trim();
    try {
      store.write("create_branch", t.tin_root, { branch_code: cur.code, name, fields });
    } catch (e) {
      const text = errorText(e);
      if (/branch code/i.test(text)) d.set({ ...cur, hostErrors: { ...cur.hostErrors, code: text } });
      else d.set({ ...cur, error: text });
      return;
    }
    app.selectBranch(cur.code);
    go("profile");
    toast(`${name} added.`);
  }

  return (
    <View testID="new-branch" class="w-full">
      <PageHeader
        title="New branch"
        lede={`A branch of ${app.taxpayer()?.name ?? ""}. It starts with the head office's details; switch any of them off to set its own.`}
      />
      <FormCard
        testID="new-branch-card"
        error={d().error}
        actions={
          <FormActions
            testID="new-branch"
            saveLabel="Add branch"
            onCancel={() => {
              if (!back()) go("profile");
            }}
            onSave={create}
          />
        }
      >
        <FieldGrid>
          <FieldCell>
            <Field label="Branch code" error={errors().code}>
              <DigitCells
                testID="new-branch-code"
                label="Branch code"
                length={5}
                value={d().code}
                onChange={(v) => patch({ code: v }, "code")}
                onBlur={() => touch("code")}
                error={Boolean(errors().code)}
              />
            </Field>
          </FieldCell>
          <FieldCell>
            <Field label="Branch name" error={errors().name}>
              <TextField
                testID="new-branch-name"
                accessibilityLabel="Branch name"
                placeholder="Cebu branch"
                value={d().name}
                onChange={(v) => patch({ name: v }, "name")}
                onBlur={() => touch("name")}
                error={Boolean(errors().name)}
              />
            </Field>
          </FieldCell>
          <Each of={BRANCH_FIELDS} key={(f) => f[0]}>
            {(f) => (
              <FieldCell full={f()[0] === "registered_address" || f()[0] === "email"}>
                <Field
                  label={f()[1]}
                  error={errors()[f()[0]]}
                  inheritSwitch={
                    <Switch
                      testID={`new-branch-inherit-${f()[0]}`}
                      checked={d().inherit[f()[0]]}
                      onChange={(on) => setInherit(f()[0], on)}
                      label={`${f()[1]}: same as head office`}
                    />
                  }
                >
                  <If
                    when={f()[0] === "rdo_code" || f()[0] === "zip_code"}
                    else={
                      <TextField
                        testID={`new-branch-${f()[0]}`}
                        accessibilityLabel={f()[1]}
                        value={d().values[f()[0]]}
                        onChange={(v) => setField(f()[0], v)}
                        onBlur={() => touch(f()[0])}
                        disabled={d().inherit[f()[0]]}
                        error={Boolean(errors()[f()[0]])}
                      />
                    }
                  >
                    <SearchPick
                      testID={`new-branch-${f()[0]}`}
                      label={f()[1]}
                      options={pickOptions(f()[0] as "rdo_code" | "zip_code")}
                      value={d().values[f()[0]]}
                      onPick={(v) => {
                        setField(f()[0], v);
                        touch(f()[0]);
                      }}
                      disabled={d().inherit[f()[0]]}
                      error={Boolean(errors()[f()[0]])}
                    />
                  </If>
                </Field>
              </FieldCell>
            )}
          </Each>
        </FieldGrid>
      </FormCard>
    </View>
  );
}
