// DOM client/scroll metrics are integer-rounded independently. A one-pixel
// delta is therefore normal at fractional point sizes; anything above this
// threshold is real layout overflow.
const GEOMETRY_TOLERANCE_PX = 1.25;

export interface RendererPageMeasurement {
  x: number;
  y: number;
  width: number;
  height: number;
  client_width: number;
  client_height: number;
  scroll_width: number;
  scroll_height: number;
  descendant_overflow_x: number;
  descendant_overflow_y: number;
  descendant_clipped_x: number;
  descendant_clipped_y: number;
}

export interface RendererGeometryMeasurement {
  type: "page_count";
  page_count: number;
  page_width_pt: number;
  page_height_pt: number;
  pages: RendererPageMeasurement[];
}

/**
 * Measure every printable page and fail-closed geometry signal used by the
 * native host. Root scroll dimensions alone are insufficient: an
 * `overflow:hidden` descendant can silently crop text while the page itself
 * still reports perfectly matching client and scroll dimensions.
 */
export function measureRenderedPages(
  root: ParentNode = document
): RendererGeometryMeasurement | null {
  const pages = Array.from(root.querySelectorAll<HTMLElement>(".form-page"));
  const firstPage = pages[0];
  if (!firstPage) return null;

  const pageBounds = firstPage.getBoundingClientRect();
  if (pageBounds.width <= 0 || pageBounds.height <= 0) return null;

  return {
    type: "page_count",
    page_count: pages.length,
    page_width_pt: pageBounds.width * 72 / 96,
    page_height_pt: pageBounds.height * 72 / 96,
    pages: pages.map(measurePage)
  };
}

export function rendererGeometryIsSafe(
  measurement: RendererGeometryMeasurement
): boolean {
  return measurement.pages.every((page) =>
    page.client_width > 0 &&
    page.client_height > 0 &&
    page.scroll_width <= page.client_width + GEOMETRY_TOLERANCE_PX &&
    page.scroll_height <= page.client_height + GEOMETRY_TOLERANCE_PX &&
    page.descendant_overflow_x === 0 &&
    page.descendant_overflow_y === 0 &&
    page.descendant_clipped_x === 0 &&
    page.descendant_clipped_y === 0
  );
}

function measurePage(page: HTMLElement): RendererPageMeasurement {
  const bounds = page.getBoundingClientRect();
  let descendantOverflowX = 0;
  let descendantOverflowY = 0;
  let descendantClippedX = 0;
  let descendantClippedY = 0;

  for (const descendant of page.querySelectorAll<HTMLElement>("*")) {
    const descendantBounds = descendant.getBoundingClientRect();
    if (descendantBounds.width <= 0 && descendantBounds.height <= 0) continue;

    if (descendant.scrollWidth > descendant.clientWidth + GEOMETRY_TOLERANCE_PX) {
      descendantOverflowX += 1;
    }
    if (descendant.scrollHeight > descendant.clientHeight + GEOMETRY_TOLERANCE_PX) {
      descendantOverflowY += 1;
    }

    const clipping = clippingAxes(descendant, page, descendantBounds);
    if (clipping.x) descendantClippedX += 1;
    if (clipping.y) descendantClippedY += 1;
  }

  return {
    x: bounds.left + window.scrollX,
    y: bounds.top + window.scrollY,
    width: bounds.width,
    height: bounds.height,
    client_width: page.clientWidth,
    client_height: page.clientHeight,
    scroll_width: page.scrollWidth,
    scroll_height: page.scrollHeight,
    descendant_overflow_x: descendantOverflowX,
    descendant_overflow_y: descendantOverflowY,
    descendant_clipped_x: descendantClippedX,
    descendant_clipped_y: descendantClippedY
  };
}

function clippingAxes(
  descendant: HTMLElement,
  page: HTMLElement,
  descendantBounds: DOMRect
): { x: boolean; y: boolean } {
  let clippedX = false;
  let clippedY = false;
  let ancestor = descendant.parentElement;

  while (ancestor) {
    const style = window.getComputedStyle(ancestor);
    const clipsX = clipsOverflow(style.overflowX);
    const clipsY = clipsOverflow(style.overflowY);
    if (clipsX || clipsY) {
      const bounds = ancestor.getBoundingClientRect();
      const left = bounds.left + ancestor.clientLeft;
      const top = bounds.top + ancestor.clientTop;
      const right = left + ancestor.clientWidth;
      const bottom = top + ancestor.clientHeight;

      clippedX ||= clipsX && (
        descendantBounds.left < left - GEOMETRY_TOLERANCE_PX ||
        descendantBounds.right > right + GEOMETRY_TOLERANCE_PX
      );
      clippedY ||= clipsY && (
        descendantBounds.top < top - GEOMETRY_TOLERANCE_PX ||
        descendantBounds.bottom > bottom + GEOMETRY_TOLERANCE_PX
      );
    }

    if (ancestor === page || (clippedX && clippedY)) break;
    ancestor = ancestor.parentElement;
  }

  return { x: clippedX, y: clippedY };
}

function clipsOverflow(value: string): boolean {
  return value === "hidden" || value === "clip" || value === "scroll" || value === "auto";
}
