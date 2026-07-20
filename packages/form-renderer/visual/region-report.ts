// Region-ranked localization report for a complete-page visual comparison.
//
// This is a NON-PROMOTIONAL diagnostic: it never changes the 1% gate, it only
// says WHERE the changed pixels live so calibration can target the worst
// regions instead of guessing from a single page-level percentage. Every
// changed pixel is attributed exactly once — to the first matching named
// critical region, else to a uniform fallback tile — and the report asserts
// that the attributed total reconciles with the page's changed-pixel count.

import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { PNG } from "pngjs";
import { INK_THRESHOLD } from "./official-fidelity";
import { darkInkMask, dilateMask } from "./official-page-diff";
import type { CriticalRegion } from "./regions/2551q";

const INK_TOLERANCE_RADIUS = 2;
const DEFAULT_GRID = { cols: 12, rows: 18 };
const DEFAULT_TOP_N = 10;
const CROP_PADDING_PX = 8;
const STRIP_SEPARATOR_PX = 4;

export interface RegionDiffStat {
  kind: "named" | "tile";
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
  region_pixels: number;
  changed_pixels: number;
  changed_percent_of_region: number;
  changed_percent_of_page: number;
  ink_missing_pixels: number;
  ink_unexpected_pixels: number;
  rank: number;
}

export interface RegionReportResult {
  jsonPath: string;
  jsonSha256: string;
  markdownPath: string;
  stats: RegionDiffStat[];
}

export function bucketDiff(
  expected: PNG,
  actual: PNG,
  diff: PNG,
  regions: readonly CriticalRegion[],
  grid: { cols: number; rows: number } = DEFAULT_GRID
): RegionDiffStat[] {
  const { width, height } = expected;
  if (
    actual.width !== width ||
    actual.height !== height ||
    diff.width !== width ||
    diff.height !== height
  ) {
    throw new Error("region report inputs must share one page geometry");
  }

  // Pixel ownership: named regions first (array order wins overlaps), then
  // one fallback tile per grid cell for pixels outside every named region.
  const owner = new Int32Array(width * height).fill(-1);
  const stats: Array<Omit<RegionDiffStat, "rank">> = [];
  for (const region of regions) {
    const bucketIndex = stats.length;
    stats.push(emptyStat("named", region.name, region));
    const x2 = Math.min(width, region.x + region.width);
    const y2 = Math.min(height, region.y + region.height);
    for (let y = Math.max(0, region.y); y < y2; y += 1) {
      for (let x = Math.max(0, region.x); x < x2; x += 1) {
        const index = y * width + x;
        if (owner[index] === -1) owner[index] = bucketIndex;
      }
    }
  }
  const tileWidth = Math.ceil(width / grid.cols);
  const tileHeight = Math.ceil(height / grid.rows);
  const tileBuckets = new Map<number, number>();

  const expectedInk = darkInkMask(expected, INK_THRESHOLD);
  const actualInk = darkInkMask(actual, INK_THRESHOLD);
  const expectedDilated = dilateMask(expectedInk, width, height, INK_TOLERANCE_RADIUS);
  const actualDilated = dilateMask(actualInk, width, height, INK_TOLERANCE_RADIUS);

  let pageChangedPixels = 0;
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const index = y * width + x;
      let bucketIndex = owner[index];
      if (bucketIndex === -1) {
        const tileKey =
          Math.floor(y / tileHeight) * grid.cols + Math.floor(x / tileWidth);
        let mapped = tileBuckets.get(tileKey);
        if (mapped === undefined) {
          mapped = stats.length;
          tileBuckets.set(tileKey, mapped);
          const column = tileKey % grid.cols;
          const row = Math.floor(tileKey / grid.cols);
          stats.push(
            emptyStat("tile", `tile-r${row + 1}-c${column + 1}`, {
              x: column * tileWidth,
              y: row * tileHeight,
              width: Math.min(tileWidth, width - column * tileWidth),
              height: Math.min(tileHeight, height - row * tileHeight)
            })
          );
        }
        bucketIndex = mapped;
      }
      const stat = stats[bucketIndex];
      stat.region_pixels += 1;
      const changed = diff.data[index * 4 + 3] !== 0;
      if (changed) {
        stat.changed_pixels += 1;
        pageChangedPixels += 1;
      }
      if (expectedInk[index] === 1 && actualDilated[index] !== 1) {
        stat.ink_missing_pixels += 1;
      }
      if (actualInk[index] === 1 && expectedDilated[index] !== 1) {
        stat.ink_unexpected_pixels += 1;
      }
    }
  }

  const accounted = stats.reduce((sum, stat) => sum + stat.changed_pixels, 0);
  if (accounted !== pageChangedPixels) {
    throw new Error(
      `region attribution lost pixels: accounted ${accounted}, page ${pageChangedPixels}`
    );
  }

  return stats
    .filter((stat) => stat.kind === "named" || stat.changed_pixels > 0)
    .map((stat) => ({
      ...stat,
      changed_percent_of_region:
        stat.region_pixels === 0
          ? 0
          : (stat.changed_pixels / stat.region_pixels) * 100,
      changed_percent_of_page:
        pageChangedPixels === 0
          ? 0
          : (stat.changed_pixels / pageChangedPixels) * 100
    }))
    .sort(
      (left, right) =>
        right.changed_pixels - left.changed_pixels || left.id.localeCompare(right.id)
    )
    .map((stat, index) => ({ ...stat, rank: index + 1 }));
}

export function writeRegionReport(options: {
  outputDir: string;
  artifactStem: string;
  formCode: string;
  formRevision: string;
  pageNumber: number;
  comparison: string;
  expected: PNG;
  actual: PNG;
  diff: PNG;
  regions: readonly CriticalRegion[];
  topN?: number;
}): RegionReportResult {
  const stats = bucketDiff(options.expected, options.actual, options.diff, options.regions);
  const pageChangedPixels = stats.reduce((sum, stat) => sum + stat.changed_pixels, 0);
  const topN = options.topN ?? DEFAULT_TOP_N;
  fs.mkdirSync(options.outputDir, { recursive: true });

  const crops: Array<{ rank: number; id: string; file: string }> = [];
  for (const stat of stats.slice(0, topN)) {
    if (stat.changed_pixels === 0) break;
    const fileName = `${options.artifactStem}-region-${stat.rank}-${slugify(stat.id)}.png`;
    writeRegionStrip(
      path.join(options.outputDir, fileName),
      options.expected,
      options.actual,
      options.diff,
      stat
    );
    crops.push({ rank: stat.rank, id: stat.id, file: fileName });
  }

  const report = {
    purpose: "non_promotional_localization_diagnostic",
    promotion_eligible: false,
    gate_unchanged: true,
    form_code: options.formCode,
    form_revision: options.formRevision,
    page: options.pageNumber,
    comparison: options.comparison,
    page_changed_pixels: pageChangedPixels,
    accounted_changed_pixels: pageChangedPixels,
    grid: DEFAULT_GRID,
    crops,
    regions: stats
  };
  const jsonPath = path.join(
    options.outputDir,
    `${options.artifactStem}-region-report.json`
  );
  const payload = `${JSON.stringify(report, null, 2)}\n`;
  fs.writeFileSync(jsonPath, payload);

  const markdownPath = path.join(
    options.outputDir,
    `${options.artifactStem}-region-report.md`
  );
  fs.writeFileSync(markdownPath, renderMarkdown(report, stats, topN));

  return {
    jsonPath,
    jsonSha256: crypto.createHash("sha256").update(payload).digest("hex"),
    markdownPath,
    stats
  };
}

function emptyStat(
  kind: "named" | "tile",
  id: string,
  bounds: { x: number; y: number; width: number; height: number }
): Omit<RegionDiffStat, "rank"> {
  return {
    kind,
    id,
    x: bounds.x,
    y: bounds.y,
    width: bounds.width,
    height: bounds.height,
    region_pixels: 0,
    changed_pixels: 0,
    changed_percent_of_region: 0,
    changed_percent_of_page: 0,
    ink_missing_pixels: 0,
    ink_unexpected_pixels: 0
  };
}

function renderMarkdown(
  report: { page: number; page_changed_pixels: number; form_code: string },
  stats: RegionDiffStat[],
  topN: number
): string {
  const lines = [
    `# ${report.form_code} page ${report.page} — region-ranked diff (non-promotional diagnostic)`,
    "",
    `Page changed pixels: ${report.page_changed_pixels}. Worst regions first;`,
    "fix the top rows before touching anything below them.",
    "",
    "| rank | region | changed px | % of region | % of page | ink missing | ink unexpected |",
    "| ---: | --- | ---: | ---: | ---: | ---: | ---: |"
  ];
  for (const stat of stats.slice(0, Math.max(topN, 20))) {
    if (stat.changed_pixels === 0) break;
    lines.push(
      `| ${stat.rank} | ${stat.id} | ${stat.changed_pixels} | ` +
        `${stat.changed_percent_of_region.toFixed(3)}% | ` +
        `${stat.changed_percent_of_page.toFixed(2)}% | ` +
        `${stat.ink_missing_pixels} | ${stat.ink_unexpected_pixels} |`
    );
  }
  lines.push("");
  return lines.join("\n");
}

function writeRegionStrip(
  filePath: string,
  expected: PNG,
  actual: PNG,
  diff: PNG,
  region: { x: number; y: number; width: number; height: number }
) {
  const x0 = Math.max(0, region.x - CROP_PADDING_PX);
  const y0 = Math.max(0, region.y - CROP_PADDING_PX);
  const x1 = Math.min(expected.width, region.x + region.width + CROP_PADDING_PX);
  const y1 = Math.min(expected.height, region.y + region.height + CROP_PADDING_PX);
  const cropWidth = x1 - x0;
  const cropHeight = y1 - y0;
  if (cropWidth <= 0 || cropHeight <= 0) return;

  const strip = new PNG({
    width: cropWidth * 3 + STRIP_SEPARATOR_PX * 2,
    height: cropHeight
  });
  strip.data.fill(255);
  blitCrop(expected, strip, x0, y0, cropWidth, cropHeight, 0, false);
  blitCrop(actual, strip, x0, y0, cropWidth, cropHeight, cropWidth + STRIP_SEPARATOR_PX, false);
  blitCrop(diff, strip, x0, y0, cropWidth, cropHeight, (cropWidth + STRIP_SEPARATOR_PX) * 2, true);
  fs.writeFileSync(filePath, PNG.sync.write(strip));
}

function blitCrop(
  source: PNG,
  target: PNG,
  x0: number,
  y0: number,
  cropWidth: number,
  cropHeight: number,
  targetX: number,
  maskOverWhite: boolean
) {
  for (let y = 0; y < cropHeight; y += 1) {
    for (let x = 0; x < cropWidth; x += 1) {
      const sourceOffset = ((y0 + y) * source.width + x0 + x) * 4;
      const targetOffset = (y * target.width + targetX + x) * 4;
      if (maskOverWhite && source.data[sourceOffset + 3] === 0) {
        continue; // keep the white background where the mask is transparent
      }
      target.data[targetOffset] = source.data[sourceOffset];
      target.data[targetOffset + 1] = source.data[sourceOffset + 1];
      target.data[targetOffset + 2] = source.data[sourceOffset + 2];
      target.data[targetOffset + 3] = 255;
    }
  }
}

function slugify(value: string) {
  return value.toLowerCase().replaceAll(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}
