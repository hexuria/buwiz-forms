# AGENTS.md

For any agent that opens this repository. Read it first.

If a file named `AGENTS.local.md` sits next to this one, read it too, right after this file: it holds this checkout's own notes, is ignored by git, and is never committed here.

## The project

eBIRForms (E-BIRForms) is an offline-first app to prepare, validate, print and file Philippine BIR returns. It covers the 43 forms of the official eBIRForms package. A Rust back end runs under screens built on NSH (Native Shell, our fork hexuria/nsh). It ships on macOS, Windows, Linux, iPhone, iPad and Android. Filing goes through the Buwiz relay; sign-in goes to the Buwiz account server (buwiz-com/buwiz-com). `README.md` is the user's view; `docs/DEVELOPMENT.md` the developer's.

## Layout

```
crates/bir-core          forms, validation, filing, the database
crates/bir-print         the frozen-sheet print engine
crates/bir-app-api       the API the screens call; the print path (static_html.rs, print_sheet.rs)
crates/bir-rules*        the validation-rules corpus tooling
nsh/                     the app: TSX screens over a Rust crate. Read nsh/AGENTS.md before changing it.
forms/, rules/           form definitions and the official form's rules, with evidence
html-frozen/             the frozen printed sheets
agent-recipes/           gpui-agent recipes; agent-recipes/e2e/ holds the e2e suites
scripts/                 setup, CI planning, the batch tool, the e2e runner; scripts/tests/ tests them
docs/                    platform pages, CI (docs/CI.md), proof (docs/proof/E2E.md), versioning
packaging/, assets/      installers and store assets
crew/                    the AI crew that ships the app
```

## Safe commands (they build nothing)

```sh
just --list                                        # every recipe, with its one-line description
python3 -m unittest discover -s scripts/tests -v   # the script tests
just e2e check smoke                               # checks the e2e suite's recipes; no app is started
```

Where these files say `python3`, use `python` on Windows.

Everything else may build the app: read `docs/DEVELOPMENT.md` and `CONTRIBUTING.md` first.

## Safety: every run of the app by an agent

Set all four, every time:

```sh
BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1
```

Never set `BIR_SFTP_LIVE`. Use a scratch database, never a real one. Never send anything to the production relay, the production account server or the real BIR: a release build of `main` files for real, so never press Queue, Send or Submit in one. Use only the demo taxpayer and invented values. Never print, commit or copy a key, token, certificate or `.env` value.

## Pull requests

One change per pull request, from `main`. Pull requests run quick checks only (`ci-ok`); the test suites run on request (`just ci-test`). `docs/CI.md` explains what runs when and what "ready" means.

## Commit identity

Commit with your own identity, as any contributor does. A session that was started as a crew role uses the identity its `crew.local.ini` sets instead (`crew/RULES.md`).

## The crew

The files in `crew/` and the crew settings apply only when you were started as a crew role (by `/crew`, `just crew start` or `crew run`). Any other session works as an ordinary contributor: it does not take a crew role, merge, tag, or follow the crew's playbooks unless the person asks for that. To run or continue the crew, read `crew/README.md`.
