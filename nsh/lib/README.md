# lib/

Pure TypeScript helpers ported from app/shell, each with a *.test.ts beside it (nsh test runs them headless; no open()). No nsh import and no host call: the clock and the store come in as parameters. The element builders stay with the screens.

- `todo.ts`: effectiveStops, stopCall, itemKeys, todoRows, compareRows, nextRow (6 exports) from app/shell/todo.js.
- `periods.ts`: frequencyOf, periodOptions, periodLabel (3) from app/shell/periods.js.
- `recent.ts`: recentEntry, recentOpenOptions, recentKey, sameReturn, loadRecent, addRecent, bumpRecent (7) from app/shell/recent.js.
- `facts.ts`: FACT_LABELS, FACT_WHY, ROW_FACTS, incomeTypeChoices, EOPT_TIERS, ELECTION_CHOICES, factValue, electionSourceText, readerChips, joinNames, notSetForForm, notSetSentence (12) from app/shell/facts.js.
- `settings_model.ts`: SETTINGS_SECTIONS, SETTINGS_ROWS, THEME_OPTIONS, queryWords, hits, settingsView, shownSection, sectionById, rowShown, hotkeyLabel, hotkeyOptions, retentionLabel, accountSummary (13) from app/shell/settings_model.js.
- `chrome_gate.ts`: shellChromeVisible (1) from app/shell/chrome_gate.js.
- `dashboard.ts`: loadDashboard, pickedDeadlines, tileSpecs, progressText, nextText (5) from the data half of app/shell/dashboard.js.
- `dates.ts`: MONTHS, MONTHS_LONG, DATE_ERROR, isoDate, addDays, daysUntil, shortDate, shortIsoDate, parseTypedDate, formatMDY, formatLongDate, shownDate, oneDateText (13) from the date helpers of app/shell/store.js and the date parsers and formatters of app/shell/form_page.js.
- `money.ts`: formatMoney, wholePesosOf (2) from app/shell/form_page.js.
- `errors.ts`: errorText (1) from app/shell/store.js.

Changes from the old code (all else is the same behaviour):
- daysUntil, periodOptions and loadDashboard take an optional clock (the system's by default). loadDashboard takes the reads (DashboardReads) where the old code took app; it never used anything else of app.
- form_page.js's shortDate is shortIsoDate here, because store.js's shortDate is another function with the same name.
- tileSpecs, progressText and nextText were private in dashboard.js; exported because Home draws from them.
- factValue reads atc_codes only from an array (the old code threw on a non-array); itemKeys reads a table with no keys as no keys (the old code threw).

Not ported here: store.js formatTin and displayCode (not date or error helpers, so not in this brief); the Store class; form_page.js dateParts, oneDate, moveCalendar, needsText, filedOriginalStrip and existingOfferFor (schema or UI); dashboard.js openFilings, openDeadline and the element builders.
