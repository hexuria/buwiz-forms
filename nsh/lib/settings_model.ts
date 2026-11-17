// What Settings can show, and which of it a search keeps (settings_model.js). Pure data: the screen draws a row only
// when its id is in the result, so a control that is not wired never appears here. Search is the ChatGPT kind: a
// section name keeps every row of that section; otherwise only the rows whose own words match. Every word of the
// query has to match.

/** A Settings section: its id, its label and icon, its group, whether only an admin sees it, and its search words. */
export interface SettingsSection {
  id: string;
  label: string;
  icon: string;
  group: string;
  adminOnly: boolean;
  search: string;
}

/** A row of a section: its id, its search words, and whether only an admin sees it. */
export interface SettingsRow {
  id: string;
  search: string;
  adminOnly?: boolean;
}

/** A section as the search shows it: its rows (ids) that the search keeps. */
export interface SettingsViewSection {
  id: string;
  label: string;
  icon: string;
  group: string;
  rows: string[];
}

/** A choice in a pill: its stored value and the label shown. */
export interface SettingsPreset {
  value: string;
  label: string;
}

/** The theme choices. */
export interface ThemeOption {
  value: "system" | "light" | "dark";
  label: string;
}

export const SETTINGS_SECTIONS: readonly SettingsSection[] = [
  { id: "general", label: "General", icon: "settings", group: "Personal", adminOnly: false, search: "general personal appearance" },
  { id: "account", label: "Account", icon: "user", group: "Personal", adminOnly: false, search: "account personal profile" },
  { id: "transfer", label: "Import / Export", icon: "archive", group: "Data", adminOnly: false, search: "import export data transfer" },
  { id: "jobs", label: "Jobs", icon: "list", group: "Data", adminOnly: true, search: "jobs data background tasks" },
  { id: "computer", label: "Computer", icon: "laptop", group: "System", adminOnly: true, search: "computer system this computer" },
];

export const SETTINGS_ROWS: Readonly<Record<string, readonly SettingsRow[]>> = {
  general: [
    { id: "theme", search: "theme appearance light dark system" },
    { id: "recent", search: "recent returns sidebar" },
    { id: "compact", search: "compact form sections" },
    { id: "updates", search: "download updates automatically release install" },
  ],
  account: [
    { id: "buwiz", search: "buwiz server cloud sign in sign out device" },
    { id: "identity", search: "account name email tin profile sign out add account taxpayer" },
    { id: "google", search: "google gmail connect disconnect oauth inbox tracking mail" },
    { id: "security", search: "pin authenticator recovery key security" },
  ],
  transfer: [
    { id: "import", search: "import zip restore database ebirforms bir official old program folder migrate filed returns", adminOnly: true },
    { id: "export", search: "export profile archive backup zip" },
  ],
  jobs: [{ id: "jobs", search: "jobs background tasks queue logs confirmation watcher cron archive" }],
  computer: [
    { id: "data", search: "database data location path local files" },
    { id: "logs", search: "logs folder retention log files" },
    { id: "hotkey", search: "hotkey shortcut global show keyboard" },
    { id: "tray", search: "tray menu bar close window hide quit" },
    { id: "setup", search: "setup owner help desk who this computer is for" },
    { id: "version", search: "version about ebirforms" },
  ],
};

export const THEME_OPTIONS: readonly ThemeOption[] = [
  { value: "system", label: "System" },
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
];

/** The lower-case words of a query, split on spaces. */
export function queryWords(query: string | null | undefined): string[] {
  return String(query || "")
    .trim()
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean);
}

/** Every word of query occurs in blob. An empty query matches. */
export function hits(query: string | null | undefined, blob: string | null | undefined): boolean {
  const words = queryWords(query);
  if (!words.length) return true;
  const hay = String(blob || "").toLowerCase();
  return words.every((word) => hay.includes(word));
}

/** The rows a search keeps in a section: all of them when the query names the section, else those whose own words match. */
function visibleRows(query: string | null | undefined, sectionSearch: string, rows: readonly SettingsRow[]): SettingsRow[] {
  if (!queryWords(query).length || hits(query, sectionSearch)) return [...rows];
  return rows.filter((row) => hits(query, row.search));
}

/**
 * The sections and rows the search keeps for this role. isAdmin is the signed-in admin (or an unlocked computer with no PIN).
 */
export function settingsView(query: string | null | undefined, isAdmin: boolean): SettingsViewSection[] {
  const out: SettingsViewSection[] = [];
  for (const section of SETTINGS_SECTIONS) {
    if (section.adminOnly && !isAdmin) continue;
    const rows = (SETTINGS_ROWS[section.id] || []).filter((row) => isAdmin || !row.adminOnly);
    const shown = visibleRows(query, section.search, rows);
    if (shown.length) {
      out.push({
        id: section.id,
        label: section.label,
        icon: section.icon,
        group: section.group,
        rows: shown.map((row) => row.id),
      });
    }
  }
  return out;
}

/** The section to draw: the one selected, or else the first the search still shows (null when none). */
export function shownSection(selected: string | null | undefined, view: readonly { id: string }[]): string | null {
  if (selected != null && view.some((section) => section.id === selected)) return selected;
  return view.length ? view[0].id : null;
}

/** The section with this id in the view, or null. */
export function sectionById<T extends { id: string }>(view: readonly T[], id: string): T | null {
  return view.find((section) => section.id === id) || null;
}

/** Whether row rowId is drawn in the section. */
export function rowShown(section: { rows: readonly string[] } | null | undefined, rowId: string): boolean {
  return Boolean(section && section.rows.includes(rowId));
}

// ---- the global show hotkey: the stored combo language and the keystrokes the shortcut recorder speaks

/**
 * The stored combo is words joined by "+", the key last and upper case ("control+alt+E", "super+shift+F12", "control+alt+SPACE": a key that is
 * not a letter, digit or F1 to F12 is a word, never the character). Modifier words the listeners read:
 * control|ctrl, alt|option|opt, shift, platform|super|cmd|meta|win (Command, the Windows key or Super). A bare key means the platform's
 * default modifiers: control+alt on macOS, super+shift elsewhere (crates/bir-app-api/src/hotkey_combo.rs split_combo). A keystroke is
 * nsh/keymap's: "ctrl+alt+e", modifiers in the order ctrl, alt, shift, cmd, the key lower case.
 */

/** The keys a stored combo may name that the old app's listener registers too: A to Z, 0 to 9, F1 to F12 (hotkey_combo.rs KEYS), upper case. */
export const HOTKEY_KEYS: readonly string[] = [
  ..."ABCDEFGHIJKLMNOPQRSTUVWXYZ".split(""),
  ..."0123456789".split(""),
  ...Array.from({ length: 12 }, (_, i) => `F${i + 1}`),
];

/**
 * The keys only this app registers (hotkey_combo.rs NSH_KEYS): the stored word (upper case: the character is never stored, "+" is the
 * separator) and the key as nsh/keymap spells it (src/desktop.rs key_name is the same table). The old app's listener cannot register
 * them and its set_global_hotkey refuses them; stored by this app and met by the old one, a combo is simply not registered there.
 */
export const HOTKEY_NSH_KEYS: Readonly<Record<string, string>> = {
  SPACE: "space",
  UP: "up",
  DOWN: "down",
  LEFT: "left",
  RIGHT: "right",
  HOME: "home",
  END: "end",
  PAGEUP: "pageup",
  PAGEDOWN: "pagedown",
  MINUS: "minus",
  EQUAL: "plus",
  LEFTBRACKET: "[",
  RIGHTBRACKET: "]",
  BACKSLASH: "\\",
  SEMICOLON: ";",
  QUOTE: "'",
  COMMA: ",",
  PERIOD: ".",
  SLASH: "/",
  BACKTICK: "`",
  ...Object.fromEntries(Array.from({ length: 8 }, (_, i) => [`F${i + 13}`, `f${i + 13}`])),
};

/** Other spellings nsh/keymap reads for the same key (a keystroke written by hand or by another version): they mean the stored word. */
const KEY_ALIASES: Readonly<Record<string, string>> = {
  "-": "MINUS",
  "=": "EQUAL",
  arrowup: "UP",
  arrowdown: "DOWN",
  arrowleft: "LEFT",
  arrowright: "RIGHT",
};

/** The stored word for a key as nsh/keymap spells it (lower case), or null when no listener of ours registers it. */
function wordOfKey(key: string): string | null {
  const upper = key.toUpperCase();
  if (HOTKEY_KEYS.includes(upper)) return upper;
  const alias = KEY_ALIASES[key];
  if (alias) return alias;
  return Object.keys(HOTKEY_NSH_KEYS).find((word) => HOTKEY_NSH_KEYS[word] === key) ?? null;
}

/** The key as nsh/keymap spells it (lower case) for a stored word (upper case), or null when it is no key of ours. */
function keyOfWord(word: string): string | null {
  if (HOTKEY_KEYS.includes(word)) return word.toLowerCase();
  return HOTKEY_NSH_KEYS[word] ?? null;
}

type Modifier = "ctrl" | "alt" | "shift" | "cmd";
/** The modifiers in the order both languages write them. */
const ORDER: readonly Modifier[] = ["ctrl", "alt", "shift", "cmd"];
/** What a stored combo's words mean (lower case); any other word is ignored, as the listeners ignore it. */
const STORED_WORDS: Readonly<Record<string, Modifier>> = {
  control: "ctrl", ctrl: "ctrl", alt: "alt", option: "alt", opt: "alt", shift: "shift", platform: "cmd", super: "cmd", cmd: "cmd", meta: "cmd", win: "cmd",
};
/** What a keystroke's modifier words mean (nsh/keymap's; "mod" depends on the platform, so it is none of them). */
const KEYSTROKE_WORDS: Readonly<Record<string, Modifier>> = {
  ctrl: "ctrl", control: "ctrl", alt: "alt", option: "alt", opt: "alt", shift: "shift", cmd: "cmd", command: "cmd", meta: "cmd", super: "cmd", win: "cmd",
};
/** The word a stored combo uses for each modifier. */
const STORED: Readonly<Record<Modifier, string>> = { ctrl: "control", alt: "alt", shift: "shift", cmd: "super" };

/** The stored combo's parts (empty ones dropped), and its key upper case; null when it names no key this app registers. */
function storedParts(combo: string | null | undefined): { words: string[]; key: string } | null {
  const words = String(combo ?? "").split("+").map((part) => part.trim()).filter(Boolean);
  const key = (words.pop() ?? "").toUpperCase();
  return keyOfWord(key) !== null ? { words, key } : null;
}

/**
 * The keystroke a stored combo is: "control+alt+E" is "ctrl+alt+e", "super+shift+F12" is "shift+cmd+f12". A bare stored key takes the
 * default modifiers of the computer the host runs on (`os`: system_info().os, "macos" or another), as hotkey_combo::split_combo does;
 * a word that is no modifier is ignored. Null for an empty combo or a key the listeners cannot take.
 */
export function comboToKeystroke(combo: string | null | undefined, os: string): string | null {
  const parts = storedParts(combo);
  if (!parts) return null;
  const words = parts.words.length ? parts.words : os === "macos" ? ["control", "alt"] : ["platform", "shift"];
  const on = new Set(words.map((word) => STORED_WORDS[word.toLowerCase()]).filter(Boolean));
  return [...ORDER.filter((m) => on.has(m)), keyOfWord(parts.key)].join("+");
}

/**
 * The stored combo a keystroke is: "ctrl+alt+e" is "control+alt+E", "ctrl+alt+space" is "control+alt+SPACE", "cmd+minus" is "super+MINUS",
 * cmd (or super, win, meta) is "super". Null for a keystroke that names a key no listener of ours registers (HOTKEY_KEYS and
 * HOTKEY_NSH_KEYS), a modifier word it cannot read (mod), or no modifier at all (a bare stored key would mean the platform's default
 * modifiers, not the bare key).
 */
export function keystrokeToCombo(keystroke: string | null | undefined): string | null {
  const words = String(keystroke ?? "").split("+").map((part) => part.trim().toLowerCase()).filter(Boolean);
  const key = words.length ? wordOfKey(words.pop() as string) : null;
  if (key === null || !words.length) return null;
  const on = new Set<Modifier>();
  for (const word of words) {
    const modifier = KEYSTROKE_WORDS[word];
    if (!modifier) return null;
    on.add(modifier);
  }
  return [...ORDER.filter((m) => on.has(m)).map((m) => STORED[m]), key].join("+");
}

/** Whether the door takes this keystroke as the global hotkey: a key of HOTKEY_KEYS or HOTKEY_NSH_KEYS with Control, Alt or Command (Shift alone, or a bare key, is refused with hotkey_combo::NOT_REGISTRABLE). */
export function hotkeyRegistrable(keystroke: string | null | undefined): boolean {
  const combo = keystrokeToCombo(keystroke);
  if (combo === null) return false;
  return combo.split("+").some((word) => word === "control" || word === "alt" || word === "super");
}

/** What to say when a chord has no Control, Alt or Command, in the platform's words (`os`: platform.os; iOS follows macOS). */
export function modifierWords(os: string): string {
  if (os === "macos" || os === "ios") return "Add Control, Option or Command.";
  if (os === "windows") return "Add Ctrl, Alt or the Windows key.";
  return "Add Ctrl, Alt or Super.";
}

/** Where the host's global hotkey stands (shell/desktop/hotkey.ts), in words for under the box; empty when there is nothing to say. */
export function hotkeyProblem(status: string): string {
  if (status === "refused") return "Another app is using this shortcut. Pick another one.";
  if (status === "invalid") return "This computer cannot register that shortcut.";
  return "";
}

/** How long the log is kept, in words. Zero, a negative or an unreadable count is the seven-day default. */
export function retentionLabel(days: unknown): string {
  const n = Number(days);
  const count = Number.isFinite(n) && n > 0 ? n : 7;
  return count === 1 ? "1 day" : `${count} days`;
}

/** The account a Settings card names: a taxpayer, or the computer's local account. */
export interface AccountTaxpayer {
  tin_root: string;
  name?: string | null;
  tin_display?: string | null;
}

/** The branch whose mailbox the card shows: its TIN, name and facts (its email). */
export interface AccountBranch {
  tin: string;
  name?: string | null;
  facts?: { email?: unknown } | null;
}

/** What accountSummary is given. */
export interface AccountSummaryInput {
  taxpayers?: readonly AccountTaxpayer[] | null;
  branch?: AccountBranch | null;
  isAdmin?: boolean;
  isHelpdesk?: boolean;
  adminRoot?: string | null;
  sessionKind?: string | null;
  sessionRoot?: string | null;
  canAddTaxpayer?: boolean;
}

/** The account card: who is signed in, the mailbox, and whether a taxpayer can be added. */
export interface AccountSummary {
  name: string;
  detail: string;
  email: string;
  mailbox: { tin: string; label: string; email: string } | null;
  showAdd: boolean;
}

/**
 * The account card. Email is the signed-in person's own branch email. A help desk admin is not that taxpayer, so
 * their card has no email; the selected profile's mailbox is separate (mailbox).
 */
export function accountSummary(input: AccountSummaryInput): AccountSummary {
  const taxpayers = input.taxpayers || [];
  const own = (root: string | null | undefined) => taxpayers.find((t) => t.tin_root === root) || null;
  const branch = input.branch || null;
  const emailOf = (b: AccountBranch | null) => (b && b.facts && b.facts.email ? String(b.facts.email) : "");
  let name = "Local account";
  let detail = "This computer";
  let email = "";
  if (input.isAdmin && input.isHelpdesk) {
    name = "Help desk admin";
    detail = "TIN 000-000-000";
  } else if (input.isAdmin) {
    const owner = own(input.adminRoot);
    name = (owner && owner.name) || name;
    detail = "Admin · this computer";
    email = emailOf(branch);
  } else if (input.sessionKind === "taxpayer") {
    const taxpayer = own(input.sessionRoot);
    name = (taxpayer && taxpayer.name) || name;
    detail = taxpayer && taxpayer.tin_display ? `TIN ${taxpayer.tin_display}` : detail;
    email = emailOf(branch);
  }
  return {
    name,
    detail,
    email,
    mailbox: branch
      ? {
          tin: branch.tin,
          label: branch.name || name,
          email: emailOf(branch),
        }
      : null,
    showAdd: Boolean(input.canAddTaxpayer),
  };
}
