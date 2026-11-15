// The entry. NSH mounts the root once; the root is the frame (shell/Shell.tsx), which draws every screen.
// BIR_SHELL_START opens a screen at launch for screenshots: "tp=<tin root>;theme=dark;view=library;form=2551Q/2026/3".
// The polls (events, device filings, update status) run in the shipped app and in a mock run (EBIR_TEST_ENV unset); in the
// dev environment (nsh run, every test and shots recipe) they wait for BIR_SHELL_POLL=1.
import { mount } from "nsh";
import "./theme/fonts";
import { doorStatus } from "./bir";
import { Shell } from "./shell/Shell";

function Root() {
  const polling = !doorStatus().testEnvironment || process.env.BIR_SHELL_POLL === "1";
  return <Shell start={process.env.BIR_SHELL_START} polling={polling} firstRun />;
}

mount(Root);
