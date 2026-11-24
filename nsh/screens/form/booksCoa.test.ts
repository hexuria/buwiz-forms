// Unit tests for BooksCoaPicker's nested COA tree builder (Phase 2).
import { expect, test } from "nsh/test";
import type { BooksAccountNode } from "../../bir";
import { buildCoaTreeRows } from "./BooksCoaPicker";

const SAMPLE: BooksAccountNode[] = [
  {
    id: "acc_41000",
    code: "41000",
    name: "Sales Revenue",
    account_type: "income",
    parent_account_id: null,
    balance: "100000.00",
    subtree_balance: "150000.00",
    is_active: true,
    accrual_only: false,
  },
  {
    id: "acc_41010",
    code: "41010",
    name: "Sales to Government",
    account_type: "income",
    parent_account_id: "acc_41000",
    balance: "50000.00",
    subtree_balance: "50000.00",
    is_active: true,
    accrual_only: false,
  },
  {
    id: "acc_51000",
    code: "51000",
    name: "Salaries & Wages",
    account_type: "expense",
    parent_account_id: null,
    balance: "30000.00",
    subtree_balance: "30000.00",
    is_active: true,
    accrual_only: false,
  },
  {
    id: "acc_99999",
    code: "99999",
    name: "Inactive Legacy",
    account_type: "expense",
    parent_account_id: null,
    balance: "0.00",
    subtree_balance: "0.00",
    is_active: false,
    accrual_only: false,
  },
];

test("builds depth-first nested COA rows and hides inactive accounts", () => {
  const rows = buildCoaTreeRows(SAMPLE, "");
  expect(rows.map((r) => [r.node.code, r.depth, r.hasChildren])).toEqual([
    ["41000", 0, true],
    ["41010", 1, false],
    ["51000", 0, false],
  ]);
});

test("search keeps parent ancestor when a child account matches", () => {
  const rows = buildCoaTreeRows(SAMPLE, "government");
  expect(rows.map((r) => [r.node.code, r.depth])).toEqual([
    ["41000", 0],
    ["41010", 1],
  ]);
});
