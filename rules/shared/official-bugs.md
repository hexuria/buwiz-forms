# Shared official bugs and hazards

## TIN testing bypass

Offline eBIRForms 7.9.6.0 `string-util.js` lines 84-89 explicitly treats `999999999` as valid after calling the external checker. This is classified as `official-bug-compatible`; production behavior should not reproduce the bypass.

## External validation dependency

TIN checksum behavior delegates to `C:\eBIRForms\chkt.exe` through VBScript.
The installed 38,400-byte helper has SHA-256
`c00bd4131a725af53f48c6385d3332c4b789e15441bf52bbac73117c96c1b0ac`.
That identity pin does not reveal or prove its internal algorithm, which
remains a gap rather than an inferred rule.

## Combined Final Copy and online submission hazards

For 2550Q April 2024 in package 7.9.6.0, the loaded shared
`checkNetConnection()` returns `true` before its commented-out network probe,
making the form's no-connection local Final Copy branch unreachable.
The form's failed-submit branch calls undefined `emailResend()` while the
loaded shared function is named `reSendEmail()`. Its encryption wrapper also
ignores the external helper exit status and continues through success-facing
UI. These are classified as `incorrect-official-behavior`; source parity must
record them, while filing-safe behavior must fail closed until separately
reviewed.
