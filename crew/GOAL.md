# Goal

eBIRForms 1.0.0 in three stores: the App Store for iPhone and iPad, the Mac App Store and the Microsoft Store.
Google Play follows as its own release right after.
Every platform files returns for real through the Buwiz relay. No platform ships as a dry-run-only build.
The accounts and lock work (one room per account on a device, one PIN per account, biometrics, the server's TIN
status) lands before the 1.0.0 cut.

## Done means

1. The release build of `main`, signed in to a real Buwiz account by the maintainer, files a return for a test TIN
   as a dry run, then a return for a real TIN for real, through the relay.
2. Every store listing is ready, and the maintainer has pressed Submit in each store.
3. One version number names the release everywhere (`docs/VERSIONING.md`).
4. `main` has one complete green run after the last merge (`just ci-main`, or a batch whose tree equals `main`).

## Tracks

| Track | Role | Where to start reading (plan file) |
| --- | --- | --- |
| Forms: gates, typing, print | `forms` | No plan file in the repository. Read `docs/FORMS.md`, `rules/README.md`, `crates/bir-print/`. |
| Accounts, lock, sign-in, relay filing (client side) | `accounts` | No plan file in the repository (the plan is the maintainer's). Read `docs/cloud-account.md`, `docs/relay.md`. |
| App Store (iPhone, iPad) and Mac App Store | `apple` | No plan file. Read `docs/platforms/ios.md`, `docs/platforms/macos.md`, `docs/VERSIONING.md`. |
| Google Play | `android` | No plan file. Read `docs/platforms/android.md`. |
| Microsoft Store | `windows-store` | No plan file. Read `docs/platforms/windows.md`, `CONTRIBUTING.md`. |
| Account server | `server` | No plan file here; the server lives in `buwiz-com/buwiz-com`. |
| The UI driver and its NSH plug | `driver` | No plan file. Read `docs/proof/E2E.md` ("Gaps to fix"). |
| The NSH runtime fork | `nsh` | No plan file. Read `nsh/AGENTS.md`, `docs/platforms/devin-environment.md`. |
| The crew's own tools | `tools` | No plan file. Read `docs/CI.md`, `docs/proof/E2E.md`. |
| Order, gate, batch, merge | `conductor` | This file, `crew/RULES.md`, `crew/playbooks/`. |
| Independent review | `reviewer` | `crew/playbooks/review.md`. |

The app's migration contract is `docs/plans/nsh-tsx-migration.md`; every role that changes `nsh/` follows it.
