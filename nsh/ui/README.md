The UI kit: every nsh/base control wrapped once and styled from theme tokens. Screens import from `ui/index.ts`, never
from nsh/base. Gallery: `screens/base/KitPage.tsx` (App.tsx, "UI kit"); screenshots: `just nsh-shots kit`.
Files: text.tsx (Text, Line, Code, titles, Icon, Kbd, Spinner), buttons.tsx, lists.tsx, status.tsx, choice.tsx, fields.tsx,
layers.tsx; pure logic in mask.ts (masks, dates) and logic.ts (avatars, chips, pick-list filters), each with a *.test.ts.
