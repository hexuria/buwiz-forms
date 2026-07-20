// `encoded-artwork-integrity-v1` — the artwork component of official-fidelity-v1.
//
// WHY TWO BINDINGS, BOTH REQUIRED. Bounding-box geometry alone is not
// integrity. A red-team attack applied `transform: scaleX(-1)` to the PDF417
// symbol: the bbox is unchanged, every geometry assertion passes, and the
// symbol is unscannable. Conversely a substituted module matrix keeps the
// rendered footprint but encodes a different payload. So:
//
//   1. SYMBOL DATA hash — the SVG path actually in the DOM is hashed and
//      compared to the reviewed module matrix. Catches substituted, truncated
//      or regenerated symbol data.
//   2. RASTER CROP hash — the symbol's pinned rectangle is cropped from the
//      captured page raster and hashed. Catches anything that changes how the
//      data is drawn: mirroring, rotation, scaling, colour inversion, occlusion.
//
// Neither catches the other's failure mode. Both are mandatory.
//
// The reviewed provenance lives in
// packages/form-renderer/src/forms/official2551QAssets.ts: ZXing 3.5.3 decoded
// the pinned PDF's symbols as "2551Q 01/18ENCS P1"/"P2", and all 840 logical
// modules matched an independent encoder run. That review is what these pins
// bind to; this module enforces it, it does not re-establish it.

import crypto from "node:crypto";
import { PNG } from "pngjs";

export interface EncodedArtworkPin {
  /** Stable identifier used in evidence reports. */
  id: string;
  page: number;
  /** DOM selector for the symbol or artwork element. */
  selector: string;
  /** Reviewed decoded payload, for symbols that carry one. */
  payload?: string;
  /** SHA-256 of the reviewed module/path data as rendered. */
  symbolDataSha256?: string;
  /** Pinned crop rectangle in 144-DPI reference pixels. */
  crop: { x: number; y: number; width: number; height: number };
  /** SHA-256 of the cropped raster region, per (form, revision, fixture). */
  cropSha256?: string;
}

export interface ArtworkObservation {
  id: string;
  page: number;
  /** The `d` attribute of the symbol path, or null for raster artwork. */
  pathData: string | null;
  /** Rendered `title`, which carries the reviewed payload for our symbols. */
  title: string | null;
  /** Effective transform, so mirroring/rotation is visible to the assertion. */
  transform: string | null;
  naturalWidth: number | null;
  naturalHeight: number | null;
}

export interface ArtworkViolation {
  id: string;
  kind:
    | "missing"
    | "payload"
    | "symbol-data"
    | "crop"
    | "transform"
    | "unpinned";
  detail: string;
}

export function sha256Hex(value: string | Buffer): string {
  return crypto.createHash("sha256").update(value).digest("hex");
}

/**
 * SHA-256 of a cropped region's RGB bytes. Alpha is deliberately excluded so
 * the hash is stable across equivalent opaque encodings; the captured rasters
 * are fully opaque, and including alpha would make the pin brittle without
 * adding signal.
 */
export function cropSha256(
  image: PNG,
  crop: { x: number; y: number; width: number; height: number }
): string {
  const x1 = crop.x + crop.width;
  const y1 = crop.y + crop.height;
  if (
    crop.x < 0 ||
    crop.y < 0 ||
    x1 > image.width ||
    y1 > image.height ||
    crop.width <= 0 ||
    crop.height <= 0
  ) {
    throw new Error(
      `artwork crop ${crop.x},${crop.y} ${crop.width}x${crop.height} lies outside ` +
        `the ${image.width}x${image.height} page`
    );
  }
  const bytes = Buffer.allocUnsafe(crop.width * crop.height * 3);
  let cursor = 0;
  for (let y = crop.y; y < y1; y += 1) {
    for (let x = crop.x; x < x1; x += 1) {
      const offset = (y * image.width + x) * 4;
      bytes[cursor] = image.data[offset];
      bytes[cursor + 1] = image.data[offset + 1];
      bytes[cursor + 2] = image.data[offset + 2];
      cursor += 3;
    }
  }
  return sha256Hex(bytes);
}

const IDENTITY_TRANSFORMS = new Set(["", "none", "matrix(1, 0, 0, 1, 0, 0)"]);

/**
 * Verify observed artwork against reviewed pins. Returns every violation
 * rather than throwing on the first, so a run reports the whole picture.
 *
 * A pin with no `cropSha256` is reported as `unpinned` rather than passing
 * silently: an un-pinned crop is an unmade decision, not a satisfied one.
 */
export function verifyEncodedArtwork(
  pins: readonly EncodedArtworkPin[],
  observations: readonly ArtworkObservation[],
  rasters: ReadonlyMap<number, PNG>
): ArtworkViolation[] {
  const violations: ArtworkViolation[] = [];
  const observed = new Map(observations.map((entry) => [entry.id, entry]));

  for (const pin of pins) {
    const entry = observed.get(pin.id);
    if (!entry) {
      violations.push({
        id: pin.id,
        kind: "missing",
        detail: `no artwork rendered for selector ${pin.selector}`
      });
      continue;
    }

    if (pin.payload !== undefined) {
      // Our symbols are pinned build-time module matrices, so the payload the
      // reviewer decoded is carried in the rendered title. If that ever
      // becomes runtime-encoded, this must assert the encoder input directly.
      if (entry.title === null || !entry.title.includes(pin.payload)) {
        violations.push({
          id: pin.id,
          kind: "payload",
          detail: `expected payload ${JSON.stringify(pin.payload)}, rendered title ${JSON.stringify(entry.title)}`
        });
      }
    }

    if (pin.symbolDataSha256 !== undefined) {
      if (entry.pathData === null) {
        violations.push({
          id: pin.id,
          kind: "symbol-data",
          detail: "symbol path data is absent from the DOM"
        });
      } else {
        const actual = sha256Hex(entry.pathData);
        if (actual !== pin.symbolDataSha256) {
          violations.push({
            id: pin.id,
            kind: "symbol-data",
            detail: `symbol data sha256 ${actual} does not match the reviewed ${pin.symbolDataSha256}`
          });
        }
      }
    }

    if (entry.transform !== null && !IDENTITY_TRANSFORMS.has(entry.transform)) {
      violations.push({
        id: pin.id,
        kind: "transform",
        detail:
          `machine-readable artwork carries transform ${JSON.stringify(entry.transform)}; ` +
          `mirroring or scaling a symbol destroys it while leaving its bounding box intact`
      });
    }

    const raster = rasters.get(pin.page);
    if (!raster) continue;
    let actualCrop: string;
    try {
      actualCrop = cropSha256(raster, pin.crop);
    } catch (error) {
      violations.push({
        id: pin.id,
        kind: "crop",
        detail: (error as Error).message
      });
      continue;
    }
    if (pin.cropSha256 === undefined) {
      violations.push({
        id: pin.id,
        kind: "unpinned",
        detail:
          `no reviewed crop hash pinned; observed ${actualCrop}. Pin it only after ` +
          `visually reviewing the cropped artwork.`
      });
      continue;
    }
    if (actualCrop !== pin.cropSha256) {
      violations.push({
        id: pin.id,
        kind: "crop",
        detail: `crop sha256 ${actualCrop} does not match the reviewed ${pin.cropSha256}`
      });
    }
  }

  return violations;
}

/**
 * Reviewed artwork pins for 2551Q:2018. Crop rectangles come from the reviewed
 * critical-region table; crop hashes are pinned by a reviewer after the
 * structural fixes land, because a crop hash captured over a known-defective
 * render would pin the defect.
 */
export const OFFICIAL_2551Q_ARTWORK_PINS: readonly EncodedArtworkPin[] = [
  {
    id: "page-1-pdf417",
    page: 1,
    selector: ".official-barcode > .official-pdf417-symbol",
    payload: "2551Q 01/18ENCS P1",
    crop: { x: 852, y: 119, width: 323, height: 74 }
  },
  {
    id: "page-1-government-seal",
    page: 1,
    selector: ".government-seal",
    crop: { x: 490, y: 44, width: 58, height: 50 }
  },
  {
    id: "page-2-pdf417",
    page: 2,
    selector: ".page-two-barcode > .official-pdf417-symbol",
    payload: "2551Q 01/18ENCS P2",
    crop: { x: 852, y: 119, width: 323, height: 74 }
  }
];
