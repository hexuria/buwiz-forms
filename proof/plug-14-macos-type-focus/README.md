driver: gpui-agent-cli 0.4.1 + nsh-gpui-agent 3e78d30346208249e09bc440902c95e87c0bcf82

# Proof for hexuria/nsh-gpui-agent#14: a bare type after the segmented TIN control lands on macOS

App: hexuria/buwiz-forms main 5079835149c1d79d9b6cee25e2639544138f5bee (pins the plug at b559e40d). The plug's
pull request is hexuria/nsh-gpui-agent#26 (head 3e78d30346208249e09bc440902c95e87c0bcf82). Every run:
BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1, a scratch database, gpui-agent
recipes only, Queue/Send never pressed.

## The probe

`sign-up-bare.wants` is the smoke suite's `sign-up.wants` with ONE difference: the workaround line
`click new-tp-name` is removed, so the name is typed BARE right after the TIN - the exact failing pattern
of the issue (the suite itself is untouched; the probe lives only here). `p14bare.json` runs setup-start
then this sign-up on the desktop variant.

## The cause (three lines)

1. On macOS the plug posts its synthetic mouse events with `postEvent_atStart` - they are delivered later, when the app's runloop turns.
2. Key events are sent synchronously (`sendEvent`), so `type`'s focusing press had not been delivered when the keys went out.
3. The keys landed in whatever still held the focus - the segmented TIN editor, whose full cells swallow extra digits: the text landed nowhere.

The fix (in the plug, not the recipes): after the focusing press, wait for the platform to settle and require
`focused() == Some(target)` before typing; error out otherwise.

## Before and after

| run | plug | setup-start | sign-up | evidence |
|---|---|---|---|---|
| before | b559e40d (the pin) | pass 748ms | **fail** s19: name empty - taxpayer created as "Tax profile: 1 TEST ST" (the address line) | before/report.md |
| after | 3e78d303 (PR #26) | pass 714ms | **pass** 8245ms | after/report.md |

After pass repeated 3/3 (this folder's `after` is the first; two more identical passes followed). The before run's
failure screenshot (before/02-sign-up/failure.png) shows the created taxpayer named after its address because the
name box never received the typed text.

## How the app's plug was pointed at the fix

`nsh/Cargo.toml` carried a LOCAL, uncommitted one-line override - the plug git deps' `rev` set to
3e78d30346208249e09bc440902c95e87c0bcf82 instead of the pinned b559e40d - never committed; `git status` in the
proof checkout shows the worktree clean apart from `proof/`. Verified in the binary: `libnsh_agent_actions` in the
after run is compiled from `.cargo/git/checkouts/nsh-gpui-agent-*/3e78d30/crates/nsh-agent-actions/src/lib.rs`
(the dep-info file quotes the path). Note for reproducing: a `[patch]` section in `nsh/Cargo.toml` does NOT reach
the generated host workspace in `nsh/.nsh/rust` (the nsh CLI regenerates that manifest), and neither do edits to
the generated manifest - changing the rev in the app's own manifest does.

## The recording

`typed-field.mov` - a full-screen capture of the after run: the bare type lands in the name box after the TIN
(sign-up passes; 80 s cap covers the whole suite).
