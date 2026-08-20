import type { RenderSchedule } from "@ebirforms/form-contracts";
import type { SchedulePolicy } from "@ebirforms/form-specs";

export interface SchedulePage {
  pageIndex: number;
  startRowIndex: number;
  rows: RenderSchedule["rows"];
  isContinuation: boolean;
  isFinal: boolean;
  summaryKind: "final_total" | "page_2_subtotal" | "continued";
}

export function paginateSchedule(
  schedule: RenderSchedule,
  policy: SchedulePolicy
): SchedulePage[] {
  const { firstPageRows, continuationPageRows, minimumRows } = policy;
  if (firstPageRows < 1 || continuationPageRows < 1) {
    throw new Error(`Schedule ${schedule.id} requires positive page capacities`);
  }

  const paddedRows = [...schedule.rows];
  while (paddedRows.length < minimumRows) {
    paddedRows.push({ key: `${schedule.id}-empty-${paddedRows.length + 1}`, cells: {} });
  }

  const pages: SchedulePage[] = [];
  let offset = 0;
  let capacity = firstPageRows;
  while (offset < paddedRows.length) {
    const rows = paddedRows.slice(offset, offset + capacity);
    const isContinuation = pages.length > 0;
    const isFinal = offset + rows.length >= paddedRows.length;
    pages.push({
      pageIndex: pages.length,
      startRowIndex: offset,
      rows,
      isContinuation,
      isFinal,
      summaryKind: isFinal
        ? "final_total"
        : isContinuation
          ? "continued"
          : "page_2_subtotal",
    });
    offset += rows.length;
    capacity = continuationPageRows;
  }
  return pages;
}
