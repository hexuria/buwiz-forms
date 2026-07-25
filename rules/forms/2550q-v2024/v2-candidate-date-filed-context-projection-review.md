# 2550Q candidate `dateFiled` context projection review

## Scope

This review closes only the value-source identity for generated `dateFiled`.
It does not make any serialization artifact executable, select a production
clock, approve a timezone, authorize Final Copy, or open the reviewed registry.

## Official behavior established by pinned source

Package 7.9.6.0 generates `dateFiled` from the runtime's local calendar date
and formats the year, month, and day as zero-padded `YYYY/MM/DD`.
`saveXML(false)` and `saveXML(true)` append that value as the final pseudo-div
occurrence before their different markers. `saveEncryptedProfile(true)` writes
the same semantic value as standalone metadata after the final marker and
before invoking the opaque encryption helper.

The value is generated metadata, not an editable DOM control. It therefore
must not become a candidate field, must not be read from a raw input buffer,
and must not be confused with a previously imported artifact's
`Form2550QDraft.date_filed` value.

Evidence:

- `official-hta-runtime#saveXML:L5242`
- `official-hta-runtime#saveXMLsubmit:L5397`
- `official-hta-runtime#saveXML:L5516-L5628`
- `official-hta-runtime#saveEncryptedProfile:L5194-L5267`
- `candidate-plaintext-save-serialization-review#value-codec-boundary`
- `candidate-final-copy-serialization-review#date-and-marker-boundary`

## Candidate projection decision

The existing required date context value `local-current-date` is the reviewed
candidate value source for generated `dateFiled`. A future checked
serialization node must project that exact context value and apply the
artifact-specific `YYYY/MM/DD` semantic formatter and placement already
recorded in the binding inventory.

The caller must supply one explicit calendar date in the evaluation request.
Validation and any future serialization derived from the resulting trusted
evaluation must reuse that immutable context snapshot and its fingerprint.
They must not read the clock a second time; this prevents a validation near
local midnight from serializing a different date.

Missing, duplicate, or non-date context values continue to fail closed through
the existing context snapshot and declared-value checks. The projection does
not provide a fallback to the draft, operating-system clock, UTC, or a
hard-coded date.

## Clock and timezone boundary remains unresolved

This decision maps a declared context identity to generated metadata; it does
not decide how a production caller obtains that context. The official runtime
uses the machine-local calendar, but the application still lacks a reviewed
clock abstraction, timezone source, daylight-boundary policy, and custody
rule for recording when the value was captured.

Until those boundaries and every other serialization blocker are reviewed,
all three candidate artifacts remain `documented_only`, node-less, and
unmaterializable. The filing-safe branch remains `unresolved`, the reviewed
registry remains empty, and queue/transport authorization remains closed.
