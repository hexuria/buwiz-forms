// What the agent host asks of the frame, and how each ask is applied: the old app's `applyAgent` (app/main.js), one command at a
// time. The Rust side (`bir_app_api::agent::session`) queues them; `bir.agent_take` hands them over as JSON text. No nsh import and
// no frame import: the frame's calls come in as `FrameDeps`, so this runs in a pure test.

/** A command from the agent host. Anything else is ignored, as the old app ignored it. */
export type AgentCommand =
  /** The data changed behind the app's back: read it again, and show this taxpayer (quietly). */
  | { op: "refresh"; tinRoot: string | null }
  /** Show a view of the frame (the nav tools: home, library, calendar, filings, profile, settings). */
  | { op: "go"; view: string }
  /** Open a return on the form page (the form tools). */
  | { op: "open"; tin: string; code: string; year: number; period: number }
  /** The return is prepared for the person: say so, and lead with Queue. The person queues it, not the agent. */
  | { op: "review"; message: string }
  /** Boxes the agent filled, to show on the open form: committed as a choice, in the order the agent wrote them. */
  | { op: "choose"; edits: Record<string, string> }
  /** The agent asked the app to shut down. */
  | { op: "quit" };

/** The sentence a `review` command carries when the agent gave none (the old app's). */
export const REVIEW_MESSAGE = "Prepared by the assistant. Review every box, then click Queue yourself.";

/** Which return the form page must be showing. */
export interface ReturnRef {
  tin: string;
  formCode: string;
  year: number;
  period: number;
}

/** What the frame does for a command. */
export interface FrameDeps {
  /** Show a view by name (unknown names are the frame's to ignore). */
  go(view: string): void;
  /** Open a return on the form page. */
  openReturn(open: { tin: string; code: string; year: number; period: number }): void;
  /** Read the data again; when a taxpayer root is given, select it without a toast. */
  refresh(tinRoot: string | null): void;
  toast(message: string): void;
  /** Mark the open return as prepared for review. */
  review(message: string): void;
  /** Resolves true once the page of this return is open and loaded. */
  whenReady(ref: ReturnRef): Promise<boolean>;
  /** Commit edits on the open page; false when no page can take them. */
  choose(edits: Record<string, string>): boolean;
  /** The app's quit path (leave hooks first). */
  quit(): Promise<void>;
}

/** What the bridge remembers between commands. */
export interface AgentState {
  /** The return the last `open` named: the fill and the review land on its page, and wait for it. */
  form: ReturnRef | null;
}

function text(value: unknown): string {
  return typeof value === "string" ? value : String(value ?? "");
}

/** The command in `raw` (the JSON text `agent_take` answers, or the object), or null when it is not one the frame knows. */
export function parseCommand(raw: unknown): AgentCommand | null {
  let value: unknown = raw;
  if (typeof raw === "string") {
    try {
      value = JSON.parse(raw);
    } catch {
      return null;
    }
  }
  if (!value || typeof value !== "object") return null;
  const cmd = value as Record<string, unknown>;
  switch (cmd.op) {
    case "refresh":
      return { op: "refresh", tinRoot: cmd.tin_root ? text(cmd.tin_root) : null };
    case "go":
      return cmd.view ? { op: "go", view: text(cmd.view) } : null;
    case "open":
      return cmd.tin && cmd.code
        ? { op: "open", tin: text(cmd.tin), code: text(cmd.code), year: Number(cmd.year), period: Number(cmd.period || 0) }
        : null;
    case "review":
      return { op: "review", message: cmd.message ? text(cmd.message) : REVIEW_MESSAGE };
    case "choose": {
      if (!cmd.edits || typeof cmd.edits !== "object") return null;
      const edits: Record<string, string> = {};
      for (const [key, v] of Object.entries(cmd.edits as Record<string, unknown>)) edits[key] = text(v);
      return { op: "choose", edits };
    }
    case "quit":
      return { op: "quit" };
    default:
      return null;
  }
}

/** The page of the return the agent opened must be up before its fill or its review lands (the old app's `afterFormLoads`). */
async function onOpenForm(deps: FrameDeps, state: AgentState, then: () => void): Promise<void> {
  // No return was opened by the agent: its fill is already in the database, and the person's own page loads it.
  if (!state.form) return;
  if (await deps.whenReady(state.form)) then();
}

/** Applies one command to the frame. A fill and a review wait for their page; the bridge runs one command at a time. */
export async function applyCommand(cmd: AgentCommand, deps: FrameDeps, state: AgentState): Promise<void> {
  switch (cmd.op) {
    case "refresh":
      deps.refresh(cmd.tinRoot);
      return;
    case "go":
      deps.go(cmd.view);
      return;
    case "open":
      state.form = { tin: cmd.tin, formCode: cmd.code, year: cmd.year, period: cmd.period };
      deps.openReturn({ tin: cmd.tin, code: cmd.code, year: cmd.year, period: cmd.period });
      return;
    case "review":
      await onOpenForm(deps, state, () => {
        deps.review(cmd.message);
        deps.toast(cmd.message);
      });
      return;
    case "choose":
      await onOpenForm(deps, state, () => void deps.choose(cmd.edits));
      return;
    case "quit":
      await deps.quit();
      return;
  }
}
