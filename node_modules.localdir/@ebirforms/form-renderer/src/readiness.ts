export type GeometryStabilityDecision = "ready" | "retry" | "timed_out";

/** Require two identical consecutive page measurements before native readiness. */
export function geometryStabilityDecision(
  previousSignature: string | undefined,
  currentSignature: string,
  deadlineReached: boolean
): GeometryStabilityDecision {
  if (previousSignature === currentSignature) return "ready";
  return deadlineReached ? "timed_out" : "retry";
}
