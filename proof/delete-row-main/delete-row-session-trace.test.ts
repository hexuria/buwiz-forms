import { test } from "nsh/test";
import { bir } from "./bir";
import { DEMO, createDemoTaxpayer, useDatabase, warmForms } from "./bir/testing";

useDatabase("/home/ubuntu/forms-validation/delete-row-main-proof/runtime.db");
warmForms();

const OUT = "/home/ubuntu/forms-validation/delete-row-main-proof";
type Any = Record<string, any>;

function emit(name: string, value: unknown) {
  console.log(`ARTIFACT:${name}:${JSON.stringify(value)}`);
}

function schedulesOf(form: string, schema: Any) {
  const rows: Any[] = [];
  (schema.parts || []).forEach((part: Any, partIndex: number) => {
    (part.items || []).forEach((item: Any, itemIndex: number) => {
      if (!item.rows || !item.cols || !item.keys) return;
      const visibleColumns = (item.cols as Any[])
        .map((col, index) => ({ ...col, index }))
        .filter((col) => col.t !== "hidden");
      const totalTable = visibleColumns.length > 0 && visibleColumns.every((col) => !!col.c);
      const editableRowContent = visibleColumns.some((col) => !col.c);
      rows.push({
        form,
        part: part.n,
        part_index: partIndex,
        item_index: itemIndex,
        item_no: item.no,
        title: item.l,
        rows: item.rows,
        row_count_actual: item.keys.length,
        first_row_keys: item.keys[0] || [],
        row_labels: item.row_labels || null,
        numbered: item.numbered ?? null,
        start: item.start ?? null,
        columns: (item.cols as Any[]).map((col, index) => ({ index, label: col.l, type: col.t, computed: !!col.c, hidden: col.t === "hidden" })),
        visible_column_count: visibleColumns.length,
        total_table_by_schedule_logic: totalTable,
        editable_row_content_by_static_schema: editableRowContent,
        delete_offered_by_static_schedule_logic: !totalTable && editableRowContent,
        draft_json_key: null,
        draft_json_key_evidence: "not discoverable from transformed schema alone",
      });
    });
  });
  return rows;
}

function scheduleContaining(schema: Any, key: string) {
  for (const part of schema.parts || []) {
    for (const item of part.items || []) {
      if (item.keys?.some((row: string[]) => row.includes(key))) return item;
    }
  }
  throw new Error(`schedule containing ${key} not found`);
}

function exactDeleteEdits(item: Any, rowIndex: number, locks: Record<string, string>, off: Record<string, string>) {
  const cols = item.cols || [];
  const keys = item.keys[rowIndex] || [];
  const edits: Record<string, string> = {};
  const excluded: Any[] = [];
  keys.forEach((key, index) => {
    const col = cols[index] || {};
    let reason = "";
    if (col.t === "hidden") reason = "hidden";
    else if (col.c) reason = "computed column, locked on page";
    else if (locks[key] !== undefined) reason = `host lock: ${locks[key]}`;
    else if (off[key]) reason = `off: ${off[key]}`;
    if (reason) excluded.push({ key, column: col.l, reason });
    else edits[key] = "";
  });
  return { edits, excluded, row_keys: keys };
}

function pickValues(values: Record<string, string>, prefixOrKeys: string[]) {
  const out: Record<string, string> = {};
  for (const key of Object.keys(values)) {
    if (prefixOrKeys.some((want) => key === want || key.startsWith(want))) out[key] = values[key];
  }
  return out;
}

test("inventory every delete-row-capable schedule and trace 2551Q/1601EQ deletes", async () => {
  createDemoTaxpayer();

  const index = bir.screen_forms_index();
  const inventory: Any[] = [];
  const schemaErrors: Any[] = [];
  for (const entry of index) {
    try {
      const schema = bir.screen_form_schema(entry.c) as Any;
      inventory.push(...schedulesOf(entry.c, schema));
    } catch (error) {
      schemaErrors.push({ form: entry.c, error: String((error as Error).message || error) });
    }
  }
  const inventoryResult = {
    evidence: "bir.screen_form_schema for every bir.screen_forms_index entry after host transformations; delete_offered uses Schedule.tsx static logic: not a total table and at least one visible non-computed row column. draft_json_key is null unless separately established.",
    form_count: index.length,
    schedule_count: inventory.length,
    delete_offered_count: inventory.filter((row) => row.delete_offered_by_static_schedule_logic).length,
    schema_errors: schemaErrors,
    schedules: inventory,
  };
  emit("inventory.json", inventoryResult);

  // Session-faithful 2551Q/1601EQ correction: reproduce FormSession.fill, including
  // removal holds and the zero retry for refused emptied numeric boxes.
  const NUMBER_KINDS = new Set(["money", "num", "pct", "int"]);
  const sessionFill = (form: string, draft: Any, editsIn: Record<string, Any>, kindOf: Record<string, string>) => {
    const edits = { ...editsIn };
    const first = bir.screen_form_fill(form, draft, edits) as Any;
    const result: Any = {
      first: {
        rejected: first.rejected || {},
        removes: first.removes || null,
        draft: first.draft,
        values: first.values,
      },
      zero_retry: null,
      final: first,
      confirmation_required: !!first.removes,
      final_edits: { ...edits },
    };
    if (first.removes) {
      result.final_edits = { ...edits };
      return result;
    }
    const refused = Object.keys(first.rejected || {}).filter((key) => key in edits && String(edits[key]).trim() === "" && NUMBER_KINDS.has(kindOf[key] || ""));
    if (!refused.length) return result;
    const zero = Object.fromEntries(refused.map((key) => [key, "0"]));
    const second = bir.screen_form_fill(form, first.draft, zero) as Any;
    const cleared = refused.filter((key) => !(key in (second.rejected || {})));
    const rejected = { ...(first.rejected || {}) };
    for (const key of cleared) {
      delete rejected[key];
      edits[key] = "0";
    }
    result.zero_retry = {
      edits: zero,
      rejected: second.rejected || {},
      removes: second.removes || null,
      draft: second.draft,
      values: second.values,
      cleared,
      merged_rejected: rejected,
    };
    result.final = { ...second, rejected, cleared };
    result.final_edits = { ...edits };
    return result;
  };
  const kindMap = (item: Any) => {
    const out: Record<string, string> = {};
    (item.keys || []).flat().forEach((key: string) => {
      const index = (item.keys || []).flat().indexOf(key);
      out[key] = item.cols?.[index]?.t || "";
    });
    return out;
  };

  // 2551Q Schedule 1.
  const qOpen = await bir.screen_form_open(DEMO.tin, "2551Q", 2026, 4);
  const qSchema = bir.screen_form_schema("2551Q") as Any;
  let q = qOpen.draft as Any;
  const qLocks: Record<string, string> = { ...(qOpen.locks || {}) };
  const qFill = (label: string, edits: Record<string, string>) => {
    const filled = bir.screen_form_fill("2551Q", q, edits) as Any;
    q = filled.draft as Any;
    for (const [key, lock] of Object.entries(filled.relocks || {})) {
      if (lock === null) delete qLocks[key]; else qLocks[key] = lock as string;
    }
    return { label, edits, rejected: filled.rejected, removes: filled.removes || null, draft_schedule_1: q.schedule_1, values: pickValues(filled.values, ["drpATC", "txtATCAmt", "txtATCRate", "txtATCDue", "txtTotalSched1", "frm2551Qv2018:txt14"]) };
  };
  const qSteps = [
    qFill("seed row 1 ATC", { drpATC1: "1" }),
    qFill("seed row 1 amount", { txtATCAmt1: "1000" }),
    qFill("seed row 2 ATC", { drpATC2: "2" }),
    qFill("seed row 2 amount", { txtATCAmt2: "7777" }),
  ];
  const qBefore = JSON.parse(JSON.stringify(q));
  const qItem = scheduleContaining(qSchema, "drpATC1");
  const qDelete = exactDeleteEdits(qItem, 0, qLocks, {});
  const qSession = sessionFill("2551Q", qBefore, qDelete.edits, kindMap(qItem));
  q = qSession.final.draft as Any;
  const qResult = {
    form: "2551Q",
    correction: "session-faithful: blank writes in one batch; no numeric zero retry when removes exists; otherwise retry rejected blank money/num/pct/int keys as 0 in a second fill",
    schedule_item: { no: qItem.no, title: qItem.l, rows: qItem.rows, keys: qItem.keys, cols: qItem.cols },
    seed_steps: qSteps,
    before: qBefore,
    exact_delete: qDelete,
    session_fill: {
      first_rejected: qSession.first.rejected,
      first_removes: qSession.first.removes,
      first_draft_schedule_1: qSession.first.draft.schedule_1,
      confirmation_required: qSession.confirmation_required,
      confirmation_message: qSession.first.removes?.message || null,
      zero_retry: qSession.zero_retry ? {
        edits: qSession.zero_retry.edits,
        rejected: qSession.zero_retry.rejected,
        removes: qSession.zero_retry.removes,
        draft_schedule_1: qSession.zero_retry.draft.schedule_1,
        merged_rejected: qSession.zero_retry.merged_rejected,
        cleared: qSession.zero_retry.cleared,
      } : null,
      final_edits: qSession.final_edits,
    },
    after: q,
    after_values: pickValues(qSession.final.values, ["drpATC", "txtATCAmt", "txtATCRate", "txtATCDue", "txtTotalSched1", "frm2551Qv2018:txt14"]),
    analysis: {
      row_count_before: qBefore.schedule_1?.length,
      row_count_after: q.schedule_1?.length,
      row0_before: qBefore.schedule_1?.[0],
      row0_after: q.schedule_1?.[0],
      row1_before: qBefore.schedule_1?.[1],
      row1_after: q.schedule_1?.[1],
      total_tax_due_before: qBefore.total_tax_due,
      total_tax_due_after: q.total_tax_due,
    },
  };
  emit("session-2551q-runtime.json", qResult);
  emit("session-2551q-after-draft.json", q);

  // 1601EQ ATC rows.
  const eqOpen = await bir.screen_form_open(DEMO.tin, "1601EQ", 2026, 4);
  const eqSchema = bir.screen_form_schema("1601EQ") as Any;
  let eq = eqOpen.draft as Any;
  const eqLocks: Record<string, string> = { ...(eqOpen.locks || {}) };
  const eqFill = (label: string, edits: Record<string, Any>) => {
    const filled = bir.screen_form_fill("1601EQ", eq, edits) as Any;
    eq = filled.draft as Any;
    for (const [key, lock] of Object.entries(filled.relocks || {})) {
      if (lock === null) delete eqLocks[key]; else eqLocks[key] = lock as string;
    }
    return { label, edits, rejected: filled.rejected, removes: filled.removes || null, draft_schedule: eq.schedule, values: pickValues(filled.values, ["frm1601EQ:txtAtcCd", "frm1601EQ:txtTaxBase", "frm1601EQ:txtTaxRate", "frm1601EQ:txtTaxbeWithHeld"]) };
  };
  const eqSteps = [
    eqFill("answer any taxes withheld yes", { "frm1601EQ:optWithheld:Y": true }),
    eqFill("answer category private", { "frm1601EQ:optCategory:P": true }),
    eqFill("seed row 1 ATC WI010", { "frm1601EQ:txtAtcCd1": "WI010" }),
    eqFill("seed row 1 base", { "frm1601EQ:txtTaxBase1": "1000" }),
    eqFill("seed row 2 ATC WI011", { "frm1601EQ:txtAtcCd2": "WI011" }),
    eqFill("seed row 2 base", { "frm1601EQ:txtTaxBase2": "2000" }),
  ];
  const eqBefore = JSON.parse(JSON.stringify(eq));
  const eqItem = scheduleContaining(eqSchema, "frm1601EQ:txtAtcCd1");
  const eqDelete = exactDeleteEdits(eqItem, 0, eqLocks, {});
  const eqSession = sessionFill("1601EQ", eqBefore, eqDelete.edits, kindMap(eqItem));
  eq = eqSession.final.draft as Any;

  // Decompose raw host operations in both orders for causation.
  const eqAtcOnly = bir.screen_form_fill("1601EQ", eqBefore, { "frm1601EQ:txtAtcCd1": "" }) as Any;
  const eqAtcThenBase = bir.screen_form_fill("1601EQ", eqAtcOnly.draft, { "frm1601EQ:txtTaxBase1": "" }) as Any;
  const eqBaseOnly = bir.screen_form_fill("1601EQ", eqBefore, { "frm1601EQ:txtTaxBase1": "" }) as Any;
  const eqBaseThenAtc = bir.screen_form_fill("1601EQ", eqBaseOnly.draft, { "frm1601EQ:txtAtcCd1": "" }) as Any;

  const eqResult = {
    form: "1601EQ",
    correction: "session-faithful: blank writes in one batch; removes holds the first host answer for confirmation and skips every numeric zero retry",
    schedule_item: { no: eqItem.no, title: eqItem.l, rows: eqItem.rows, keys: eqItem.keys.slice(0, 2), cols: eqItem.cols },
    seed_steps: eqSteps,
    before: eqBefore,
    exact_delete: eqDelete,
    session_fill: {
      first_rejected: eqSession.first.rejected,
      first_removes: eqSession.first.removes,
      first_draft_schedule: eqSession.first.draft.schedule,
      confirmation_required: eqSession.confirmation_required,
      confirmation_message: eqSession.first.removes?.message || null,
      zero_retry: eqSession.zero_retry ? {
        edits: eqSession.zero_retry.edits,
        rejected: eqSession.zero_retry.rejected,
        removes: eqSession.zero_retry.removes,
        draft_schedule: eqSession.zero_retry.draft.schedule,
        merged_rejected: eqSession.zero_retry.merged_rejected,
        cleared: eqSession.zero_retry.cleared,
      } : null,
      final_edits: eqSession.final_edits,
    },
    after: eq,
    after_values: pickValues(eqSession.final.values, ["frm1601EQ:txtAtcCd", "frm1601EQ:txtTaxBase", "frm1601EQ:txtTaxRate", "frm1601EQ:txtTaxbeWithHeld"]),
    decomposition: {
      atc_blank_only: { schedule: (eqAtcOnly.draft as Any).schedule, rejected: eqAtcOnly.rejected, removes: eqAtcOnly.removes || null },
      atc_blank_then_base_blank: { schedule: (eqAtcThenBase.draft as Any).schedule, rejected: eqAtcThenBase.rejected, removes: eqAtcThenBase.removes || null },
      base_blank_only: { schedule: (eqBaseOnly.draft as Any).schedule, rejected: eqBaseOnly.rejected, removes: eqBaseOnly.removes || null },
      base_blank_then_atc_blank: { schedule: (eqBaseThenAtc.draft as Any).schedule, rejected: eqBaseThenAtc.rejected, removes: eqBaseThenAtc.removes || null },
    },
    analysis: {
      row_count_before: eqBefore.schedule?.length,
      row_count_after: eq.schedule?.length,
      rows_before: eqBefore.schedule,
      rows_after: eq.schedule,
      total_taxes_withheld_before: eqBefore.total_taxes_withheld,
      total_taxes_withheld_after: eq.total_taxes_withheld,
      exact_delete_key_order: Object.keys(eqDelete.edits),
    },
  };
  emit("session-1601eq-runtime.json", eqResult);
  emit("session-1601eq-after-draft.json", eq);

  console.log("DELETEROW inventory schedules=" + inventory.length + " offered=" + inventoryResult.delete_offered_count);
});
