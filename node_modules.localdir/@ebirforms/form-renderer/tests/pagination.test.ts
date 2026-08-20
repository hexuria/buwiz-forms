import type { RenderSchedule } from "@ebirforms/form-contracts";
import type { SchedulePolicy } from "@ebirforms/form-specs";
import { describe, expect, it } from "vitest";
import { paginateSchedule } from "../src/pagination";

function schedule(rowCount: number): RenderSchedule {
  return {
    id: "schedule_1",
    columns: [{ key: "atc", label: "ATC", alignment: "left" }],
    rows: Array.from({ length: rowCount }, (_, index) => ({
      key: `row-${index + 1}`,
      cells: { atc: { type: "text", value: `PT${index + 1}` } },
    }))
  };
}

const percentageTaxPolicy: SchedulePolicy = {
  minimumRows: 6,
  firstPageRows: 6,
  continuationPageRows: 12,
  repeatHeader: true,
  finalTotalsOnLastPage: true
};

describe("paginateSchedule", () => {
  it("pads 2551Q to the six rows printed on the official base page", () => {
    expect(paginateSchedule(schedule(1), percentageTaxPolicy)[0].rows).toHaveLength(6);
  });

  for (const [count, expectedPages] of [
    [0, 1],
    [1, 1],
    [4, 1],
    [5, 1],
    [6, 1],
    [7, 2],
    [10, 2],
    [100, 9],
    [1000, 84]
  ] as const) {
    it(`preserves every 2551Q stable key and page boundary for ${count} rows`, () => {
      const pages = paginateSchedule(schedule(count), percentageTaxPolicy);
      const realKeys = pages
        .flatMap((page) => page.rows)
        .map((row) => row.key)
        .filter((key) => !key.includes("-empty-"));
      expect(realKeys).toEqual(Array.from({ length: count }, (_, index) => `row-${index + 1}`));
      expect(new Set(realKeys).size).toBe(count);
      expect(pages).toHaveLength(expectedPages);
      expect(pages.map((page) => page.startRowIndex)).toEqual(
        pages.map((_, index) => (index === 0 ? 0 : 6 + (index - 1) * 12))
      );
      expect(pages.filter((page) => page.isFinal)).toHaveLength(1);
      expect(pages.at(-1)?.isFinal).toBe(true);
      expect(pages.at(-1)?.summaryKind).toBe("final_total");
      if (count > percentageTaxPolicy.firstPageRows) {
        expect(pages[0].summaryKind).toBe("page_2_subtotal");
      }
      for (const page of pages.slice(1, -1)) {
        expect(page.summaryKind).toBe("continued");
      }
    });
  }

  it("distinguishes the base carry-forward, intermediate pages, and final total", () => {
    expect(paginateSchedule(schedule(31), percentageTaxPolicy).map((page) => page.summaryKind))
      .toEqual(["page_2_subtotal", "continued", "continued", "final_total"]);
  });

  it("rejects a zero-capacity pagination policy", () => {
    expect(() =>
      paginateSchedule(schedule(1), {
        ...percentageTaxPolicy,
        firstPageRows: 0
      })
    ).toThrow(
      "positive page capacities"
    );
  });
});
