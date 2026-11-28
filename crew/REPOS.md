# The repositories

eBIRForms is built from five repositories. This one, hexuria/buwiz-forms, is the hub: the app, the crew files and the CI that proves a release.

| Repository | What it is | Work branch | How the app pins it | Who merges |
| --- | --- | --- | --- | --- |
| hexuria/buwiz-forms | The app: Rust crates, the NSH screens in `nsh/`, forms, print, packaging, CI. | `main` | (the app itself) | whoever GitHub lets merge: the maintainers, or a conductor they told to (by batch, `crew/RULES.md`) |
| hexuria/nsh | Our fork of NSH (Native Shell), the runtime the screens run on. Default branch `main`; the app's line is `buwiz-next`. | `buwiz-next` | `NSH_PINNED_REF` in `.github/workflows/nsh.yml`: an exact commit on `buwiz-next` | whoever GitHub lets merge, when ready (`crew/RULES.md`) |
| hexuria/gpui-agent | The UI driver: protocol, recipe runner, CLI. Releases publish crates. | `main` | the `gpui-agent` version in `Cargo.toml` and `nsh/Cargo.toml`; `gpui-agent-cli` at the version `Cargo.lock` pins | whoever GitHub lets merge, when ready (`crew/RULES.md`) |
| hexuria/nsh-gpui-agent | The driver's NSH plug: puts the app's windows into the tree, delivers presses and keys. | `main` | the `rev` of every `nsh-gpui-agent*` dependency in `nsh/Cargo.toml` (all the same rev) | whoever GitHub lets merge, when ready (`crew/RULES.md`) |
| buwiz-com/buwiz-com | The Buwiz account server (`account.buwiz.com`, `api.buwiz.com`). | `main` | not pinned: the app speaks to it by the contracts in `docs/cloud-account.md` | the maintainer; deploys to production are always the maintainer's |

Read each pin from its file when you need it. Never copy a pin value into another document: it goes stale.

```sh
grep -n NSH_PINNED_REF .github/workflows/nsh.yml
grep -n 'gpui-agent' Cargo.toml nsh/Cargo.toml
```

One more repository feeds the app: hexuria/buwiz-forms-html, the source of the frozen print sheets, owned by the `forms` role. CI checks out its default branch (`.github/workflows/ci.yml`). Only the maintainer merges there.

## The order of a cross-repo change

A change that spans repositories lands one step at a time, each step merged and released before the next starts:

```
driver (gpui-agent) → plug (nsh-gpui-agent) → app (buwiz-forms)
nsh (buwiz-next) → app (buwiz-forms, NSH_PINNED_REF)
server (buwiz-com) contract page → server and app, each by its own pull request
```

- **Driver, then plug, then app.** The driver merges and its release is tagged (`crew/playbooks/release.md` says who may). The plug moves to the new driver and merges. The app moves to the new plug rev (and driver version) in its own pull request.
- **NSH, then app.** A kit pull request merges on `buwiz-next`. The app moves `NSH_PINNED_REF` in its own pull request, proven on every system, followed by `just ci-main`.
- **Lock files** are written by cargo on a machine with the sibling checkouts (the NSH checkout beside this one), never by hand.
- **Never the other way round.** An app pull request that points at an unmerged driver, plug or kit commit is not ready: the commit can still change or vanish.

## Sibling checkouts

The app's Rust compiles NSH from a sibling checkout (`../nsh` from the repository root), at `NSH_PINNED_REF`. The setup scripts check it out and build the `nsh` tool and its runtimes from it. It is read-only: never edit it, and never patch NSH to fit the app.
