export const RELEASE_VISUAL_MAX_CHANGED_PERCENT = 1;

export function parsePromotionVisualThreshold(rawValue: string | undefined): number {
  const value = Number(rawValue ?? String(RELEASE_VISUAL_MAX_CHANGED_PERCENT));
  if (
    !Number.isFinite(value) ||
    value < 0 ||
    value > RELEASE_VISUAL_MAX_CHANGED_PERCENT
  ) {
    throw new Error(
      `FORM_VISUAL_MAX_CHANGED_PERCENT must be between 0 and ${RELEASE_VISUAL_MAX_CHANGED_PERCENT} for promotion evidence`
    );
  }
  return value;
}
