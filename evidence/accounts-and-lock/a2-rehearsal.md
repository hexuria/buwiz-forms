# A2 rehearsal: per-table content digests (M5)

Made by `cargo test -p bir-core --lib a2_rehearsal -- --ignored` at 2026-11-26 22:41 UTC. Digests by `scripts/a2_table_digests.py` (Python 3.9.6, its own sqlite3). Fixtures made by this build's own code (schema 24, v0.2.0's), in temporary folders.

## Shape 1: solo (owner PIN, four forms, filed + receipt, eBIRForms BLOB, year facts, overrides, dark theme) (PASS)

| table | before | after | equal |
|---|---|---|---|
| amendment_snapshots | a37c993ebadc | a37c993ebadc | yes |
| announcements | 3a84b434900b | 3a84b434900b | yes |
| app_alerts | 95b16f9829d1 | 95b16f9829d1 | yes |
| bir_notices | f270695fd7ad | f270695fd7ad | yes |
| data_providers | 1bd377fa1d64 | 1bd377fa1d64 | yes |
| form_drafts | 54785329cc6b | 54785329cc6b | yes |
| form_finalizations | 3a602e34dbc1 | 3a602e34dbc1 | yes |
| form_rule_migrations | 41aaf465e125 | 41aaf465e125 | yes |
| form_templates | e36a8ea30504 | e36a8ea30504 | yes |
| inbox_oauth_tokens | 7f4940e4ddf3 | 7f4940e4ddf3 | yes |
| job_queue | e07a995eaae8 | e07a995eaae8 | yes |
| legacy_filed_returns | 8bd39a286a9d | 8bd39a286a9d | yes |
| penalties_cache | 852eb072a4da | 852eb072a4da | yes |
| per_year_forms | b47abbfb21c1 | b47abbfb21c1 | yes |
| profile_calendar_events | 8243106005ad | 8243106005ad | yes |
| profile_calendar_links | 52907032ceff | 52907032ceff | yes |
| profiles | ff8a5ecbe7a5 | ff8a5ecbe7a5 | yes |
| settings | be61f81d6b06 | be61f81d6b06 | yes |
| setups | b80526df3635 | b80526df3635 | yes |
| sqlite_sequence | d0b76930ef76 | d0b76930ef76 | yes |
| submission_receipts | 39c131c9379a | 39c131c9379a | yes |
| submissions | 8fecab395f0d | 8fecab395f0d | yes |
| tax_deadlines | f78246c53fc6 | f78246c53fc6 | yes |
| taxpayers | 371f88377ff9 | 371f88377ff9 | yes |
| year_facts | 76ecbded9a53 | 76ecbded9a53 | yes |

- before: integrity_check "ok", user_version 24, rows 27, raw SHA-256 62ebb49fcd01
- after: integrity_check "ok", user_version 24, rows 27, raw SHA-256 0538de14fb43
- step-6 keys, before: {"buwiz_server_url":"https://old.example"}, after: {"hand_to_clients":"off"}
- restored by this build's Import: raw SHA-256 62ebb49fcd01 equal

## Shape 2: help desk (admin PIN, authenticator, recovery key, six clients) (PASS)

| table | before | after | equal |
|---|---|---|---|
| amendment_snapshots | a37c993ebadc | a37c993ebadc | yes |
| announcements | 3a84b434900b | 3a84b434900b | yes |
| app_alerts | 95b16f9829d1 | 95b16f9829d1 | yes |
| bir_notices | f270695fd7ad | f270695fd7ad | yes |
| data_providers | 1bd377fa1d64 | 1bd377fa1d64 | yes |
| form_drafts | ca9812947930 | ca9812947930 | yes |
| form_finalizations | 3a602e34dbc1 | 3a602e34dbc1 | yes |
| form_rule_migrations | 41aaf465e125 | 41aaf465e125 | yes |
| form_templates | e36a8ea30504 | e36a8ea30504 | yes |
| inbox_oauth_tokens | 7f4940e4ddf3 | 7f4940e4ddf3 | yes |
| job_queue | e07a995eaae8 | e07a995eaae8 | yes |
| legacy_filed_returns | e41d087d4565 | e41d087d4565 | yes |
| penalties_cache | 852eb072a4da | 852eb072a4da | yes |
| per_year_forms | b47abbfb21c1 | b47abbfb21c1 | yes |
| profile_calendar_events | 8243106005ad | 8243106005ad | yes |
| profile_calendar_links | 52907032ceff | 52907032ceff | yes |
| profiles | 1abfeb8ec9b4 | 1abfeb8ec9b4 | yes |
| settings | 83c79d1b8762 | 83c79d1b8762 | yes |
| setups | b80526df3635 | b80526df3635 | yes |
| sqlite_sequence | db376d3cf403 | db376d3cf403 | yes |
| submission_receipts | 415ce7fc6a03 | 415ce7fc6a03 | yes |
| submissions | bcfd1aa11be6 | bcfd1aa11be6 | yes |
| tax_deadlines | f78246c53fc6 | f78246c53fc6 | yes |
| taxpayers | 2e2058220aad | 2e2058220aad | yes |
| year_facts | 9156dfb8d7ed | 9156dfb8d7ed | yes |

- before: integrity_check "ok", user_version 24, rows 25, raw SHA-256 5156ea319832
- after: integrity_check "ok", user_version 24, rows 25, raw SHA-256 68d41fdd3ad9
- step-6 keys, before: {}, after: {"hand_to_clients":"on"}
- restored by this build's Import: raw SHA-256 5156ea319832 equal

## Shape 3: no saved sign-in, then A (Not mine) and B (admin PIN) (PASS)

| table | before | after | equal |
|---|---|---|---|
| amendment_snapshots | a37c993ebadc | a37c993ebadc | yes |
| announcements | 3a84b434900b | 3a84b434900b | yes |
| app_alerts | 95b16f9829d1 | 95b16f9829d1 | yes |
| bir_notices | f270695fd7ad | f270695fd7ad | yes |
| data_providers | 1bd377fa1d64 | 1bd377fa1d64 | yes |
| form_drafts | ca9812947930 | ca9812947930 | yes |
| form_finalizations | 3a602e34dbc1 | 3a602e34dbc1 | yes |
| form_rule_migrations | 41aaf465e125 | 41aaf465e125 | yes |
| form_templates | e36a8ea30504 | e36a8ea30504 | yes |
| inbox_oauth_tokens | 7f4940e4ddf3 | 7f4940e4ddf3 | yes |
| job_queue | e07a995eaae8 | e07a995eaae8 | yes |
| legacy_filed_returns | e41d087d4565 | e41d087d4565 | yes |
| penalties_cache | 852eb072a4da | 852eb072a4da | yes |
| per_year_forms | b47abbfb21c1 | b47abbfb21c1 | yes |
| profile_calendar_events | 8243106005ad | 8243106005ad | yes |
| profile_calendar_links | 52907032ceff | 52907032ceff | yes |
| profiles | 1abfeb8ec9b4 | 1abfeb8ec9b4 | yes |
| settings | 83c79d1b8762 | 83c79d1b8762 | yes |
| setups | b80526df3635 | b80526df3635 | yes |
| sqlite_sequence | db376d3cf403 | db376d3cf403 | yes |
| submission_receipts | 415ce7fc6a03 | 415ce7fc6a03 | yes |
| submissions | bcfd1aa11be6 | bcfd1aa11be6 | yes |
| tax_deadlines | f78246c53fc6 | f78246c53fc6 | yes |
| taxpayers | 2e2058220aad | 2e2058220aad | yes |
| year_facts | 9156dfb8d7ed | 9156dfb8d7ed | yes |

- before: integrity_check "ok", user_version 24, rows 25, raw SHA-256 5156ea319832
- after: integrity_check "ok", user_version 24, rows 25, raw SHA-256 68d41fdd3ad9
- step-6 keys, before: {}, after: {"hand_to_clients":"on"}
- restored by this build's Import: raw SHA-256 5156ea319832 equal

## Shape 4: queued returns (three Queued, one Submitted awaiting its email) (PASS)

| table | before | after | equal |
|---|---|---|---|
| amendment_snapshots | a37c993ebadc | a37c993ebadc | yes |
| announcements | 3a84b434900b | 3a84b434900b | yes |
| app_alerts | 95b16f9829d1 | 95b16f9829d1 | yes |
| bir_notices | f270695fd7ad | f270695fd7ad | yes |
| data_providers | 1bd377fa1d64 | 1bd377fa1d64 | yes |
| form_drafts | 5eb5bded2709 | 5eb5bded2709 | yes |
| form_finalizations | 3a602e34dbc1 | 3a602e34dbc1 | yes |
| form_rule_migrations | 41aaf465e125 | 41aaf465e125 | yes |
| form_templates | e36a8ea30504 | e36a8ea30504 | yes |
| inbox_oauth_tokens | 7f4940e4ddf3 | 7f4940e4ddf3 | yes |
| job_queue | e07a995eaae8 | e07a995eaae8 | yes |
| legacy_filed_returns | e41d087d4565 | e41d087d4565 | yes |
| penalties_cache | 852eb072a4da | 852eb072a4da | yes |
| per_year_forms | b47abbfb21c1 | b47abbfb21c1 | yes |
| profile_calendar_events | 8243106005ad | 8243106005ad | yes |
| profile_calendar_links | 52907032ceff | 52907032ceff | yes |
| profiles | 9934d028eb1c | 9934d028eb1c | yes |
| settings | afeae7a50283 | afeae7a50283 | yes |
| setups | b80526df3635 | b80526df3635 | yes |
| sqlite_sequence | a024c8f8b363 | a024c8f8b363 | yes |
| submission_receipts | 415ce7fc6a03 | 415ce7fc6a03 | yes |
| submissions | bcfd1aa11be6 | bcfd1aa11be6 | yes |
| tax_deadlines | f78246c53fc6 | f78246c53fc6 | yes |
| taxpayers | cb7ba165a3ee | cb7ba165a3ee | yes |
| year_facts | 9156dfb8d7ed | 9156dfb8d7ed | yes |

- before: integrity_check "ok", user_version 24, rows 18, raw SHA-256 da549eeb18f0
- after: integrity_check "ok", user_version 24, rows 18, raw SHA-256 89bf519f11ad
- step-6 keys, before: {"buwiz_server_url":"https://old.example"}, after: {"hand_to_clients":"off"}
- restored by this build's Import: raw SHA-256 da549eeb18f0 equal

## Shape 6: practice account first, then a real account opens the old lock (PASS)

| table | before | after | equal |
|---|---|---|---|
| amendment_snapshots | a37c993ebadc | a37c993ebadc | yes |
| announcements | 3a84b434900b | 3a84b434900b | yes |
| app_alerts | 95b16f9829d1 | 95b16f9829d1 | yes |
| bir_notices | f270695fd7ad | f270695fd7ad | yes |
| data_providers | 1bd377fa1d64 | 1bd377fa1d64 | yes |
| form_drafts | 3b1f84c89fbd | 3b1f84c89fbd | yes |
| form_finalizations | 3a602e34dbc1 | 3a602e34dbc1 | yes |
| form_rule_migrations | 41aaf465e125 | 41aaf465e125 | yes |
| form_templates | e36a8ea30504 | e36a8ea30504 | yes |
| inbox_oauth_tokens | 7f4940e4ddf3 | 7f4940e4ddf3 | yes |
| job_queue | e07a995eaae8 | e07a995eaae8 | yes |
| legacy_filed_returns | e41d087d4565 | e41d087d4565 | yes |
| penalties_cache | 852eb072a4da | 852eb072a4da | yes |
| per_year_forms | b47abbfb21c1 | b47abbfb21c1 | yes |
| profile_calendar_events | 8243106005ad | 8243106005ad | yes |
| profile_calendar_links | 52907032ceff | 52907032ceff | yes |
| profiles | 1abfeb8ec9b4 | 1abfeb8ec9b4 | yes |
| settings | 83c79d1b8762 | 83c79d1b8762 | yes |
| setups | b80526df3635 | b80526df3635 | yes |
| sqlite_sequence | db376d3cf403 | db376d3cf403 | yes |
| submission_receipts | 415ce7fc6a03 | 415ce7fc6a03 | yes |
| submissions | bcfd1aa11be6 | bcfd1aa11be6 | yes |
| tax_deadlines | f78246c53fc6 | f78246c53fc6 | yes |
| taxpayers | 65d97ab030ba | 65d97ab030ba | yes |
| year_facts | 9156dfb8d7ed | 9156dfb8d7ed | yes |

- before: integrity_check "ok", user_version 24, rows 25, raw SHA-256 ac527764c595
- after: integrity_check "ok", user_version 24, rows 25, raw SHA-256 f97a97faceb2
- step-6 keys, before: {}, after: {"hand_to_clients":"on"}
- restored by this build's Import: raw SHA-256 ac527764c595 equal

## Shape 7: a v22-shaped file (this build's file with the v23/v24 tables dropped and user_version 22) (PASS)

| table | before | after | equal |
|---|---|---|---|
| amendment_snapshots | a37c993ebadc | a37c993ebadc | yes |
| announcements | 3a84b434900b | 3a84b434900b | yes |
| app_alerts | 95b16f9829d1 | 95b16f9829d1 | yes |
| bir_notices | f270695fd7ad | f270695fd7ad | yes |
| data_providers | 1bd377fa1d64 | 1bd377fa1d64 | yes |
| form_drafts | ac8c96217df9 | ac8c96217df9 | yes |
| form_finalizations | 3a602e34dbc1 | 3a602e34dbc1 | yes |
| form_rule_migrations | 41aaf465e125 | 41aaf465e125 | yes |
| form_templates | e36a8ea30504 | e36a8ea30504 | yes |
| inbox_oauth_tokens | 7f4940e4ddf3 | 7f4940e4ddf3 | yes |
| job_queue | e07a995eaae8 | e07a995eaae8 | yes |
| legacy_filed_returns | e41d087d4565 | e41d087d4565 | yes |
| penalties_cache | 852eb072a4da | 852eb072a4da | yes |
| per_year_forms | b47abbfb21c1 | b47abbfb21c1 | yes |
| profile_calendar_events | 8243106005ad | 8243106005ad | yes |
| profile_calendar_links | 52907032ceff | 52907032ceff | yes |
| profiles | 9934d028eb1c | 9934d028eb1c | yes |
| settings | afeae7a50283 | afeae7a50283 | yes |
| setups | b80526df3635 | b80526df3635 | yes |
| sqlite_sequence | 1a69edf3067a | 1a69edf3067a | yes |
| submission_receipts | 415ce7fc6a03 | 415ce7fc6a03 | yes |
| submissions | bcfd1aa11be6 | bcfd1aa11be6 | yes |
| tax_deadlines | f78246c53fc6 | f78246c53fc6 | yes |
| taxpayers | 2d7ebc755181 | 2d7ebc755181 | yes |
| year_facts | 9156dfb8d7ed | 9156dfb8d7ed | yes |

- before: integrity_check "ok", user_version 24, rows 14, raw SHA-256 d63d256fcbf0
- after: integrity_check "ok", user_version 24, rows 14, raw SHA-256 323b45ca82c7
- step-6 keys, before: {"buwiz_server_url":"https://old.example"}, after: {"hand_to_clients":"off"}
- restored by this build's Import: raw SHA-256 6f0398be5ad6 equal

## Shape 8: solo with an authenticator and no PIN (PASS)

| table | before | after | equal |
|---|---|---|---|
| amendment_snapshots | a37c993ebadc | a37c993ebadc | yes |
| announcements | 3a84b434900b | 3a84b434900b | yes |
| app_alerts | 95b16f9829d1 | 95b16f9829d1 | yes |
| bir_notices | f270695fd7ad | f270695fd7ad | yes |
| data_providers | 1bd377fa1d64 | 1bd377fa1d64 | yes |
| form_drafts | bcd849fdc83e | bcd849fdc83e | yes |
| form_finalizations | 3a602e34dbc1 | 3a602e34dbc1 | yes |
| form_rule_migrations | 41aaf465e125 | 41aaf465e125 | yes |
| form_templates | e36a8ea30504 | e36a8ea30504 | yes |
| inbox_oauth_tokens | 7f4940e4ddf3 | 7f4940e4ddf3 | yes |
| job_queue | e07a995eaae8 | e07a995eaae8 | yes |
| legacy_filed_returns | e41d087d4565 | e41d087d4565 | yes |
| penalties_cache | 852eb072a4da | 852eb072a4da | yes |
| per_year_forms | b47abbfb21c1 | b47abbfb21c1 | yes |
| profile_calendar_events | 8243106005ad | 8243106005ad | yes |
| profile_calendar_links | 52907032ceff | 52907032ceff | yes |
| profiles | 9934d028eb1c | 9934d028eb1c | yes |
| settings | afeae7a50283 | afeae7a50283 | yes |
| setups | b80526df3635 | b80526df3635 | yes |
| sqlite_sequence | 1a69edf3067a | 1a69edf3067a | yes |
| submission_receipts | 415ce7fc6a03 | 415ce7fc6a03 | yes |
| submissions | bcfd1aa11be6 | bcfd1aa11be6 | yes |
| tax_deadlines | f78246c53fc6 | f78246c53fc6 | yes |
| taxpayers | 2f98cb3e69f5 | 2f98cb3e69f5 | yes |
| year_facts | 9156dfb8d7ed | 9156dfb8d7ed | yes |

- before: integrity_check "ok", user_version 24, rows 14, raw SHA-256 0c43109a9383
- after: integrity_check "ok", user_version 24, rows 14, raw SHA-256 248925f5912f
- step-6 keys, before: {"buwiz_server_url":"https://old.example"}, after: {"hand_to_clients":"off"}
- restored by this build's Import: raw SHA-256 0c43109a9383 equal

## Shape 9a: a taxpayer with no access_mode (PASS)

| table | before | after | equal |
|---|---|---|---|
| amendment_snapshots | a37c993ebadc | a37c993ebadc | yes |
| announcements | 3a84b434900b | 3a84b434900b | yes |
| app_alerts | 95b16f9829d1 | 95b16f9829d1 | yes |
| bir_notices | f270695fd7ad | f270695fd7ad | yes |
| data_providers | 1bd377fa1d64 | 1bd377fa1d64 | yes |
| form_drafts | 42447be65f62 | 42447be65f62 | yes |
| form_finalizations | 3a602e34dbc1 | 3a602e34dbc1 | yes |
| form_rule_migrations | 41aaf465e125 | 41aaf465e125 | yes |
| form_templates | e36a8ea30504 | e36a8ea30504 | yes |
| inbox_oauth_tokens | 7f4940e4ddf3 | 7f4940e4ddf3 | yes |
| job_queue | e07a995eaae8 | e07a995eaae8 | yes |
| legacy_filed_returns | e41d087d4565 | e41d087d4565 | yes |
| penalties_cache | 852eb072a4da | 852eb072a4da | yes |
| per_year_forms | b47abbfb21c1 | b47abbfb21c1 | yes |
| profile_calendar_events | 8243106005ad | 8243106005ad | yes |
| profile_calendar_links | 52907032ceff | 52907032ceff | yes |
| profiles | 4b60256c4f27 | 4b60256c4f27 | yes |
| settings | 210d9b3d3d22 | 210d9b3d3d22 | yes |
| setups | b80526df3635 | b80526df3635 | yes |
| sqlite_sequence | 1bdb85dd19b0 | 1bdb85dd19b0 | yes |
| submission_receipts | 415ce7fc6a03 | 415ce7fc6a03 | yes |
| submissions | bcfd1aa11be6 | bcfd1aa11be6 | yes |
| tax_deadlines | f78246c53fc6 | f78246c53fc6 | yes |
| taxpayers | a7487891c108 | a7487891c108 | yes |
| year_facts | 9156dfb8d7ed | 9156dfb8d7ed | yes |

- before: integrity_check "ok", user_version 24, rows 6, raw SHA-256 93b9967e9676
- after: integrity_check "ok", user_version 24, rows 6, raw SHA-256 27f8834208c8
- step-6 keys, before: {}, after: {"hand_to_clients":"off"}
- restored by this build's Import: raw SHA-256 93b9967e9676 equal

## Shape 9b: solo with no admin_tin_root (PASS)

| table | before | after | equal |
|---|---|---|---|
| amendment_snapshots | a37c993ebadc | a37c993ebadc | yes |
| announcements | 3a84b434900b | 3a84b434900b | yes |
| app_alerts | 95b16f9829d1 | 95b16f9829d1 | yes |
| bir_notices | f270695fd7ad | f270695fd7ad | yes |
| data_providers | 1bd377fa1d64 | 1bd377fa1d64 | yes |
| form_drafts | 7b6873763464 | 7b6873763464 | yes |
| form_finalizations | 3a602e34dbc1 | 3a602e34dbc1 | yes |
| form_rule_migrations | 41aaf465e125 | 41aaf465e125 | yes |
| form_templates | e36a8ea30504 | e36a8ea30504 | yes |
| inbox_oauth_tokens | 7f4940e4ddf3 | 7f4940e4ddf3 | yes |
| job_queue | e07a995eaae8 | e07a995eaae8 | yes |
| legacy_filed_returns | e41d087d4565 | e41d087d4565 | yes |
| penalties_cache | 852eb072a4da | 852eb072a4da | yes |
| per_year_forms | b47abbfb21c1 | b47abbfb21c1 | yes |
| profile_calendar_events | 8243106005ad | 8243106005ad | yes |
| profile_calendar_links | 52907032ceff | 52907032ceff | yes |
| profiles | 9934d028eb1c | 9934d028eb1c | yes |
| settings | 822652fd1609 | 822652fd1609 | yes |
| setups | b80526df3635 | b80526df3635 | yes |
| sqlite_sequence | 1a69edf3067a | 1a69edf3067a | yes |
| submission_receipts | 415ce7fc6a03 | 415ce7fc6a03 | yes |
| submissions | bcfd1aa11be6 | bcfd1aa11be6 | yes |
| tax_deadlines | f78246c53fc6 | f78246c53fc6 | yes |
| taxpayers | 6dd88dac75ce | 6dd88dac75ce | yes |
| year_facts | 9156dfb8d7ed | 9156dfb8d7ed | yes |

- before: integrity_check "ok", user_version 24, rows 13, raw SHA-256 e16daa5eb556
- after: integrity_check "ok", user_version 24, rows 13, raw SHA-256 a5f46b760c9e
- step-6 keys, before: {"buwiz_server_url":"https://old.example"}, after: {"hand_to_clients":"off"}
- restored by this build's Import: raw SHA-256 e16daa5eb556 equal

## Shape 10: a file from an archive restore (foreign help-desk mode and PINs) with two safety files beside it (PASS)

| table | before | after | equal |
|---|---|---|---|
| amendment_snapshots | a37c993ebadc | a37c993ebadc | yes |
| announcements | 3a84b434900b | 3a84b434900b | yes |
| app_alerts | 95b16f9829d1 | 95b16f9829d1 | yes |
| bir_notices | f270695fd7ad | f270695fd7ad | yes |
| data_providers | 1bd377fa1d64 | 1bd377fa1d64 | yes |
| form_drafts | 9dcfd1b16f53 | 9dcfd1b16f53 | yes |
| form_finalizations | 3a602e34dbc1 | 3a602e34dbc1 | yes |
| form_rule_migrations | 41aaf465e125 | 41aaf465e125 | yes |
| form_templates | e36a8ea30504 | e36a8ea30504 | yes |
| inbox_oauth_tokens | 7f4940e4ddf3 | 7f4940e4ddf3 | yes |
| job_queue | e07a995eaae8 | e07a995eaae8 | yes |
| legacy_filed_returns | e41d087d4565 | e41d087d4565 | yes |
| penalties_cache | 852eb072a4da | 852eb072a4da | yes |
| per_year_forms | b47abbfb21c1 | b47abbfb21c1 | yes |
| profile_calendar_events | 8243106005ad | 8243106005ad | yes |
| profile_calendar_links | 52907032ceff | 52907032ceff | yes |
| profiles | 13ae5e76dc91 | 13ae5e76dc91 | yes |
| settings | ea1c5c0cf3b0 | ea1c5c0cf3b0 | yes |
| setups | b80526df3635 | b80526df3635 | yes |
| sqlite_sequence | bd1c84e178ba | bd1c84e178ba | yes |
| submission_receipts | 415ce7fc6a03 | 415ce7fc6a03 | yes |
| submissions | bcfd1aa11be6 | bcfd1aa11be6 | yes |
| tax_deadlines | f78246c53fc6 | f78246c53fc6 | yes |
| taxpayers | da3c7d51a378 | da3c7d51a378 | yes |
| year_facts | 9156dfb8d7ed | 9156dfb8d7ed | yes |

- before: integrity_check "ok", user_version 24, rows 15, raw SHA-256 104ca070f144
- after: integrity_check "ok", user_version 24, rows 15, raw SHA-256 ae2e2f0266e9
- step-6 keys, before: {}, after: {"hand_to_clients":"on"}
- restored by this build's Import: raw SHA-256 104ca070f144 equal

## Shape 11a: an unversioned painted-era file (user_version 0) (PASS)

| table | before | after | equal |
|---|---|---|---|
| amendment_snapshots | a37c993ebadc | a37c993ebadc | yes |
| announcements | 3a84b434900b | 3a84b434900b | yes |
| app_alerts | 95b16f9829d1 | 95b16f9829d1 | yes |
| bir_notices | f270695fd7ad | f270695fd7ad | yes |
| data_providers | 1bd377fa1d64 | 1bd377fa1d64 | yes |
| form_drafts | adc01401d2ba | adc01401d2ba | yes |
| form_finalizations | 3a602e34dbc1 | 3a602e34dbc1 | yes |
| form_rule_migrations | 41aaf465e125 | 41aaf465e125 | yes |
| form_templates | e36a8ea30504 | e36a8ea30504 | yes |
| inbox_oauth_tokens | 7f4940e4ddf3 | 7f4940e4ddf3 | yes |
| job_queue | e07a995eaae8 | e07a995eaae8 | yes |
| legacy_filed_returns | e41d087d4565 | e41d087d4565 | yes |
| penalties_cache | 852eb072a4da | 852eb072a4da | yes |
| per_year_forms | b47abbfb21c1 | b47abbfb21c1 | yes |
| profile_calendar_events | 8243106005ad | 8243106005ad | yes |
| profile_calendar_links | 52907032ceff | 52907032ceff | yes |
| profiles | 4b60256c4f27 | 4b60256c4f27 | yes |
| settings | 210d9b3d3d22 | 210d9b3d3d22 | yes |
| setups | b80526df3635 | b80526df3635 | yes |
| sqlite_sequence | 1bdb85dd19b0 | 1bdb85dd19b0 | yes |
| submission_receipts | 415ce7fc6a03 | 415ce7fc6a03 | yes |
| submissions | bcfd1aa11be6 | bcfd1aa11be6 | yes |
| tax_deadlines | f78246c53fc6 | f78246c53fc6 | yes |
| taxpayers | 0dd35f2f76be | 0dd35f2f76be | yes |
| year_facts | 9156dfb8d7ed | 9156dfb8d7ed | yes |

- before: integrity_check "ok", user_version 24, rows 6, raw SHA-256 19613c722276
- after: integrity_check "ok", user_version 24, rows 6, raw SHA-256 3d63b6e21945
- step-6 keys, before: {}, after: {"hand_to_clients":"off"}
- restored by this build's Import: raw SHA-256 65d9d2d9841f equal

## Shape 12: a crash -wal with real commits in it (PASS)

| table | before | after | equal |
|---|---|---|---|
| amendment_snapshots | a37c993ebadc | a37c993ebadc | yes |
| announcements | 3a84b434900b | 3a84b434900b | yes |
| app_alerts | 95b16f9829d1 | 95b16f9829d1 | yes |
| bir_notices | f270695fd7ad | f270695fd7ad | yes |
| data_providers | 1bd377fa1d64 | 1bd377fa1d64 | yes |
| form_drafts | 914ecba007d2 | 914ecba007d2 | yes |
| form_finalizations | 3a602e34dbc1 | 3a602e34dbc1 | yes |
| form_rule_migrations | 41aaf465e125 | 41aaf465e125 | yes |
| form_templates | e36a8ea30504 | e36a8ea30504 | yes |
| inbox_oauth_tokens | 7f4940e4ddf3 | 7f4940e4ddf3 | yes |
| job_queue | e07a995eaae8 | e07a995eaae8 | yes |
| legacy_filed_returns | e41d087d4565 | e41d087d4565 | yes |
| penalties_cache | 852eb072a4da | 852eb072a4da | yes |
| per_year_forms | b47abbfb21c1 | b47abbfb21c1 | yes |
| profile_calendar_events | 8243106005ad | 8243106005ad | yes |
| profile_calendar_links | 52907032ceff | 52907032ceff | yes |
| profiles | f32570333cda | f32570333cda | yes |
| settings | 210d9b3d3d22 | 210d9b3d3d22 | yes |
| setups | b80526df3635 | b80526df3635 | yes |
| sqlite_sequence | 1bdb85dd19b0 | 1bdb85dd19b0 | yes |
| submission_receipts | 415ce7fc6a03 | 415ce7fc6a03 | yes |
| submissions | bcfd1aa11be6 | bcfd1aa11be6 | yes |
| tax_deadlines | f78246c53fc6 | f78246c53fc6 | yes |
| taxpayers | 0d2c45e86af7 | 0d2c45e86af7 | yes |
| year_facts | 9156dfb8d7ed | 9156dfb8d7ed | yes |

- before: integrity_check "ok", user_version 24, rows 6, raw SHA-256 4f2d7401f390
- after: integrity_check "ok", user_version 24, rows 6, raw SHA-256 41fef03199fb
- step-6 keys, before: {}, after: {"hand_to_clients":"off"}
- restored by this build's Import: raw SHA-256 4f2d7401f390 equal

## Shape 13: a debug build's bir_data.dev.db beside the real file (PASS)

| table | before | after | equal |
|---|---|---|---|
| amendment_snapshots | a37c993ebadc | a37c993ebadc | yes |
| announcements | 3a84b434900b | 3a84b434900b | yes |
| app_alerts | 95b16f9829d1 | 95b16f9829d1 | yes |
| bir_notices | f270695fd7ad | f270695fd7ad | yes |
| data_providers | 1bd377fa1d64 | 1bd377fa1d64 | yes |
| form_drafts | a1b2caa6e76f | a1b2caa6e76f | yes |
| form_finalizations | 3a602e34dbc1 | 3a602e34dbc1 | yes |
| form_rule_migrations | 41aaf465e125 | 41aaf465e125 | yes |
| form_templates | e36a8ea30504 | e36a8ea30504 | yes |
| inbox_oauth_tokens | 7f4940e4ddf3 | 7f4940e4ddf3 | yes |
| job_queue | e07a995eaae8 | e07a995eaae8 | yes |
| legacy_filed_returns | e41d087d4565 | e41d087d4565 | yes |
| penalties_cache | 852eb072a4da | 852eb072a4da | yes |
| per_year_forms | b47abbfb21c1 | b47abbfb21c1 | yes |
| profile_calendar_events | 8243106005ad | 8243106005ad | yes |
| profile_calendar_links | 52907032ceff | 52907032ceff | yes |
| profiles | 4b60256c4f27 | 4b60256c4f27 | yes |
| settings | 210d9b3d3d22 | 210d9b3d3d22 | yes |
| setups | b80526df3635 | b80526df3635 | yes |
| sqlite_sequence | 1bdb85dd19b0 | 1bdb85dd19b0 | yes |
| submission_receipts | 415ce7fc6a03 | 415ce7fc6a03 | yes |
| submissions | bcfd1aa11be6 | bcfd1aa11be6 | yes |
| tax_deadlines | f78246c53fc6 | f78246c53fc6 | yes |
| taxpayers | 69c6348e67a8 | 69c6348e67a8 | yes |
| year_facts | 9156dfb8d7ed | 9156dfb8d7ed | yes |

- before: integrity_check "ok", user_version 24, rows 6, raw SHA-256 9f0a49f4be0e
- after: integrity_check "ok", user_version 24, rows 6, raw SHA-256 450fdb9d248f
- step-6 keys, before: {}, after: {"hand_to_clients":"off"}
- restored by this build's Import: raw SHA-256 9f0a49f4be0e equal

## Shape 11b: an encrypted painted-era file (PASS)

- permanent (not plain SQLite): true; file byte for byte: true; no room made by the claim; the note: Tax profiles from an earlier version of eBIRForms could not be moved (the file is encrypted by an older version, or is not a database). They are still on this device, unchanged. Write to support@buwiz.com.

## Shape 14: power loss at each step (in-process cut after each fsync boundary, shape 1) (PASS)

- cut after AfterUnlockRecorded: cut; resumed to Done with every table equal to its before, one zip: true
- cut after AfterZipWritten: cut; resumed to Done with every table equal to its before, one zip: true
- cut after AfterCopyRecorded: cut; resumed to Done with every table equal to its before, one zip: true
- cut after AfterRoomDir: cut; resumed to Done with every table equal to its before, one zip: true
- cut after AfterLink: cut; resumed to Done with every table equal to its before, one zip: true
- cut after AfterUnlink: cut; resumed to Done with every table equal to its before, one zip: true
- cut after AfterSafetyFiles: cut; resumed to Done with every table equal to its before, one zip: true
- cut after AfterMoveRecorded: cut; resumed to Done with every table equal to its before, one zip: true
- cut after AfterRoomRow: cut; resumed to Done with every table equal to its before, one zip: true
- cut after AfterHashChecked: cut; resumed to Done with every table equal to its before, one zip: true
- cut after AfterFixups: cut; resumed to Done with every table equal to its before, one zip: true
- cut after AfterOpenChecked: cut; resumed to Done with every table equal to its before, one zip: true

## Not run here, and where it runs

- **Done here: the undo by v0.2.0's own code.** `scripts/a2_old_release_restore.sh v0.2.0 <zips> <work>` builds a test from the v0.2.0 tag that restores every zip this run kept (`A2_REHEARSAL_ZIPS`) with that release's own `import::stage_database_zip` (which runs its `check_restorable_database`) and `StagedDatabase::install`, the two calls its `restore_staged_database` makes: each restored file's raw SHA-256 equals the recorded `raw_sha256`, and v0.2.0 opens it. Output: `a2-rehearsal-v0.2.0-restore.txt` beside this file.
- **Still for a Devin Mac with the v0.2.0 build.** "Before" here is this build's open of the copy (the same schema, 24, and the same migrations as v0.2.0); the plan's column taken by the v0.2.0 binary's own open, and the restore through v0.2.0's Settings › Import screen.
- **v0.1.0 (shape 7).** v0.1.0 was the SQLCipher build: its file is encrypted and lives in another folder (macOS `~/Library/Group Containers/group.dev.goldcoders.bir`, Windows `%LOCALAPPDATA%\Taxman\eBIRForms`, Linux `~/.taxman-ebir`; crates/bir-core/src/platform/*.rs, `cfg(feature = "sqlcipher")`), never the plain build's `dev.goldcoders.bir` folder that the claim reads. So no v0.1.0 install reaches the claim as a schema-22 file; shape 7 here is a v22-shaped plain file made by this build. A v0.1.0 backup zip restored by v0.2.0 is already schema 24 by the time the claim sees it.
- **Shapes 4 and 5 (sends).** The three Queued returns going out exactly once each under their own account, and a send killed mid-pipe at the upgrade, need the running app, the worker and the mock relay's log: the A.9 device proof (and A3b's binding), after A3a adds the call sites.
- **Shape 14 on a real disk.** The VM power cut (`echo b > /proc/sysrq-trigger`) at each step runs on the Linux sandbox.
- **Phones.** Shape 1 on the iOS simulator and the Android emulator (their `data_dir()` paths).
