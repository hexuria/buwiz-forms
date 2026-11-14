The frame around every screen (agent B3). Entry: `Shell.tsx` (mounted by `../App.tsx`). What a screen imports: `index.ts`. How to add a screen, the dev start route, the test fixture and the pitfalls: `../AGENTS.md`, section "The frame".

- `app.ts` the app state (one owner per piece: route and history, layers, selection, session seam, prefs, Recent, notifications, theme, polls, boot). `store.ts` the cached reads over `../bir`, one signal per key.
- `routes.ts` the views (label, icon, needsTaxpayer, layout) and the four tabs. `views.tsx` the component per view (Placeholder until the screen exists). `start.ts` the dev start route. `theme.ts` the theme preference.
- Wide: `TitleBar.tsx`, `Sidebar.tsx`, `Panels.tsx` (profile, notifications), floating under 860. Narrow (under 600): `Narrow.tsx` (header, tab bar, sheets). `Palette.tsx` + `paletteItems.ts`, `NewReturn.tsx`, `DevPages.tsx`, `UpdateSlot.tsx` (wave 4), `parts.tsx` (the few parts the kit has none for).
- Tests: `frame.test.tsx`, `palette.test.tsx`, `narrow.test.tsx` (headless, phone and desktop), `store.test.ts`, `start.test.ts`; real-window shots `../shots/shell.test.tsx`.
