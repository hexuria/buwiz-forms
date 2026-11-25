# macOS build-machine setup proof

Date: 2026-11-25
Machine: Apple Virtual Machine 1 (`ip-172-16-5-2.us-west-2.compute.internal`)
macOS: 26.6.2 (25G83)
Xcode: 26.6 (17F113)
NSH CLI: `nsh 0.6.1 (8c78cfd)`
NSH pin: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`
Free disk after setup and proof: 16 GiB of 123 GiB

## Proof

| Step | Result | Log |
|---|---|---|
| `just nsh-check` | PASS | [logs/nsh-check.txt](logs/nsh-check.txt) |
| `just nsh-build` | PASS | [logs/nsh-build.txt](logs/nsh-build.txt) |
| Booted iPhone screenshot | PASS | [logs/simulator-screenshot.txt](logs/simulator-screenshot.txt) |

The first `nsh-check` invocation failed before Cargo ran because the non-login shell did not have `/opt/homebrew/opt/rustup/bin` on `PATH`; that log is retained at [logs/nsh-check-initial-path-failure.txt](logs/nsh-check-initial-path-failure.txt). The passing run used the required rustup path explicitly.

## Visual evidence

- [iPhone 17 Pro simulator screenshot](simulator-iphone-17-pro.png)
- [iPhone 17 Pro simulator screen recording](simulator-iphone-17-pro.mov)
- [Simulator recording log](logs/simulator-recording.txt)

## Environment safety

Proof commands ran with:

```sh
BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1
```

`BIR_SFTP_LIVE` was not set. No production relay or real BIR endpoint was used.

## Machine details

- [machine-report.txt](machine-report.txt)
- [simulators.txt](simulators.txt)
