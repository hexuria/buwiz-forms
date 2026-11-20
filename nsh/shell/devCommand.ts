import { devCommand, doorStatus } from "../bir";
import { errorText } from "../lib/errors";
import { startPolls } from "../lib/poll";
import { app } from "./app";
import { store } from "./store";

export function dispatchDevCommand(command: string | null): void {
  if (command === "lock") app.lockApp();
  else if (command === "logout") app.logOut();
}

interface Mailbox {
  enabled: () => boolean;
  read: () => string | null;
  schedule: typeof startPolls;
}

/** Owned by Shell, not focus, visibility, session state or the ordinary polling preference. */
export function startDevCommandMailbox(mailbox: Mailbox = {
  enabled: () => doorStatus().devCommandEnabled,
  read: devCommand,
  schedule: startPolls,
}): () => void {
  if (!mailbox.enabled()) return () => {};
  return mailbox.schedule(
    [{ name: "dev-command", every: 500, run: () => dispatchDevCommand(mailbox.read()) }],
    (name, error) => store.logScreen("warn", `${name}: ${errorText(error)}`),
  );
}
