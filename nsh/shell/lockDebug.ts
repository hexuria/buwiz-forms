// The compile-time gate of the debug lock-page route: Rust links Desktop.lockPagesDebug only with debug_assertions.
// In a release host the method itself is absent, so this is false rather than merely a hidden or refused route.
import { Desktop } from "ebirforms/native";

export function lockPagesDebugCompiled(): boolean {
  const probe = (Desktop as unknown as { lockPagesDebug?: () => boolean }).lockPagesDebug;
  return typeof probe === "function" && probe.call(Desktop) === true;
}
