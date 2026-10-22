// What Settings can show, and which of it a search keeps.
//
// Pure data: the screen draws a row only when its id is in the result, so a
// control that is not wired never appears here. Search is the ChatGPT kind:
// a section name keeps every row of that section; otherwise only the rows
// whose own words match. Every word of the query has to match.

/** @typedef {{ id: string, label: string, icon: string, group: string, adminOnly: boolean, search: string }} SettingsSection */
/** @typedef {{ id: string, search: string, adminOnly?: boolean }} SettingsRow */

/** @type {SettingsSection[]} */
export const SETTINGS_SECTIONS = [
  { id: "general", label: "General", icon: "settings", group: "Personal", adminOnly: false, search: "general personal appearance" },
  { id: "account", label: "Account", icon: "user", group: "Personal", adminOnly: false, search: "account personal profile" },
  { id: "transfer", label: "Import / Export", icon: "archive", group: "Data", adminOnly: false, search: "import export data transfer" },
  { id: "jobs", label: "Jobs", icon: "list", group: "Data", adminOnly: true, search: "jobs data background tasks" },
  { id: "computer", label: "Computer", icon: "laptop", group: "System", adminOnly: true, search: "computer system this computer" },
];

/** @type {Record<string, SettingsRow[]>} */
export const SETTINGS_ROWS = {
  general: [
    { id: "theme", search: "theme appearance light dark system" },
    { id: "recent", search: "recent returns sidebar" },
    { id: "compact", search: "compact form sections" },
  ],
  account: [
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

export const THEME_OPTIONS = [
  { value: "system", label: "System" },
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
];

/** @param {string} query */
export function queryWords(query) {
  return String(query || "")
    .trim()
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean);
}

/** Every word of `query` occurs in `blob`. An empty query matches. */
export function hits(query, blob) {
  const words = queryWords(query);
  if (!words.length) return true;
  const hay = String(blob || "").toLowerCase();
  return words.every((word) => hay.includes(word));
}

/**
 * Sections and rows the search keeps for this role.
 * `isAdmin` is the signed-in admin (or an unlocked computer with no PIN).
 * @returns {{ id: string, label: string, icon: string, group: string, rows: string[] }[]}
 */
export function settingsView(query, isAdmin) {
  const out = [];
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

function visibleRows(query, sectionSearch, rows) {
  if (!queryWords(query).length || hits(query, sectionSearch)) return rows;
  return rows.filter((row) => hits(query, row.search));
}

/** The section to draw: the one selected, or the first the search still shows. */
export function shownSection(selected, view) {
  if (view.some((section) => section.id === selected)) return selected;
  return view.length ? view[0].id : null;
}

export function sectionById(view, id) {
  return view.find((section) => section.id === id) || null;
}

export function rowShown(section, rowId) {
  return Boolean(section && section.rows.includes(rowId));
}

/** Label for a stored chord. Empty is Off; an unknown chord is shown as stored. */
export function hotkeyLabel(combo, presets) {
  const value = String(combo || "").trim();
  if (!value) return "Off";
  const found = (presets || []).find((preset) => preset.value === value);
  return found ? found.label : value;
}

/** Options for the hotkey pill, with Off first and a custom chord kept. */
export function hotkeyOptions(combo, presets) {
  const options = [{ value: "", label: "Off" }, ...(presets || [])];
  const value = String(combo || "").trim();
  if (value && !options.some((option) => option.value === value)) {
    options.push({ value, label: value });
  }
  return options;
}

export function retentionLabel(days) {
  const n = Number(days);
  const count = Number.isFinite(n) && n > 0 ? n : 7;
  return count === 1 ? "1 day" : `${count} days`;
}

/**
 * The account card. Email is the signed-in person's own branch email.
 * A help desk admin is not that taxpayer, so their card has no email;
 * the selected profile's mailbox is separate (`mailbox`).
 */
export function accountSummary(input) {
  const taxpayers = input.taxpayers || [];
  const own = (root) => taxpayers.find((t) => t.tin_root === root) || null;
  const branch = input.branch || null;
  const emailOf = (b) => (b && b.facts && b.facts.email ? String(b.facts.email) : "");
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
