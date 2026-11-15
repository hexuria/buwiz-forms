// The entry. NSH mounts the root once; the root is the frame (shell/Shell.tsx), which draws every screen.
// BIR_SHELL_START opens a screen at launch for screenshots: "tp=<tin root>;theme=dark;view=library;form=2551Q/2026/3".
// The polls (events, device filings, update status) run in the shipped app and in a mock run (EBIR_TEST_ENV unset); in the
// dev environment (nsh run, every test and shots recipe) they wait for BIR_SHELL_POLL=1.
import { mount, View } from "nsh";
import "./theme/fonts";
import { doorStatus } from "./bir";
import { DesktopHost } from "./shell/desktop";
import { Shell } from "./shell/Shell";
// The print preview window: listens to the Rust side and opens the second window (screens/print/host.tsx).
import "./screens/print/host";

function Root() {
  const polling = !doorStatus().testEnvironment || process.env.BIR_SHELL_POLL === "1";
  // The desktop host (menu, tray, hotkey, updater) draws nothing but the About dialog; it sits beside the frame.
  return (
    <View class="flex-1">
      <Shell start={process.env.BIR_SHELL_START} polling={polling} firstRun agent />
      <DesktopHost />
    </View>
  );
}

mount(Root);
