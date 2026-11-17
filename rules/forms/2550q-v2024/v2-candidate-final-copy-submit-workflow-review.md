# 2550Q v2024 candidate Final Copy and Submit workflow review

## Scope and pinned authority

This decision covers the combined `Submit / Final Copy` control and every
locally observable call made before the external online transport boundary.
No online submission was performed.

The authoritative form source is
`C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}\forms\BIR-Form2550Qv2024.hta`,
SHA-256
`3a5a4d3b2b342a4dfc55a05c69b560fb9be47af6451d8c651d19e1d33406ec70`.
The loaded shared JavaScript is
`C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}\js\string-util.js`,
SHA-256
`bc7f86f70bf993389a3a0135dcbd76c3e370c49d2eb95e2fc66ff318a2ebe43c`.
The loaded VBScript is
`C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}\js\eBIRTools.vbs`,
SHA-256
`7d0ceb5aad2c0eb90aeca189d6104ff05163ecd1820379f456125634ff7460f7`.

The external helpers reached by that source are:

- `C:\eBIRForms\Encrypt.exe`, 489,452 bytes, SHA-256
  `429337f44f84b93cd1095df48c8f3265e5ede7c646d1b48d9b80f4f92de74d2c`;
- `C:\eBIRForms\cFTPSend.exe`, 335,360 bytes, SHA-256
  `5d3dbda56e3ffffefb23f2fd46a5af0c0decc389d70921c453c3f813bb806262`.

The helper hashes pin identity only. They do not establish the helpers'
internal algorithms or authorize their execution.

## Exact pinned call graph

The button at HTA lines 3274-3276 is labelled `Submit / Final Copy` and calls
`openAlertEmail()`.

For an ordinary validated return whose `txtFinalFlag` is not `2`,
`openAlertEmail()` at lines 9679-9758:

1. initializes connection configuration from TIN and form identity;
2. applies save-file, amended-return, prior-version, and prior-final-copy
   checks;
3. asks the exact confirmation:

   ```text
   Please ensure that you have INTERNET access and a VALID email address is indicated in your tax return.

   Are you sure you want to submit?
   ```

4. calls `checkNetConnection()`; and
5. on its true branch resets `txtFinalFlag` to `0` (or changes `3` to `2`),
   initializes the enrollment controls, hides the form, and shows
   `ebirEnroll`.

The loaded `checkNetConnection()` at `string-util.js` lines 910-933 returns
`true` immediately. Its network probe is commented out. Therefore the false
branch at HTA lines 9751-9753, which would set `txtFinalFlag` to `3` and call
`saveEncryptedProfile()` without online transport, is source-present but
unreachable in this pinned runtime.

The enrollment and credential controls eventually call `sendEmail(sourceElement)`
at HTA lines 9878-10002. Before entering its transport `try` block, that
function:

1. performs the applicable credential-confirmation check;
2. calls `initializeOnSendEmail()`;
3. calls `saveEncryptedProfile(true)`;
4. derives the encrypted filename and submission metadata; and
5. only then calls `RenameAndSendFile(...)`.

`saveEncryptedProfile(true)` at HTA lines 5194-5267 first calls
`saveXML(true)`. The normal `saveXML(true)` path at lines 5411-5645 reruns
`initialValidateBeforeSave()`, applies filesystem/amended-version guards,
writes the editable save in current DOM order, adds pseudo-div `dateFiled`,
appends the final marker, and sets `gIsReadOnly = true`.

Only after that succeeds does `saveEncryptedProfile(true)` write the
`IAF_RDO_Copy` plaintext staging file in current DOM order, excluding button,
hidden, and undefined controls; append `All Rights Reserved BIR 2012.0`;
append standalone `<dateFiled>YYYY/MM/DD</dateFiled>` metadata; invoke
`EncryptFile`; disable the form; enable Upload; and return the absolute path.
`EncryptFile` at `eBIRTools.vbs` lines 96-109 synchronously launches the pinned
`Encrypt.exe` and returns its process exit code.

## Exact local prompts and alerts

The two non-amended prior-version guards are textually different and must not
be normalized:

- HTA line 9722 omits spaces around `or` after `Save`:

  ```text
  If your intention is to make an amended return, kindly tick Amended Return to 'Yes' before hitting 'Save'or 'Final Copy' or 'Submit'.
  ```

- HTA line 9734 includes those spaces:

  ```text
  If your intention is to make an amended return, kindly tick Amended Return to 'Yes' before hitting 'Save' or 'Final Copy' or 'Submit'.
  ```

The source-present false connectivity branch at lines 9751-9753 would emit:

```text
The system detected that you have no internet connection.
Please contact your internet service provider.
```

The credential flow uses these exact shared-runtime alerts:

```text
Please input Username and Password.
Username does not match from the previous dialog.
Password does not match from the previous dialog.
```

After staging and invoking encryption without checking its result,
`saveEncryptedProfile()` asks:

```text
File saved and encrypted.
Do you want to save this in USB flash drive or CD-RW?
```

The normal transport failure and exception branches at HTA lines 9951-9962
emit the same source string:

```text
Your Tax Return was not submitted online due to any of the 
 following reasons that may interrupt the submission process: 
 - No internet connection 
 - Slow internet connection 
 - Overly restrictive firewall
```

These strings are evidence, not executable notifications. The complete,
long-form success alert remains source-pinned at HTA line 9944 and was not
observed through an online submission.

## Validation-phase boundary

The combined workflow has two distinct validation obligations:

- opening the Final Copy/Submit enrollment flow consumes the still-current
  successful `validate` result that established the `validated` state; and
- the later Submit path performs a fresh `save`-phase preflight through
  `initialValidateBeforeSave()` before either artifact is written.

These results are not interchangeable. In particular, the pinned full
Validate path accepts RDO `000`, while Save preflight rejects it. A successful
Validate result alone therefore cannot authorize artifact materialization or
submission.

The current executable transition API binds one exact evaluated request/result
to one transition. It does not yet represent the additional filesystem,
amended-return, version, confirmation, credential, serialization, encryption,
and transport prerequisites in this call graph.

## Documented-only transition decision

The test-only v2 candidate records two non-executable semantic edges:

- `final-copy-open-enrollment`: `validated` plus action `final-copy`, bound to
  the prior `validate` evaluation, reaches `submission-enrollment` only after
  the source's non-rule guards and user confirmation; and
- `submit-after-enrollment`: `submission-enrollment` plus action `submit`,
  bound to a fresh `save` evaluation, reaches `submission-attempted`, whose
  success/failure outcome is deliberately not modeled.

Both official branches are `documented_only`. They carry no executable guard
or effects, cannot be selected by `transition_workflow`, and do not authorize
artifact creation, encryption, queueing, upload, FTP, or filing. The states
are descriptive source landmarks, not capability flags.

## Serialization boundary

The observed encrypted sample and source establish an artifact shape, but they
do not establish an executable serialization plan:

- the exact 159 pseudo-div order is available in the encrypted audit fixture;
- standalone `dateFiled` is outside that pseudo-div sequence;
- current DOM order, repeated-row occurrence identity, per-node codecs,
  final-marker bytes, filename/version rules, plaintext staging, and external
  encryption must all be represented before materialization is executable;
- the editable save's exact 160-pseudo-div order remains unproven by the
  lexicographically sorted v1 field inventory; and
- the external encryption algorithm is not inferred from a process call or a
  sample ciphertext.

The follow-up serialization review records a package-specific
`encrypted-final-copy` identity with a `documented_only` official branch and
an `unresolved` filing-safe branch. It contains no nodes and grants no
materialization authority.

## Source-static official defects

These findings are source-static assessments; they were not black-box online
submission tests.

1. `checkNetConnection()` always returns `true`, making the advertised local
   no-connection Final Copy branch unreachable. Classification:
   `incorrect-official-behavior`.
2. When `txtFinalFlag == "2"`, `openAlertEmail()` calls undefined
   `emailResend()`. The loaded shared runtime defines `reSendEmail()` at lines
   1092 onward, not `emailResend()`. Classification:
   `incorrect-official-behavior`.
3. `saveEncryptedProfile()` stores the `EncryptFile` exit code in `succ` but
   never checks it. It continues to the “File saved and encrypted” flow and
   returns the path even when the helper reports failure. Classification:
   `incorrect-official-behavior`.
4. If `saveXML(true)` fails, `saveEncryptedProfile(true)` returns
   `undefined`; `sendEmail()` dereferences it with `substring()` before its
   transport `try` block. Classification: `incorrect-official-behavior`.

Parity tooling must preserve these observations. Production filing-safe
behavior must not reproduce them without an explicit reviewed compatibility
decision.

## Filing-safe unresolved

No filing-safe decision has approved the combined button semantics, amended
and prior-version policy, confirmation wording, credential flow, raw artifact
contract, encryption provider, retry behavior, queue authorization, transport,
or success/failure state transitions. Both filing-safe branches remain
`unresolved`.

The reviewed registry remains empty. This evidence does not activate Final
Copy, Upload, Submit, queueing, transport, release status, or any production
capability.
