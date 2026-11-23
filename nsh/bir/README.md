# bir: the typed door to the Rust brain

Screens import from here and nowhere else (`import { bir, type Taxpayer } from "../../bir"`). Never import `Door` from `ebirforms/native` in a screen.

| File | What |
|---|---|
| `index.ts` | The public face: the wrappers, the types, `doorStatus()`, `hostFunctions()`, `shutdown()`. |
| `functions.ts` | **Generated.** One typed function per host function the old app calls (`bir.system_info()`, `await bir.screen_form_open(...)`), the `bir` object of them, and every type of `types.d.ts`. |
| `types.d.ts` | **Generated** from the TypeScript declarations bir-app-api writes for the `bir` module. A Rust test (`door::tests::the_committed_types_file_is_current`) fails while it is stale. |
| `door.ts` | Status, function list and shutdown, over the native module. |
| `testing.ts` | Tests only: `createDemoTaxpayer()`, `DEMO`, `useDatabase(path)`. |
| `DoorProbe.tsx`, `*.test.ts(x)` | A mounted component calling the door, and the door's tests (empty scratch database per engine). |
| `regenerate.sh`, `old-app-functions.txt`, `extra-functions.txt` | `bash nsh/bir/regenerate.sh` rewrites the two generated files. Names come from `old-app-functions.txt` (what the old app called, frozen when its sources were deleted) plus the extra list, where a new screen's function goes. |

The Rust side is `nsh/src` (`door.rs` boot and dispatch, `guards.rs` the send and database guards, `hooks.rs` what the host functions ask of a UI). Details: `nsh/AGENTS.md`, "The door".
