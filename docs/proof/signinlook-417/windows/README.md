# PR #417 sign-in look: Windows proof

Real native Win32 windows on Windows Server 2022. AFTER is `80fa1f98af8b2290afde32956d4fd3b031f7fbfe`; BEFORE is `979714b2a4211e237083e2a504a24b953a58fb43`. The shots fixtures still pass `platform: "macos"` because the Windows target accepts desktop-to-desktop platform values; this means macOS-specific copy appears in a few fixture pages. No app code or test expectations were changed. The harness used a local, uncommitted resize/verification workaround because the pinned Windows host initially opens test windows at 1440x900 and Windows/NSH clamps some client sizes.

BEFORE has no `signin-wide-light-start.png` because that screenshot did not exist in the BEFORE test file.

| file | page | window look | before/after | anything odd |
|---|---|---|---|---|
| `after/signin-wide-light-start.png` | sign-in flow: start | wide-light, 1440x900 | after |  |
| `after/signin-wide-light-email.png` | sign-in flow: email | wide-light, 1440x900 | after |  |
| `after/signin-wide-light-email-bad-address.png` | sign-in flow: email-bad-address | wide-light, 1440x900 | after |  |
| `after/signin-wide-light-email-busy.png` | sign-in flow: email-busy | wide-light, 1440x900 | after |  |
| `after/signin-wide-light-email-refused.png` | sign-in flow: email-refused | wide-light, 1440x900 | after |  |
| `after/signin-wide-light-email-not-available.png` | sign-in flow: email-not-available | wide-light, 1440x900 | after |  |
| `after/signin-wide-light-email-no-connection.png` | sign-in flow: email-no-connection | wide-light, 1440x900 | after |  |
| `after/signin-wide-light-code.png` | sign-in flow: code | wide-light, 1440x900 | after |  |
| `after/signin-wide-light-code-typing.png` | sign-in flow: code-typing | wide-light, 1440x900 | after |  |
| `after/signin-wide-light-code-wrong.png` | sign-in flow: code-wrong | wide-light, 1440x900 | after |  |
| `after/signin-wide-light-code-checking.png` | sign-in flow: code-checking | wide-light, 1440x900 | after |  |
| `after/signin-wide-light-code-expired-resend.png` | sign-in flow: code-expired-resend | wide-light, 1440x900 | after |  |
| `after/signin-wide-light-code-too-many.png` | sign-in flow: code-too-many | wide-light, 1440x900 | after |  |
| `after/signin-wide-light-code-restored.png` | sign-in flow: code-restored | wide-light, 1440x900 | after |  |
| `after/signin-wide-light-settings-account-signed-in.png` | sign-in flow: settings-account-signed-in | wide-light, 1440x900 | after |  |
| `after/signin-wide-light-settings-account-sign-out-question.png` | sign-in flow: settings-account-sign-out-question | wide-light, 1440x900 | after |  |
| `after/signin-wide-light-settings-account-signed-out.png` | sign-in flow: settings-account-signed-out | wide-light, 1440x900 | after |  |
| `after/signin-wide-light-email-after-change.png` | sign-in flow: email-after-change | wide-light, 1440x900 | after |  |
| `after/signin-wide-light-code-after-change.png` | sign-in flow: code-after-change | wide-light, 1440x900 | after |  |
| `after/signin-wide-dark-start.png` | sign-in flow: start | wide-dark, 1440x900 | after |  |
| `after/signin-wide-dark-email.png` | sign-in flow: email | wide-dark, 1440x900 | after |  |
| `after/signin-wide-dark-email-bad-address.png` | sign-in flow: email-bad-address | wide-dark, 1440x900 | after |  |
| `after/signin-wide-dark-email-busy.png` | sign-in flow: email-busy | wide-dark, 1440x900 | after |  |
| `after/signin-wide-dark-email-refused.png` | sign-in flow: email-refused | wide-dark, 1440x900 | after |  |
| `after/signin-wide-dark-email-not-available.png` | sign-in flow: email-not-available | wide-dark, 1440x900 | after |  |
| `after/signin-wide-dark-email-no-connection.png` | sign-in flow: email-no-connection | wide-dark, 1440x900 | after |  |
| `after/signin-wide-dark-code.png` | sign-in flow: code | wide-dark, 1440x900 | after |  |
| `after/signin-wide-dark-code-typing.png` | sign-in flow: code-typing | wide-dark, 1440x900 | after |  |
| `after/signin-wide-dark-code-wrong.png` | sign-in flow: code-wrong | wide-dark, 1440x900 | after |  |
| `after/signin-wide-dark-code-checking.png` | sign-in flow: code-checking | wide-dark, 1440x900 | after |  |
| `after/signin-wide-dark-code-expired-resend.png` | sign-in flow: code-expired-resend | wide-dark, 1440x900 | after |  |
| `after/signin-wide-dark-code-too-many.png` | sign-in flow: code-too-many | wide-dark, 1440x900 | after |  |
| `after/signin-wide-dark-code-restored.png` | sign-in flow: code-restored | wide-dark, 1440x900 | after |  |
| `after/signin-wide-dark-settings-account-signed-in.png` | sign-in flow: settings-account-signed-in | wide-dark, 1440x900 | after |  |
| `after/signin-wide-dark-settings-account-sign-out-question.png` | sign-in flow: settings-account-sign-out-question | wide-dark, 1440x900 | after |  |
| `after/signin-wide-dark-settings-account-signed-out.png` | sign-in flow: settings-account-signed-out | wide-dark, 1440x900 | after |  |
| `after/signin-wide-dark-email-after-change.png` | sign-in flow: email-after-change | wide-dark, 1440x900 | after |  |
| `after/signin-wide-dark-code-after-change.png` | sign-in flow: code-after-change | wide-dark, 1440x900 | after |  |
| `after/signin-narrow-light-start.png` | sign-in flow: start | narrow-light, 390x844 | after |  |
| `after/signin-narrow-light-email.png` | sign-in flow: email | narrow-light, 390x844 | after |  |
| `after/signin-narrow-light-email-bad-address.png` | sign-in flow: email-bad-address | narrow-light, 390x844 | after |  |
| `after/signin-narrow-light-email-busy.png` | sign-in flow: email-busy | narrow-light, 390x844 | after |  |
| `after/signin-narrow-light-email-refused.png` | sign-in flow: email-refused | narrow-light, 390x844 | after |  |
| `after/signin-narrow-light-email-not-available.png` | sign-in flow: email-not-available | narrow-light, 390x844 | after |  |
| `after/signin-narrow-light-email-no-connection.png` | sign-in flow: email-no-connection | narrow-light, 390x844 | after |  |
| `after/signin-narrow-light-code.png` | sign-in flow: code | narrow-light, 390x844 | after |  |
| `after/signin-narrow-light-code-typing.png` | sign-in flow: code-typing | narrow-light, 390x844 | after |  |
| `after/signin-narrow-light-code-wrong.png` | sign-in flow: code-wrong | narrow-light, 390x844 | after |  |
| `after/signin-narrow-light-code-checking.png` | sign-in flow: code-checking | narrow-light, 390x844 | after |  |
| `after/signin-narrow-light-code-expired-resend.png` | sign-in flow: code-expired-resend | narrow-light, 390x844 | after |  |
| `after/signin-narrow-light-code-too-many.png` | sign-in flow: code-too-many | narrow-light, 390x844 | after |  |
| `after/signin-narrow-light-code-restored.png` | sign-in flow: code-restored | narrow-light, 390x844 | after |  |
| `after/signin-narrow-light-settings-account-signed-in.png` | sign-in flow: settings-account-signed-in | narrow-light, 390x844 | after |  |
| `after/signin-narrow-light-settings-account-sign-out-question.png` | sign-in flow: settings-account-sign-out-question | narrow-light, 390x844 | after |  |
| `after/signin-narrow-light-settings-account-signed-out.png` | sign-in flow: settings-account-signed-out | narrow-light, 390x844 | after |  |
| `after/signin-narrow-light-email-after-change.png` | sign-in flow: email-after-change | narrow-light, 390x844 | after |  |
| `after/signin-narrow-light-code-after-change.png` | sign-in flow: code-after-change | narrow-light, 390x844 | after |  |
| `after/signin-narrow-dark-start.png` | sign-in flow: start | narrow-dark, 390x844 | after |  |
| `after/signin-narrow-dark-email.png` | sign-in flow: email | narrow-dark, 390x844 | after |  |
| `after/signin-narrow-dark-email-bad-address.png` | sign-in flow: email-bad-address | narrow-dark, 390x844 | after |  |
| `after/signin-narrow-dark-email-busy.png` | sign-in flow: email-busy | narrow-dark, 390x844 | after |  |
| `after/signin-narrow-dark-email-refused.png` | sign-in flow: email-refused | narrow-dark, 390x844 | after |  |
| `after/signin-narrow-dark-email-not-available.png` | sign-in flow: email-not-available | narrow-dark, 390x844 | after |  |
| `after/signin-narrow-dark-email-no-connection.png` | sign-in flow: email-no-connection | narrow-dark, 390x844 | after |  |
| `after/signin-narrow-dark-code.png` | sign-in flow: code | narrow-dark, 390x844 | after |  |
| `after/signin-narrow-dark-code-typing.png` | sign-in flow: code-typing | narrow-dark, 390x844 | after |  |
| `after/signin-narrow-dark-code-wrong.png` | sign-in flow: code-wrong | narrow-dark, 390x844 | after |  |
| `after/signin-narrow-dark-code-checking.png` | sign-in flow: code-checking | narrow-dark, 390x844 | after |  |
| `after/signin-narrow-dark-code-expired-resend.png` | sign-in flow: code-expired-resend | narrow-dark, 390x844 | after |  |
| `after/signin-narrow-dark-code-too-many.png` | sign-in flow: code-too-many | narrow-dark, 390x844 | after |  |
| `after/signin-narrow-dark-code-restored.png` | sign-in flow: code-restored | narrow-dark, 390x844 | after |  |
| `after/signin-narrow-dark-settings-account-signed-in.png` | sign-in flow: settings-account-signed-in | narrow-dark, 390x844 | after |  |
| `after/signin-narrow-dark-settings-account-sign-out-question.png` | sign-in flow: settings-account-sign-out-question | narrow-dark, 390x844 | after |  |
| `after/signin-narrow-dark-settings-account-signed-out.png` | sign-in flow: settings-account-signed-out | narrow-dark, 390x844 | after |  |
| `after/signin-narrow-dark-email-after-change.png` | sign-in flow: email-after-change | narrow-dark, 390x844 | after |  |
| `after/signin-narrow-dark-code-after-change.png` | sign-in flow: code-after-change | narrow-dark, 390x844 | after |  |
| `after/signin-tall-light-start.png` | sign-in flow: start | tall-light, 1100x1300 | after |  |
| `after/signin-tall-light-email.png` | sign-in flow: email | tall-light, 1100x1300 | after |  |
| `after/signin-tall-light-email-bad-address.png` | sign-in flow: email-bad-address | tall-light, 1100x1300 | after |  |
| `after/signin-tall-light-email-busy.png` | sign-in flow: email-busy | tall-light, 1100x1300 | after |  |
| `after/signin-tall-light-email-refused.png` | sign-in flow: email-refused | tall-light, 1100x1300 | after |  |
| `after/signin-tall-light-email-not-available.png` | sign-in flow: email-not-available | tall-light, 1100x1300 | after |  |
| `after/signin-tall-light-email-no-connection.png` | sign-in flow: email-no-connection | tall-light, 1100x1300 | after |  |
| `after/signin-tall-light-code.png` | sign-in flow: code | tall-light, 1100x1300 | after |  |
| `after/signin-tall-light-code-typing.png` | sign-in flow: code-typing | tall-light, 1100x1300 | after |  |
| `after/signin-tall-light-code-wrong.png` | sign-in flow: code-wrong | tall-light, 1100x1300 | after |  |
| `after/signin-tall-light-code-checking.png` | sign-in flow: code-checking | tall-light, 1100x1300 | after |  |
| `after/signin-tall-light-code-expired-resend.png` | sign-in flow: code-expired-resend | tall-light, 1100x1300 | after |  |
| `after/signin-tall-light-code-too-many.png` | sign-in flow: code-too-many | tall-light, 1100x1300 | after |  |
| `after/signin-tall-light-code-restored.png` | sign-in flow: code-restored | tall-light, 1100x1300 | after |  |
| `after/signin-tall-light-settings-account-signed-in.png` | sign-in flow: settings-account-signed-in | tall-light, 1100x1300 | after |  |
| `after/signin-tall-light-settings-account-sign-out-question.png` | sign-in flow: settings-account-sign-out-question | tall-light, 1100x1300 | after |  |
| `after/signin-tall-light-settings-account-signed-out.png` | sign-in flow: settings-account-signed-out | tall-light, 1100x1300 | after |  |
| `after/signin-tall-light-email-after-change.png` | sign-in flow: email-after-change | tall-light, 1100x1300 | after |  |
| `after/signin-tall-light-code-after-change.png` | sign-in flow: code-after-change | tall-light, 1100x1300 | after |  |
| `after/signin-short-light-start.png` | sign-in flow: start | short-light, 1280x640 | after |  |
| `after/signin-short-light-email.png` | sign-in flow: email | short-light, 1280x640 | after |  |
| `after/signin-short-light-email-bad-address.png` | sign-in flow: email-bad-address | short-light, 1280x640 | after |  |
| `after/signin-short-light-email-busy.png` | sign-in flow: email-busy | short-light, 1280x640 | after |  |
| `after/signin-short-light-email-refused.png` | sign-in flow: email-refused | short-light, 1280x640 | after |  |
| `after/signin-short-light-email-not-available.png` | sign-in flow: email-not-available | short-light, 1280x640 | after |  |
| `after/signin-short-light-email-no-connection.png` | sign-in flow: email-no-connection | short-light, 1280x640 | after |  |
| `after/signin-short-light-code.png` | sign-in flow: code | short-light, 1280x640 | after |  |
| `after/signin-short-light-code-typing.png` | sign-in flow: code-typing | short-light, 1280x640 | after |  |
| `after/signin-short-light-code-wrong.png` | sign-in flow: code-wrong | short-light, 1280x640 | after |  |
| `after/signin-short-light-code-checking.png` | sign-in flow: code-checking | short-light, 1280x640 | after |  |
| `after/signin-short-light-code-expired-resend.png` | sign-in flow: code-expired-resend | short-light, 1280x640 | after |  |
| `after/signin-short-light-code-too-many.png` | sign-in flow: code-too-many | short-light, 1280x640 | after |  |
| `after/signin-short-light-code-restored.png` | sign-in flow: code-restored | short-light, 1280x640 | after |  |
| `after/signin-short-light-settings-account-signed-in.png` | sign-in flow: settings-account-signed-in | short-light, 1280x640 | after | the scrollable settings page continues below the visible area |
| `after/signin-short-light-settings-account-sign-out-question.png` | sign-in flow: settings-account-sign-out-question | short-light, 1280x640 | after |  |
| `after/signin-short-light-settings-account-signed-out.png` | sign-in flow: settings-account-signed-out | short-light, 1280x640 | after | the scrollable settings page continues below the visible area |
| `after/signin-short-light-email-after-change.png` | sign-in flow: email-after-change | short-light, 1280x640 | after |  |
| `after/signin-short-light-code-after-change.png` | sign-in flow: code-after-change | short-light, 1280x640 | after |  |
| `after/auth-wide-light-gate.png` | gate/setup/lock: gate | wide-light, 1440x900 | after |  |
| `after/auth-wide-light-gate-refused.png` | gate/setup/lock: gate-refused | wide-light, 1440x900 | after |  |
| `after/auth-wide-light-setup-choose.png` | gate/setup/lock: setup-choose | wide-light, 1440x900 | after |  |
| `after/auth-wide-light-setup-pin-error.png` | gate/setup/lock: setup-pin-error | wide-light, 1440x900 | after | this page uses a bottom Back link, not the top-left back control |
| `after/auth-wide-light-setup-key.png` | gate/setup/lock: setup-key | wide-light, 1440x900 | after |  |
| `after/auth-wide-light-setup-solo-taxpayer.png` | gate/setup/lock: setup-solo-taxpayer | wide-light, 1440x900 | after |  |
| `after/auth-wide-light-signin.png` | gate/setup/lock: signin | wide-light, 1440x900 | after |  |
| `after/auth-wide-light-signin-error.png` | gate/setup/lock: signin-error | wide-light, 1440x900 | after |  |
| `after/auth-wide-light-signin-authenticator.png` | gate/setup/lock: signin-authenticator | wide-light, 1440x900 | after |  |
| `after/auth-wide-light-forgot-taxpayer.png` | gate/setup/lock: forgot-taxpayer | wide-light, 1440x900 | after |  |
| `after/auth-wide-light-forgot-admin.png` | gate/setup/lock: forgot-admin | wide-light, 1440x900 | after | copy says Mac login because the fixture uses the macOS desktop platform |
| `after/auth-wide-light-signin-no-code.png` | gate/setup/lock: signin-no-code | wide-light, 1440x900 | after |  |
| `after/auth-wide-dark-gate.png` | gate/setup/lock: gate | wide-dark, 1440x900 | after |  |
| `after/auth-wide-dark-gate-refused.png` | gate/setup/lock: gate-refused | wide-dark, 1440x900 | after |  |
| `after/auth-wide-dark-setup-choose.png` | gate/setup/lock: setup-choose | wide-dark, 1440x900 | after |  |
| `after/auth-wide-dark-setup-pin-error.png` | gate/setup/lock: setup-pin-error | wide-dark, 1440x900 | after | this page uses a bottom Back link, not the top-left back control |
| `after/auth-wide-dark-setup-key.png` | gate/setup/lock: setup-key | wide-dark, 1440x900 | after |  |
| `after/auth-wide-dark-setup-solo-taxpayer.png` | gate/setup/lock: setup-solo-taxpayer | wide-dark, 1440x900 | after |  |
| `after/auth-wide-dark-signin.png` | gate/setup/lock: signin | wide-dark, 1440x900 | after |  |
| `after/auth-wide-dark-signin-error.png` | gate/setup/lock: signin-error | wide-dark, 1440x900 | after |  |
| `after/auth-wide-dark-signin-authenticator.png` | gate/setup/lock: signin-authenticator | wide-dark, 1440x900 | after |  |
| `after/auth-wide-dark-forgot-taxpayer.png` | gate/setup/lock: forgot-taxpayer | wide-dark, 1440x900 | after |  |
| `after/auth-wide-dark-forgot-admin.png` | gate/setup/lock: forgot-admin | wide-dark, 1440x900 | after | copy says Mac login because the fixture uses the macOS desktop platform |
| `after/auth-wide-dark-signin-no-code.png` | gate/setup/lock: signin-no-code | wide-dark, 1440x900 | after |  |
| `after/auth-narrow-light-gate.png` | gate/setup/lock: gate | narrow-light, 390x844 | after |  |
| `after/auth-narrow-light-gate-refused.png` | gate/setup/lock: gate-refused | narrow-light, 390x844 | after |  |
| `after/auth-narrow-light-setup-choose.png` | gate/setup/lock: setup-choose | narrow-light, 390x844 | after |  |
| `after/auth-narrow-light-setup-pin-error.png` | gate/setup/lock: setup-pin-error | narrow-light, 390x844 | after | this page uses a bottom Back link, not the top-left back control |
| `after/auth-narrow-light-setup-key.png` | gate/setup/lock: setup-key | narrow-light, 390x844 | after |  |
| `after/auth-narrow-light-setup-solo-taxpayer.png` | gate/setup/lock: setup-solo-taxpayer | narrow-light, 390x844 | after | the taxpayer form continues below the visible area |
| `after/auth-narrow-light-signin.png` | gate/setup/lock: signin | narrow-light, 390x844 | after |  |
| `after/auth-narrow-light-signin-error.png` | gate/setup/lock: signin-error | narrow-light, 390x844 | after |  |
| `after/auth-narrow-light-signin-authenticator.png` | gate/setup/lock: signin-authenticator | narrow-light, 390x844 | after |  |
| `after/auth-narrow-light-forgot-taxpayer.png` | gate/setup/lock: forgot-taxpayer | narrow-light, 390x844 | after |  |
| `after/auth-narrow-light-forgot-admin.png` | gate/setup/lock: forgot-admin | narrow-light, 390x844 | after | copy says Mac login because the fixture uses the macOS desktop platform |
| `after/auth-narrow-light-signin-no-code.png` | gate/setup/lock: signin-no-code | narrow-light, 390x844 | after |  |
| `after/auth-narrow-dark-gate.png` | gate/setup/lock: gate | narrow-dark, 390x844 | after |  |
| `after/auth-narrow-dark-gate-refused.png` | gate/setup/lock: gate-refused | narrow-dark, 390x844 | after |  |
| `after/auth-narrow-dark-setup-choose.png` | gate/setup/lock: setup-choose | narrow-dark, 390x844 | after |  |
| `after/auth-narrow-dark-setup-pin-error.png` | gate/setup/lock: setup-pin-error | narrow-dark, 390x844 | after | this page uses a bottom Back link, not the top-left back control |
| `after/auth-narrow-dark-setup-key.png` | gate/setup/lock: setup-key | narrow-dark, 390x844 | after |  |
| `after/auth-narrow-dark-setup-solo-taxpayer.png` | gate/setup/lock: setup-solo-taxpayer | narrow-dark, 390x844 | after | the taxpayer form continues below the visible area |
| `after/auth-narrow-dark-signin.png` | gate/setup/lock: signin | narrow-dark, 390x844 | after |  |
| `after/auth-narrow-dark-signin-error.png` | gate/setup/lock: signin-error | narrow-dark, 390x844 | after |  |
| `after/auth-narrow-dark-signin-authenticator.png` | gate/setup/lock: signin-authenticator | narrow-dark, 390x844 | after |  |
| `after/auth-narrow-dark-forgot-taxpayer.png` | gate/setup/lock: forgot-taxpayer | narrow-dark, 390x844 | after |  |
| `after/auth-narrow-dark-forgot-admin.png` | gate/setup/lock: forgot-admin | narrow-dark, 390x844 | after | copy says Mac login because the fixture uses the macOS desktop platform |
| `after/auth-narrow-dark-signin-no-code.png` | gate/setup/lock: signin-no-code | narrow-dark, 390x844 | after |  |
| `after/auth-tall-light-gate.png` | gate/setup/lock: gate | tall-light, 1100x1300 | after |  |
| `after/auth-tall-light-gate-refused.png` | gate/setup/lock: gate-refused | tall-light, 1100x1300 | after |  |
| `after/auth-tall-light-setup-choose.png` | gate/setup/lock: setup-choose | tall-light, 1100x1300 | after |  |
| `after/auth-tall-light-setup-pin-error.png` | gate/setup/lock: setup-pin-error | tall-light, 1100x1300 | after | this page uses a bottom Back link, not the top-left back control |
| `after/auth-tall-light-setup-key.png` | gate/setup/lock: setup-key | tall-light, 1100x1300 | after |  |
| `after/auth-tall-light-setup-solo-taxpayer.png` | gate/setup/lock: setup-solo-taxpayer | tall-light, 1100x1300 | after |  |
| `after/auth-tall-light-signin.png` | gate/setup/lock: signin | tall-light, 1100x1300 | after |  |
| `after/auth-tall-light-signin-error.png` | gate/setup/lock: signin-error | tall-light, 1100x1300 | after |  |
| `after/auth-tall-light-signin-authenticator.png` | gate/setup/lock: signin-authenticator | tall-light, 1100x1300 | after |  |
| `after/auth-tall-light-forgot-taxpayer.png` | gate/setup/lock: forgot-taxpayer | tall-light, 1100x1300 | after |  |
| `after/auth-tall-light-forgot-admin.png` | gate/setup/lock: forgot-admin | tall-light, 1100x1300 | after | copy says Mac login because the fixture uses the macOS desktop platform |
| `after/auth-tall-light-signin-no-code.png` | gate/setup/lock: signin-no-code | tall-light, 1100x1300 | after |  |
| `after/auth-short-light-gate.png` | gate/setup/lock: gate | short-light, 1280x640 | after |  |
| `after/auth-short-light-gate-refused.png` | gate/setup/lock: gate-refused | short-light, 1280x640 | after |  |
| `after/auth-short-light-setup-choose.png` | gate/setup/lock: setup-choose | short-light, 1280x640 | after |  |
| `after/auth-short-light-setup-pin-error.png` | gate/setup/lock: setup-pin-error | short-light, 1280x640 | after | this page uses a bottom Back link, not the top-left back control |
| `after/auth-short-light-setup-key.png` | gate/setup/lock: setup-key | short-light, 1280x640 | after |  |
| `after/auth-short-light-setup-solo-taxpayer.png` | gate/setup/lock: setup-solo-taxpayer | short-light, 1280x640 | after | the taxpayer form continues past the bottom; ZIP/RDO and the lower controls are not visible |
| `after/auth-short-light-signin.png` | gate/setup/lock: signin | short-light, 1280x640 | after |  |
| `after/auth-short-light-signin-error.png` | gate/setup/lock: signin-error | short-light, 1280x640 | after |  |
| `after/auth-short-light-signin-authenticator.png` | gate/setup/lock: signin-authenticator | short-light, 1280x640 | after |  |
| `after/auth-short-light-forgot-taxpayer.png` | gate/setup/lock: forgot-taxpayer | short-light, 1280x640 | after |  |
| `after/auth-short-light-forgot-admin.png` | gate/setup/lock: forgot-admin | short-light, 1280x640 | after | copy says Mac login because the fixture uses the macOS desktop platform |
| `after/auth-short-light-signin-no-code.png` | gate/setup/lock: signin-no-code | short-light, 1280x640 | after |  |
| `before/signin-wide-light-code-after-change.png` | sign-in flow: code-after-change | wide-light, 1440x900 | before |  |
| `before/signin-wide-light-code-checking.png` | sign-in flow: code-checking | wide-light, 1440x900 | before |  |
| `before/signin-wide-light-code-expired-resend.png` | sign-in flow: code-expired-resend | wide-light, 1440x900 | before |  |
| `before/signin-wide-light-code-restored.png` | sign-in flow: code-restored | wide-light, 1440x900 | before |  |
| `before/signin-wide-light-code-too-many.png` | sign-in flow: code-too-many | wide-light, 1440x900 | before |  |
| `before/signin-wide-light-code-typing.png` | sign-in flow: code-typing | wide-light, 1440x900 | before |  |
| `before/signin-wide-light-code-wrong.png` | sign-in flow: code-wrong | wide-light, 1440x900 | before |  |
| `before/signin-wide-light-code.png` | sign-in flow: code | wide-light, 1440x900 | before |  |
| `before/signin-wide-light-email-after-change.png` | sign-in flow: email-after-change | wide-light, 1440x900 | before |  |
| `before/signin-wide-light-email-bad-address.png` | sign-in flow: email-bad-address | wide-light, 1440x900 | before |  |
| `before/signin-wide-light-email-busy.png` | sign-in flow: email-busy | wide-light, 1440x900 | before |  |
| `before/signin-wide-light-email-no-connection.png` | sign-in flow: email-no-connection | wide-light, 1440x900 | before |  |
| `before/signin-wide-light-email-not-available.png` | sign-in flow: email-not-available | wide-light, 1440x900 | before |  |
| `before/signin-wide-light-email-refused.png` | sign-in flow: email-refused | wide-light, 1440x900 | before |  |
| `before/signin-wide-light-email.png` | sign-in flow: email | wide-light, 1440x900 | before |  |
| `before/signin-wide-light-settings-account-sign-out-question.png` | sign-in flow: settings-account-sign-out-question | wide-light, 1440x900 | before |  |
| `before/signin-wide-light-settings-account-signed-in.png` | sign-in flow: settings-account-signed-in | wide-light, 1440x900 | before |  |
| `before/signin-wide-light-settings-account-signed-out.png` | sign-in flow: settings-account-signed-out | wide-light, 1440x900 | before |  |
| `before/auth-wide-light-forgot-admin.png` | gate/setup/lock: forgot-admin | wide-light, 1440x900 | before | copy says Mac login because the fixture uses the macOS desktop platform |
| `before/auth-wide-light-forgot-taxpayer.png` | gate/setup/lock: forgot-taxpayer | wide-light, 1440x900 | before |  |
| `before/auth-wide-light-gate-refused.png` | gate/setup/lock: gate-refused | wide-light, 1440x900 | before |  |
| `before/auth-wide-light-gate.png` | gate/setup/lock: gate | wide-light, 1440x900 | before |  |
| `before/auth-wide-light-setup-choose.png` | gate/setup/lock: setup-choose | wide-light, 1440x900 | before |  |
| `before/auth-wide-light-setup-key.png` | gate/setup/lock: setup-key | wide-light, 1440x900 | before |  |
| `before/auth-wide-light-setup-pin-error.png` | gate/setup/lock: setup-pin-error | wide-light, 1440x900 | before | this page uses a bottom Back link, not the top-left back control |
| `before/auth-wide-light-setup-solo-taxpayer.png` | gate/setup/lock: setup-solo-taxpayer | wide-light, 1440x900 | before |  |
| `before/auth-wide-light-signin-authenticator.png` | gate/setup/lock: signin-authenticator | wide-light, 1440x900 | before |  |
| `before/auth-wide-light-signin-error.png` | gate/setup/lock: signin-error | wide-light, 1440x900 | before |  |
| `before/auth-wide-light-signin-no-code.png` | gate/setup/lock: signin-no-code | wide-light, 1440x900 | before |  |
| `before/auth-wide-light-signin.png` | gate/setup/lock: signin | wide-light, 1440x900 | before |  |
| `signin-email-code-home-resize.mp4` | video: email to code to Home, tall-to-short resize | tall-light then short-light | after | screen recording of one real window |
