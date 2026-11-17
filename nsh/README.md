# nsh/

The eBIRForms desktop and phone app on Native Shell (NSH): TSX screens over a Rust crate. Start with [AGENTS.md](AGENTS.md); the plan is
`docs/plans/nsh-tsx-migration.md`. From the repository root: `just nsh-check`, `just nsh-build`, `just nsh-test`, `just nsh-run`, `just nsh-shots base`.

Building anywhere other than the owner's Mac, and what works on Windows and Linux (nothing there has been built or run): [docs/platforms/nsh.md](../docs/platforms/nsh.md).
The NSH checkout must sit next to this repository's folder (`<parent>/buwiz-forms` and `<parent>/nsh`): `Cargo.toml` names the NSH crates as `../../nsh/crates/...`.
