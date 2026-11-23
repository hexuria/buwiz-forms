# lib/

Pure TypeScript helpers ported from the old shell, each with a *.test.ts beside it (nsh test runs them headless; no open()). No nsh import and no host call: the clock and the store come in as parameters. The element builders stay with the screens.

- `todo.ts`: effectiveStops, stopCall, itemKeys, todoRows, compareRows, nextRow (6 exports) from the old shell's todo.js.
- `periods.ts`: frequencyOf, periodOptions, periodLabel (3) from the old shell's periods.js.
- `recent.ts`: recentEntry, recentOpenOptions, recentKey, sameReturn, loadRecent, addRecent, bumpRecent (7) from the old shell's recent.js.
- `facts.ts`: FACT_LABELS, FACT_WHY, ROW_FACTS, incomeTypeChoices, EOPT_TIERS, ELECTION_CHOICES, factValue, electionSourceText, readerChips, joinNames, notSetForForm, notSetSentence (12) from the old shell's facts.js.
- `settings_model.ts`: SETTINGS_SECTIONS, SETTINGS_ROWS, THEME_OPTIONS, queryWords, hits, settingsView, shownSection, sectionById, rowShown, hotkeyLabel, hotkeyOptions, retentionLabel, accountSummary (13) from the old shell's settings_model.js.
- `chrome_gate.ts`: shellChromeVisible (1) from the old shell's chrome_gate.js.
- `dashboard.ts`: loadDashboard, pickedDeadlines, tileSpecs, progressText, nextText (5) from the data half of the old shell's dashboard.js.
- `dates.ts`: MONTHS, MONTHS_LONG, DATE_ERROR, isoDate, addDays, daysUntil, shortDate, shortIsoDate, parseTypedDate, formatMDY, formatLongDate, shownDate, oneDateText (13) from the date helpers of the old shell's store.js and the date parsers and formatters of the old shell's form_page.js.
- `money.ts`: formatMoney, wholePesosOf (2) from the old shell's form_page.js.
- `errors.ts`: errorText (1) from the old shell's store.js.
- `format.ts`: formatTin, displayCode (2) from the old shell's store.js.

Changes from the old code (all else is the same behaviour):
- daysUntil, periodOptions and loadDashboard take an optional clock (the system's by default). loadDashboard takes the reads (DashboardReads) where the old code took app; it never used anything else of app.
- form_page.js's shortDate is shortIsoDate here, because store.js's shortDate is another function with the same name.
- tileSpecs, progressText and nextText were private in dashboard.js; exported because Home draws from them.
- factValue reads atc_codes only from an array (the old code threw on a non-array); itemKeys reads a table with no keys as no keys (the old code threw).

Two formatTin functions, on purpose (different inputs, different answers; each test file pins its own): `lib/format.ts` is the old store.js one, for a STORED TIN (a root string or number, null and undefined allowed, with the branch code given apart): non-digits dropped, a root of 9 or more digits shows its first nine (a 14-digit TIN loses its branch: "00012345300000" is "000-123-453"), a shorter root stays bare digits ("12345" stays "12345", "1234" stays "1234"). `ui/mask.ts` formatTin is the typing one: digits grouped 3-3-3-5 as far as they go ("1234" is "123-4", "12345" is "123-45", "0001234530" is "000-123-453-0", "00012345300000" is "000-123-453-00000"), strings only. They agree only on a 9-digit root, with or without a branch code. Merging them would break one test file or the other.

Not ported here: the Store class; form_page.js dateParts, oneDate, moveCalendar, needsText, filedOriginalStrip and existingOfferFor (schema or UI); dashboard.js openFilings, openDeadline and the element builders.
