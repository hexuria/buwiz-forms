# Job 42b — sign-in look, keyboard up, at 1cf908a6

Build: debug APK x86_64 at 1cf908a651db63ed204cb5485cb269b2c05f43c6, kit pin 8c78cfd6, docs/relay.md dev recipe, folder target-1cf908a6. APK sha256 (first 16): a737e6e2d6ebfa51. BIR_AUTH_BYPASS confirmed ABSENT — the string `BIR_AUTH_BYPASS=1` does not appear anywhere in the packaged lib. EBIR_TEST_ENV=1, BUWIZ_NO_KEYCHAIN=1, BIR_SFTP_DRY_RUN=1 baked. Mock account server on 3008 with adb reverse (job 39's, --auto-approve, still up). Network cut. pm clear before install — no lock.

## PARTIAL — what was captured before the emulator went unresponsive

| picture | state | checks |
|---|---|---|
| light-email-empty-kb.png | email page, keyboard up, empty | logo whole with its words, nothing cut under the Back row; no large empty gap; field and Continue in view — PASS unless noted in the report |
| light-email-baderr-kb.png | email page, keyboard up, bad address with error | same — PASS unless noted |

NOT captured: the dark versions, the code page, the TIN+PIN page. After the two light shots the valid-email continue stalled, and minutes later the emulator itself showed "Process system isn't responding" — the session machine ran out of budget with the emulator in that state. The dark-email-baderr-kb.png accidentally captured that ANR dialog and is omitted as worthless.
