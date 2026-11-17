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
    { id: "import", search: "import zip restore database", adminOnly: true },
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

/** The label for a stored chord. Empty is Off; an unknown chord is shown as stored. */
export function hotkeyLabel(combo: string | null | undefined, presets?: readonly SettingsPreset[] | null): string {
  const value = String(combo || "").trim();
  if (!value) return "Off";
  const found = (presets || []).find((preset) => preset.value === value);
  return found ? found.label : value;
}

/** The options for the hotkey pill: Off first, the presets, and a custom chord kept at the end. */
export function hotkeyOptions(combo: string | null | undefined, presets?: readonly SettingsPreset[] | null): SettingsPreset[] {
  const options: SettingsPreset[] = [{ value: "", label: "Off" }, ...(presets || [])];
  const value = String(combo || "").trim();
  if (value && !options.some((option) => option.value === value)) {
    options.push({ value, label: value });
  }
  return options;
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
