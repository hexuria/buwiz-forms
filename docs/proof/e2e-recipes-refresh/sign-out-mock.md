# No-bypass sign-out proof

`agent-recipes/e2e/sign-out-mock.json` is deliberately outside the default smoke suite. `just e2e` always launches with `BIR_AUTH_BYPASS=1`, and with `--no-launch` its runner refuses `sign_in` recipes, so this variant cannot be run by the current suite runner without changing code outside this PR's permitted paths.

To run it with gpui-agent only:

```sh
just mock-account --auto-approve &
EBIR_TEST_ENV=1 BUWIZ_NO_KEYCHAIN=1 BIR_AUTH_BYPASS=0 BIR_SFTP_DRY_RUN=1 nsh run --target macos
# or the iOS Simulator equivalent with SIMCTL_CHILD_BIR_AUTH_BYPASS=0
gpui-agent-cli recipe run agent-recipes/e2e/sign-in-mock.wants
gpui-agent-cli recipe run agent-recipes/e2e/sign-out.wants      # desktop
gpui-agent-cli recipe run agent-recipes/e2e/sign-out-phone.wants # phone
```

The sign-in recipe uses only the loopback mock account server. The sign-out recipe ends by waiting for `locked-screen`, which exists only when the bypass is absent.
