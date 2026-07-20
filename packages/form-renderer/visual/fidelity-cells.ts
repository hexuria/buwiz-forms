// Scoring-cell construction for `cell-edge-f1-v1`.
//
// Cells are the union of three sources, deduplicated by (x,y,w,h):
//   N  - the reviewed named critical regions (the geometry anchors)
//   G0 - a non-overlapping 64x64 device-px grid at origin (0,0), clipped
//   G1 - the same grid at origin (32,32), clipped
//
// The two grids exist SOLELY to close a measured coverage attack. Three
// red-team defects - a mirrored PDF417, a displaced masthead, and a fabricated
// "NOT VALID FOR FILING" advisory line - each scored a perfect 1.000000
// because they fell outside every one of the 113 named regions. Coverage
// cannot be a review promise; it must be a construction. The 32px offset
// guarantees any defect up to 32x32 px lies wholly inside at least one cell,
// so a defect straddling a G0 boundary cannot be diluted into invisibility.
//
// See docs/form-print-readiness/official-fidelity-criterion-v1.md section 1.2.

import crypto from "node:crypto";
import { criticalRegionsFor } from "./regions/2551q";

export const GRID_SIZE_PX = 64;
export const GRID_OFFSET_PX = 32;
/** Below this, single-pixel changes swing F1; measured, see criterion 2. */
export const MIN_CELL_EDGE_PIXELS = 200;
/** With G0 u G1 the achieved value should be ~1.0; slack for page margins. */
export const MIN_EDGE_COVERAGE = 0.98;

export interface FidelityCell {
  id: string;
  kind: "named" | "grid";
  x: number;
  y: number;
  width: number;
  height: number;
}

function clipCell(
  id: string,
  kind: "named" | "grid",
  x: number,
  y: number,
  width: number,
  height: number,
  pageWidth: number,
  pageHeight: number
): FidelityCell | null {
  const x0 = Math.max(0, x);
  const y0 = Math.max(0, y);
  const x1 = Math.min(pageWidth, x + width);
  const y1 = Math.min(pageHeight, y + height);
  if (x1 <= x0 || y1 <= y0) return null;
  return { id, kind, x: x0, y: y0, width: x1 - x0, height: y1 - y0 };
}

function appendGrid(
  cells: FidelityCell[],
  seen: Set<string>,
  originX: number,
  originY: number,
  label: string,
  pageWidth: number,
  pageHeight: number
) {
  for (let y = originY, row = 0; y < pageHeight; y += GRID_SIZE_PX, row += 1) {
    for (let x = originX, col = 0; x < pageWidth; x += GRID_SIZE_PX, col += 1) {
      const cell = clipCell(
        `${label}-r${row}-c${col}`,
        "grid",
        x,
        y,
        GRID_SIZE_PX,
        GRID_SIZE_PX,
        pageWidth,
        pageHeight
      );
      if (!cell) continue;
      const key = `${cell.x},${cell.y},${cell.width},${cell.height}`;
      if (seen.has(key)) continue;
      seen.add(key);
      cells.push(cell);
    }
  }
}

/**
 * Deterministic cell table for one official page. Ordering is stable: named
 * regions in reviewed order, then G0 in raster order, then G1 in raster
 * order. The audit reconstructs this table independently and compares
 * `cellTableSha256`, so ordering and geometry are part of the contract.
 */
export function buildFidelityCells(
  formCode: string,
  formRevision: string,
  pageNumber: number,
  pageWidth: number,
  pageHeight: number
): FidelityCell[] {
  const cells: FidelityCell[] = [];
  const seen = new Set<string>();

  for (const region of criticalRegionsFor(formCode, formRevision, pageNumber)) {
    const cell = clipCell(
      region.name,
      "named",
      region.x,
      region.y,
      region.width,
      region.height,
      pageWidth,
      pageHeight
    );
    if (!cell) continue;
    const key = `${cell.x},${cell.y},${cell.width},${cell.height}`;
    if (seen.has(key)) continue;
    seen.add(key);
    cells.push(cell);
  }

  appendGrid(cells, seen, 0, 0, "g0", pageWidth, pageHeight);
  appendGrid(
    cells,
    seen,
    GRID_OFFSET_PX,
    GRID_OFFSET_PX,
    "g1",
    pageWidth,
    pageHeight
  );

  return cells;
}

/**
 * Hash of the cell table's geometry, in table order. Binds the audit's
 * independently reconstructed table to the one the producer scored.
 */
export function cellTableSha256(cells: readonly FidelityCell[]): string {
  const canonical = cells
    .map((c) => `${c.kind}:${c.id}:${c.x},${c.y},${c.width},${c.height}`)
    .join("\n");
  return crypto.createHash("sha256").update(`${canonical}\n`).digest("hex");
}
