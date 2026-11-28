// Headless-test fixture for DebugLockPages; host.params names the state.
import { host, mount } from "nsh";
import "../../theme/fonts";
import { DebugLockPages, type DebugLockParams } from "./DebugLockPages";

mount(() => {
  const p = (host.params || {}) as DebugLockParams;
  return <DebugLockPages {...p} />;
});
