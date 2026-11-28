# No-bypass sign-out proof

`agent-recipes/e2e/sign-out-mock.json` is deliberately outside the default smoke suite. `just e2e` always launches with `BIR_AUTH_BYPASS=1`, and with `--no-launch` its runner refuses `sign_in` recipes, so this variant cannot be run by the current suite runner without changing code outside this PR's permitted paths.

Run it with gpui-agent only, on macOS, against the loopback mock account server:

```sh
just mock-account --auto-approve &
token=$(python3 -c 'import secrets; print(secrets.token_hex(16))')
GPUI_AGENT=1 GPUI_AGENT_TOKEN="$token" GPUI_AGENT_ADDR=127.0.0.1:17421 \
EBIR_TEST_ENV=1 BUWIZ_NO_KEYCHAIN=1 BIR_AUTH_BYPASS=0 BIR_SFTP_DRY_RUN=1 \
CARGO="$PWD/nsh/agent/cargo-with-agent.sh" nsh run --target macos &
GPUI_AGENT_TOKEN="$token" gpui-agent-cli recipe run agent-recipes/e2e/sign-in-mock.wants
GPUI_AGENT_TOKEN="$token" gpui-agent-cli recipe run agent-recipes/e2e/sign-out.wants
```

For a phone-sized run, launch the iOS development host without the bypass (`SIMCTL_CHILD_BIR_AUTH_BYPASS=0`, with the same dry-run test settings and the iOS token handoff described by `just nsh-run-agent`) and run `sign-out-phone.wants` after `sign-in-mock.wants`. The iOS Simulator reaches the mock on the host's loopback.

The sign-in recipe uses only the loopback mock account server. The sign-out recipe ends by waiting for `locked-screen`, which exists only when the bypass is absent.

Known gaps, marked in `agent-recipes/e2e/sign-out-mock.json`: the mock's `--auto-approve` does not approve the email sign-in code, and the driver cannot type it because the focusable code box has no test id (#608); after that, a fresh database has no local PIN or authenticator, so `endSession` refuses to lock the admin session and the suite may not type either secret (#450). Until both close, the commands above are the manual rehearsal harness, not a passing proof.
