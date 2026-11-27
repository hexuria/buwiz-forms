// The email-and-code sign-in's host calls as a test needs them (AuthDemo.tsx, SettingsDemo.tsx): never the network. What a fixture
// asks for, in host.params:
//   email      "code" (default): Continue sends a code; "refuse": the server's sentence for an address it refuses; "unavailable":
//              the server does not offer it; "offline": no connection
//   expiresIn  seconds the code lives (default 300); resendIn: seconds until Resend unlocks (default: expiresIn)
//   pending    seconds left of a request kept from before a relaunch (the gate opens on the code page). Without it, the request a
//              start made is kept as the host keeps it (cloud_account_email_pending answers it, with what is left of its time)
//   resend     "early": Resend is refused as too early (the server's wait, 60 s); "ended": the request has ended
//   slow       true: Continue, the check, Resend and Send a new code take 4 seconds (the spinner, "Checking…", a double press)
//   blocked    "html": the check and Resend meet a challenge page; "throttled": the outer limit's HTML 429
// The code 246810 signs in; 111111 has expired; 999999 is one too many; 555555 belongs to an ended request; 777777 meets the
// network's limit on code checks and 888888 a busy server (both "wait": no try spent); any other code is wrong,
// with 4, 3, 2, 1 tries left and then too many. The address is the one typed (pending: juana@example.com).
import type { CloudAccountStatus, CloudEmailCheck, CloudEmailStep } from "../../bir";

export interface EmailParams {
  email?: "code" | "refuse" | "unavailable" | "offline" | "throttled";
  expiresIn?: number;
  resendIn?: number;
  pending?: number;
  resend?: "early" | "ended";
  slow?: boolean;
  blocked?: "html" | "throttled";
}

const BLOCKED = {
  html: "Sign-in is not available right now. Try again in a few minutes.",
  throttled: "Too many tries. Wait a moment and try again.",
};

export const SIGNED_IN: CloudAccountStatus = {
  server_url: "http://localhost:3008",
  server_url_source: "build",
  account_url: "",
  signed_in: true,
  email: "juana@example.com",
  user_id: "demo",
  offline: false,
  practice: false,
  detail: "",
};

const REFUSED: Record<string, string> = {
  refuse: "A sign-in code was already sent to this address. You can ask for a new one in 4 minutes.",
  unavailable: "Sign-in is not available right now. Try again in a few minutes.",
  offline: "No connection. Check your internet and try again.",
  throttled: "Too many tries. Wait a moment and try again.",
};

/** Puts the fake email calls on `host` (the bir module); `called` hears each call's name. */
export function fakeEmail(
  host: Record<string, unknown>,
  p: EmailParams,
  called: (name: string) => void,
  signedIn: CloudAccountStatus = SIGNED_IN,
  /** Hears each read of the kept request (cloud_account_email_pending: local, never the server). */
  read: () => void = () => undefined,
): void {
  const life = p.expiresIn ?? 300;
  let address = "";
  let wrong = 0;
  let pending = p.pending !== undefined;
  /** When the kept request's code was sent (ms), 0 for the one of `pending`; its address. */
  let sentAt = 0;
  let kept = "juana@example.com";
  const step = (kind: CloudEmailStep["step"], extra: Partial<CloudEmailStep> = {}): CloudEmailStep => ({
    step: kind,
    email: address,
    code_length: 6,
    expires_in: life,
    resend_in: p.resendIn ?? life,
    message: null,
    retry_after: null,
    ...extra,
  });
  const out = { ...signedIn, signed_in: false, email: null };
  const check = (outcome: CloudEmailCheck["outcome"], message: string | null, attempts: number | null = null): CloudEmailCheck => ({
    outcome,
    message,
    attempts_left: attempts,
    status: outcome === "approved" ? signedIn : out,
  });
  host.cloud_account_email_start = async (email: string) => {
    called("cloud_account_email_start");
    // A moment, so the button's spinner can be seen.
    await new Promise((resolve) => setTimeout(resolve, p.slow ? 4000 : 300));
    const kind = p.email ?? "code";
    // A refusal keeps the request before it (the host replaces it only with one the server accepted).
    if (kind !== "code") return step("refused", { email, message: REFUSED[kind], expires_in: 0, resend_in: 0 });
    address = email;
    wrong = 0;
    pending = true;
    sentAt = Date.now();
    kept = email.trim();
    return step("code");
  };
  host.cloud_account_email_verify = async (code: string) => {
    called(`cloud_account_email_verify ${code}`);
    // A moment, so "Checking…" can be seen.
    await new Promise((resolve) => setTimeout(resolve, p.slow ? 4000 : 150));
    if (p.blocked) return check("error", BLOCKED[p.blocked]);
    if (code === "246810") {
      pending = false;
      return check("approved", null);
    }
    if (code === "777777") return check("wait", "Too many sign-in codes are being checked right now. Try again in 3 minutes.");
    if (code === "888888") return check("wait", "Sign-in is busy right now. Try again in a moment.");
    if (code === "111111") return check("expired", "This code has expired. Send a new code to try again.");
    if (code === "555555") {
      pending = false;
      return check("restart", "This sign-in request has ended. Start again.");
    }
    wrong += 1;
    if (code === "999999" || wrong >= 5) return check("too-many", "Too many wrong codes. Send a new code to try again.");
    const left = 5 - wrong;
    return check("wrong", `That code is not right. ${left} ${left === 1 ? "try" : "tries"} left.`, left);
  };
  host.cloud_account_email_resend = async () => {
    called("cloud_account_email_resend");
    if (p.slow) await new Promise((resolve) => setTimeout(resolve, 4000));
    if (p.blocked) return step("refused", { message: BLOCKED[p.blocked], resend_in: 0 });
    if (p.resend === "early") return step("refused", { message: "A new code can be sent once this one has expired.", resend_in: 60, retry_after: 60 });
    if (p.resend === "ended") {
      pending = false;
      return step("restart", { message: "This sign-in request has ended. Start again." });
    }
    wrong = 0;
    sentAt = Date.now();
    return step("code");
  };
  host.cloud_account_email_cancel = () => {
    called("cloud_account_email_cancel");
    pending = false;
    return null;
  };
  host.cloud_account_email_pending = () => {
    read();
    if (!pending) return null;
    address = kept;
    if (sentAt === 0) {
      const left = p.pending ?? 0;
      return step("code", { expires_in: left, resend_in: Math.min(left, p.resendIn ?? left) });
    }
    const gone = (Date.now() - sentAt) / 1000;
    // The host clears a request whose code has expired.
    if (gone >= life) return null;
    return step("code", { expires_in: Math.ceil(life - gone), resend_in: Math.max(0, Math.ceil((p.resendIn ?? life) - gone)) });
  };
}
