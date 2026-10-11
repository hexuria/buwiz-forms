# 2550Q: lift the application freeze (user approved 2026-10-10)

Goal: 2550Q joins the generic submission queue like the other 42 forms, its sample is byte-identical to the official saveEncryptedProfile upload with a pinned Encrypt.exe hash, and the bir-rules-codegen gate/rules check pass with the freeze pins updated (not deleted wholesale).

## What the gate pins today (crates/bir-rules-codegen/src/status.rs)
- check_capability_matrix: 2550Q row ("2550Q","2024","2550Qv2024",1007,false) at ~1066. 1007 = all bits 0-9 except bit 4 queue_submission. Flip -> 1023, release_ready stays false.
- check_closed_2550q_actions (~1131-1170): QUEUE_SUBMISSION_SUPPORTED must be `pub const ... = false` (form_2550q.rs:22); transition_to_queued Err-only body (form_2550q.rs:1551-1557); desktop mark_submitted notification-only (form_2550q_view.rs:1655). Update these pins to the new, queue-capable shape (or retire this specific check) — keep the rest.
- check_2550q_transport_references (~1172-1192): background_cron.rs, official_import.rs, transport.rs must not name Form2550Q/form_2550q. Keep it that way: the worker dispatches via QueueableKind::from_form_code (background_cron.rs:1136), so don't name 2550Q there.
- KEEP intact: FROZEN_AUTHORITY_TOKENS inventory and the occurrence counts (~819-894: evaluate_trusted, preflight_active_form_submission, create_form_final_copy, reviewed_rule_set_entries...), check_desktop_evaluator_surface (view keeps exactly one .evaluate( and Form2550QDiagnosticState NotRun/Unavailable/Incomplete), check_closed_2550q_core_surface, the cfg(test) guards (~336-384). If queueing 2550Q would require a new production caller of any of those, STOP and report instead of changing the inventory.
- Tests that assert the freeze: support_level.rs:1201-1205 (queue_gate_is_semantic_and_registry_owned "2550Q stays frozen"), form_2550q.rs:3422-3428 (queue_transition_always_fails_closed), status.rs:3599-3628 (application_freeze_rejects_capability_and_queue_authority fixture strings), plus status tests application_freeze_is_a_held_boundary_criterion / application_freeze_rejects_new_production_authority_surfaces / landed_boundaries_are_all_held / application_freeze_ignores_direct_cfg_test_authority_exercises. Update them to the new truth; don't delete coverage of the authority-token inventory.

## Registry
crates/bir-core/src/forms/support_level.rs:349-363: add queue_submission: true. EXPECTED rows: take capability_bits from the gate's own "found" output.

## Generic queue (model on 2553)
- impl QueueableForm for Form2550QDraft (form_2553.rs:646-650 pattern: FORM_CODE, FORM_TYPE, LAYOUT_ID "2550q-v2024").
- QueueableKind::Form2550Q in crates/bir-core/src/forms/queueable.rs (~396), in QueueableKind::ALL (~443-484) and the with_queueable_kind! macro (~516).
- Desktop: 2550Q keeps its bespoke page (form_2550q_view.rs). Its queue/submit action should enqueue through the same generic path the generic pages use (see how views/queueable_forms.rs pages queue). Don't add 2550Q to form_view_specs! unless that is the only way; don't touch the page layout (a view-builder is making it responsive in parallel — touch only the submit/queue action code).
- QUEUE_SUBMISSION_SUPPORTED -> true; transition_to_queued real (mirror the generic lifecycle).

## Upload parity
- Real upload = saveEncryptedProfile(true) output; 2550Q appends <dateFiled>. Tooling: tools/official-xml (runtime.js <hta> <steps.json> > official.xml; see its README and setup/<id>.js). Official package HTAs: /tmp/claude-1000/-home-agent-workspace/75a3a4d5-f426-4ca2-af38-3fdce8d073d7/scratchpad/validation/package/forms/.
- Layout crates/bir-core/data/official-xml/2550q-v2024.json exists (no dateFiled key). Existing writers: form_2550q_xml.rs to_bir_xml_payload (#[cfg(test)]), try_to_bir_xml_payload, from_bir_xml_payload; dateFiled written at form_2550q_xml.rs:752-756.
- Add tests/sample_payloads/form_2550q.rs (pattern form_2553.rs: dummy_profile draft -> payload -> check_sample(stem, layout, &payload, "<sha256>")), register in sample_payloads/mod.rs, fixtures 2550Q-<period>.steps.json/.official.xml/.plain.xml/.iaf.xml. The payload the queue uploads must be the one the sample checks.
- Pin: leave sha as the value check_sample computes; a parity-checker will confirm it against the Encrypt.exe emulator.
