Filings (C4): every saved and filed return, from draft to confirmed. Ported from app/shell/filings.js.

- `Filings.tsx` the view (registered in shell/views.tsx; route `filings`, layout full). Wide (page 720 pt and up): search, four MultiCombos, Clear filters, count, then `Board.tsx` (four columns; Paid joins Confirmed). Narrow: search + Filters button (a `Sheet` of the four combos), kit `Tabs` (All, Draft, Queued, Submitted, Confirmed, with counts, scrolling sideways), `NarrowList.tsx`.
- `ReturnCard.tsx` one return as a card (the whole card opens it; Cancel queue on a queued one); the card is the kit's `PressCard` (`size="row"`).
- `model.ts` pure rules (tested in `model.test.ts`); `filters.ts` the filter state (module signals, survives leaving the view); `FilterCombos.tsx` the four combos.
- `LegacyGroup.tsx` + `legacy.ts` the group "Filed in BIR eBIRForms" under the board (wide) or the list's All tab (narrow): what the import from the old program brought in, for the taxpayer in view, newest period first. Records only (no card, no press, never queued); nothing is drawn without rows, and it steps aside while a search or filter is on. Tests `legacy.test.ts`, `LegacyGroup.test.tsx` (the fixture's `legacy: "rows"`), shots `just nsh-shots f20`.
- A Home tile arrives as `go("filings", { statuses, updatedMonth })` (also read inside `preset`): it replaces every filter, once per navigation.
- Tests `Filings.test.tsx` (desktop 1440x900 and phone 390x844) against `seed.ts` through the fixture `nsh/FilingsDemo.tsx`; shots `nsh/shots/filings.test.tsx` (`just nsh-shots filings`).
