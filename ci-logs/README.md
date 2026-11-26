# win-flake-418: do tinPage and session.clear fail alone or only under shard load?

Machine buwiz-forms-win (Windows Server 2022, console session). One log per run, raw bytes
(cmd redirection, no shell mangling). `nsh test "./<file>"` from buwiz-forms/nsh, exactly the
nsh.yml "NSH app (Windows)" invocation, with the workflow's env (safety variables, LIBCLANG_PATH,
RUST_MIN_STACK, CARGO_INCREMENTAL=0, RUSTUP_TOOLCHAIN=1.99.0). RUSTC_WRAPPER empty. nsh CLI
0.6.1 (8c78cfd), sibling nsh worktree at the pin 8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf.

- A-dc76c69d = PR #418 head dc76c69dc86b05179e9b42013683c9d30bd703e4 (feat/accounts-and-lock-d1)
- B-460ab613 = main 460ab613173882ea982f693ca98698cf578168b9

Each folder: nsh-check.log / nsh-build.log (the build as nsh.yml does), status.tsv (exit codes
and wall seconds), and three solo runs per file. Result: every run passes at both commits;
the failures seen in shards do not reproduce when the files run alone.

Long first runs (A tinPage run1 215 s, B tinPage run1 881 s) are cold-compile time for the
test runner, not test time; the test lines inside name the real durations.
