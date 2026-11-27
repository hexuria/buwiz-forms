# A2 rehearsal: per-table content digests (M5)

Made by `cargo test -p bir-core --lib a2_rehearsal -- --ignored` at 2026-11-27 09:04 UTC. Digests by `scripts/a2_table_digests.py` (Python 3.9.6, its own sqlite3). Fixtures made by this build's own code (schema 24, v0.2.0's), in temporary folders.

## Shape 1: solo (owner PIN, four forms, filed + receipt, eBIRForms BLOB, year facts, overrides, dark theme) (PASS)

| table | before | after | equal |
|---|---|---|---|
| amendment_snapshots | a37c993ebadc | a37c993ebadc | yes |
| announcements | 3a84b434900b | 3a84b434900b | yes |
| app_alerts | 95b16f9829d1 | 95b16f9829d1 | yes |
| bir_notices | f270695fd7ad | f270695fd7ad | yes |
| data_providers | 1bd377fa1d64 | 1bd377fa1d64 | yes |
| form_drafts | 684641f2d7cf | 684641f2d7cf | yes |
| form_finalizations | 3a602e34dbc1 | 3a602e34dbc1 | yes |
| form_rule_migrations | 41aaf465e125 | 41aaf465e125 | yes |
| form_templates | e36a8ea30504 | e36a8ea30504 | yes |
| inbox_oauth_tokens | 7f4940e4ddf3 | 7f4940e4ddf3 | yes |
| job_queue | e07a995eaae8 | e07a995eaae8 | yes |
| legacy_filed_returns | 6b65e6b00577 | 6b65e6b00577 | yes |
| penalties_cache | 852eb072a4da | 852eb072a4da | yes |
| per_year_forms | b47abbfb21c1 | b47abbfb21c1 | yes |
| profile_calendar_events | 8243106005ad | 8243106005ad | yes |
| profile_calendar_links | 52907032ceff | 52907032ceff | yes |
| profiles | ff8a5ecbe7a5 | ff8a5ecbe7a5 | yes |
| settings | be61f81d6b06 | be61f81d6b06 | yes |
| setups | b80526df3635 | b80526df3635 | yes |
| sqlite_sequence | d0b76930ef76 | d0b76930ef76 | yes |
| submission_receipts | f541f8afc04c | f541f8afc04c | yes |
| submissions | cf2af87d7e4f | cf2af87d7e4f | yes |
| tax_deadlines | f78246c53fc6 | f78246c53fc6 | yes |
| taxpayers | b62eea3ead91 | b62eea3ead91 | yes |
| year_facts | 12e0a9598066 | 12e0a9598066 | yes |

- before: integrity_check "ok", user_version 24, rows 27, raw SHA-256 9ff581725d46
- after: integrity_check "ok", user_version 24, rows 27, raw SHA-256 2c64214187c5
- step-6 keys, before: {"buwiz_server_url":"https://old.example"}, after: {"hand_to_clients":"off"}
- restored by this build's Import: raw SHA-256 9ff581725d46 equal

## Shape 2: help desk (admin PIN, authenticator, recovery key, six clients) (PASS)

| table | before | after | equal |
|---|---|---|---|
| amendment_snapshots | a37c993ebadc | a37c993ebadc | yes |
| announcements | 3a84b434900b | 3a84b434900b | yes |
| app_alerts | 95b16f9829d1 | 95b16f9829d1 | yes |
| bir_notices | f270695fd7ad | f270695fd7ad | yes |
| data_providers | 1bd377fa1d64 | 1bd377fa1d64 | yes |
| form_drafts | beaa09515fca | beaa09515fca | yes |
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
| taxpayers | 714944960451 | 714944960451 | yes |
| year_facts | 9156dfb8d7ed | 9156dfb8d7ed | yes |

- before: integrity_check "ok", user_version 24, rows 25, raw SHA-256 056aa16564f7
- after: integrity_check "ok", user_version 24, rows 25, raw SHA-256 ce4fde2f54c3
- step-6 keys, before: {}, after: {"hand_to_clients":"on"}
- restored by this build's Import: raw SHA-256 056aa16564f7 equal

## Shape 3: no saved sign-in, then A (Not mine) and B (admin PIN) (PASS)

| table | before | after | equal |
|---|---|---|---|
| amendment_snapshots | a37c993ebadc | a37c993ebadc | yes |
| announcements | 3a84b434900b | 3a84b434900b | yes |
| app_alerts | 95b16f9829d1 | 95b16f9829d1 | yes |
| bir_notices | f270695fd7ad | f270695fd7ad | yes |
| data_providers | 1bd377fa1d64 | 1bd377fa1d64 | yes |
| form_drafts | 2396e55d75fa | 2396e55d75fa | yes |
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
| taxpayers | e14fac49d4d5 | e14fac49d4d5 | yes |
| year_facts | 9156dfb8d7ed | 9156dfb8d7ed | yes |

- before: integrity_check "ok", user_version 24, rows 25, raw SHA-256 ada68c0233b8
- after: integrity_check "ok", user_version 24, rows 25, raw SHA-256 61ec3191d2a0
- step-6 keys, before: {}, after: {"hand_to_clients":"on"}
- restored by this build's Import: raw SHA-256 ada68c0233b8 equal

## Shape 4: queued returns (three Queued, one Submitted awaiting its email) (PASS)

| table | before | after | equal |
|---|---|---|---|
| amendment_snapshots | a37c993ebadc | a37c993ebadc | yes |
| announcements | 3a84b434900b | 3a84b434900b | yes |
| app_alerts | 95b16f9829d1 | 95b16f9829d1 | yes |
| bir_notices | f270695fd7ad | f270695fd7ad | yes |
| data_providers | 1bd377fa1d64 | 1bd377fa1d64 | yes |
| form_drafts | f0a0588ef07b | f0a0588ef07b | yes |
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
| taxpayers | 44a0c18ffd43 | 44a0c18ffd43 | yes |
| year_facts | 9156dfb8d7ed | 9156dfb8d7ed | yes |

- before: integrity_check "ok", user_version 24, rows 18, raw SHA-256 49ec31b0681f
- after: integrity_check "ok", user_version 24, rows 18, raw SHA-256 6863dcc64366
- step-6 keys, before: {"buwiz_server_url":"https://old.example"}, after: {"hand_to_clients":"off"}
- restored by this build's Import: raw SHA-256 49ec31b0681f equal

## Shape 6: practice account first, then a real account opens the old lock (PASS)

| table | before | after | equal |
|---|---|---|---|
| amendment_snapshots | a37c993ebadc | a37c993ebadc | yes |
| announcements | 3a84b434900b | 3a84b434900b | yes |
| app_alerts | 95b16f9829d1 | 95b16f9829d1 | yes |
| bir_notices | f270695fd7ad | f270695fd7ad | yes |
| data_providers | 1bd377fa1d64 | 1bd377fa1d64 | yes |
| form_drafts | f9357273f227 | f9357273f227 | yes |
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
| taxpayers | a4b0cf645d18 | a4b0cf645d18 | yes |
| year_facts | 9156dfb8d7ed | 9156dfb8d7ed | yes |

- before: integrity_check "ok", user_version 24, rows 25, raw SHA-256 6ba35b06f75e
- after: integrity_check "ok", user_version 24, rows 25, raw SHA-256 4c0a96f11b54
- step-6 keys, before: {}, after: {"hand_to_clients":"on"}
- restored by this build's Import: raw SHA-256 6ba35b06f75e equal

## Shape 7: a v22-shaped file (this build's file with the v23/v24 tables dropped and user_version 22) (PASS)

| table | before | after | equal |
|---|---|---|---|
| amendment_snapshots | a37c993ebadc | a37c993ebadc | yes |
| announcements | 3a84b434900b | 3a84b434900b | yes |
| app_alerts | 95b16f9829d1 | 95b16f9829d1 | yes |
| bir_notices | f270695fd7ad | f270695fd7ad | yes |
| data_providers | 1bd377fa1d64 | 1bd377fa1d64 | yes |
| form_drafts | 5338efe0d0f5 | 5338efe0d0f5 | yes |
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
| taxpayers | f3fa86a98c53 | f3fa86a98c53 | yes |
| year_facts | 9156dfb8d7ed | 9156dfb8d7ed | yes |

- before: integrity_check "ok", user_version 24, rows 14, raw SHA-256 270263502772
- after: integrity_check "ok", user_version 24, rows 14, raw SHA-256 7064fed849ac
- step-6 keys, before: {"buwiz_server_url":"https://old.example"}, after: {"hand_to_clients":"off"}
- restored by this build's Import: raw SHA-256 d5fb6daeb0b6 equal

## Shape 8: solo with an authenticator and no PIN (PASS)

| table | before | after | equal |
|---|---|---|---|
| amendment_snapshots | a37c993ebadc | a37c993ebadc | yes |
| announcements | 3a84b434900b | 3a84b434900b | yes |
| app_alerts | 95b16f9829d1 | 95b16f9829d1 | yes |
| bir_notices | f270695fd7ad | f270695fd7ad | yes |
| data_providers | 1bd377fa1d64 | 1bd377fa1d64 | yes |
| form_drafts | 94b88d162e4a | 94b88d162e4a | yes |
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
| taxpayers | 83bd306db52e | 83bd306db52e | yes |
| year_facts | 9156dfb8d7ed | 9156dfb8d7ed | yes |

- before: integrity_check "ok", user_version 24, rows 14, raw SHA-256 c7214aabde8c
- after: integrity_check "ok", user_version 24, rows 14, raw SHA-256 743500bb8f72
- step-6 keys, before: {"buwiz_server_url":"https://old.example"}, after: {"hand_to_clients":"off"}
- restored by this build's Import: raw SHA-256 c7214aabde8c equal

## Shape 9a: a taxpayer with no access_mode (PASS)

| table | before | after | equal |
|---|---|---|---|
| amendment_snapshots | a37c993ebadc | a37c993ebadc | yes |
| announcements | 3a84b434900b | 3a84b434900b | yes |
| app_alerts | 95b16f9829d1 | 95b16f9829d1 | yes |
| bir_notices | f270695fd7ad | f270695fd7ad | yes |
| data_providers | 1bd377fa1d64 | 1bd377fa1d64 | yes |
| form_drafts | 7379dd1763c0 | 7379dd1763c0 | yes |
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
| taxpayers | e3ed051f7fc7 | e3ed051f7fc7 | yes |
| year_facts | 9156dfb8d7ed | 9156dfb8d7ed | yes |

- before: integrity_check "ok", user_version 24, rows 6, raw SHA-256 f07fa9c5f366
- after: integrity_check "ok", user_version 24, rows 6, raw SHA-256 ae7602229995
- step-6 keys, before: {}, after: {"hand_to_clients":"off"}
- restored by this build's Import: raw SHA-256 f07fa9c5f366 equal

## Shape 9b: solo with no admin_tin_root (PASS)

| table | before | after | equal |
|---|---|---|---|
| amendment_snapshots | a37c993ebadc | a37c993ebadc | yes |
| announcements | 3a84b434900b | 3a84b434900b | yes |
| app_alerts | 95b16f9829d1 | 95b16f9829d1 | yes |
| bir_notices | f270695fd7ad | f270695fd7ad | yes |
| data_providers | 1bd377fa1d64 | 1bd377fa1d64 | yes |
| form_drafts | 3a196cbec889 | 3a196cbec889 | yes |
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
| taxpayers | 7df6b40c3723 | 7df6b40c3723 | yes |
| year_facts | 9156dfb8d7ed | 9156dfb8d7ed | yes |

- before: integrity_check "ok", user_version 24, rows 13, raw SHA-256 196f1f6e85e7
- after: integrity_check "ok", user_version 24, rows 13, raw SHA-256 f6c86d3c76d4
- step-6 keys, before: {"buwiz_server_url":"https://old.example"}, after: {"hand_to_clients":"off"}
- restored by this build's Import: raw SHA-256 196f1f6e85e7 equal

## Shape 10: a file from an archive restore (foreign help-desk mode and PINs) with two safety files beside it (PASS)

| table | before | after | equal |
|---|---|---|---|
| amendment_snapshots | a37c993ebadc | a37c993ebadc | yes |
| announcements | 3a84b434900b | 3a84b434900b | yes |
| app_alerts | 95b16f9829d1 | 95b16f9829d1 | yes |
| bir_notices | f270695fd7ad | f270695fd7ad | yes |
| data_providers | 1bd377fa1d64 | 1bd377fa1d64 | yes |
| form_drafts | 7eb8466bc39c | 7eb8466bc39c | yes |
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
| taxpayers | c512729479d1 | c512729479d1 | yes |
| year_facts | 9156dfb8d7ed | 9156dfb8d7ed | yes |

- before: integrity_check "ok", user_version 24, rows 15, raw SHA-256 d941c41fbad0
- after: integrity_check "ok", user_version 24, rows 15, raw SHA-256 50dd8ae2a99d
- step-6 keys, before: {}, after: {"hand_to_clients":"on"}
- restored by this build's Import: raw SHA-256 d941c41fbad0 equal

## Shape 11a: an unversioned painted-era file (user_version 0) (PASS)

| table | before | after | equal |
|---|---|---|---|
| amendment_snapshots | a37c993ebadc | a37c993ebadc | yes |
| announcements | 3a84b434900b | 3a84b434900b | yes |
| app_alerts | 95b16f9829d1 | 95b16f9829d1 | yes |
| bir_notices | f270695fd7ad | f270695fd7ad | yes |
| data_providers | 1bd377fa1d64 | 1bd377fa1d64 | yes |
| form_drafts | 1815cc15cd46 | 1815cc15cd46 | yes |
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
| taxpayers | 0f59ffa06240 | 0f59ffa06240 | yes |
| year_facts | 9156dfb8d7ed | 9156dfb8d7ed | yes |

- before: integrity_check "ok", user_version 24, rows 6, raw SHA-256 59c1e544de25
- after: integrity_check "ok", user_version 24, rows 6, raw SHA-256 e56b557f1c56
- step-6 keys, before: {}, after: {"hand_to_clients":"off"}
- restored by this build's Import: raw SHA-256 d27c98930d47 equal

## Shape 12: a crash -wal with real commits in it (PASS)

| table | before | after | equal |
|---|---|---|---|
| amendment_snapshots | a37c993ebadc | a37c993ebadc | yes |
| announcements | 3a84b434900b | 3a84b434900b | yes |
| app_alerts | 95b16f9829d1 | 95b16f9829d1 | yes |
| bir_notices | f270695fd7ad | f270695fd7ad | yes |
| data_providers | 1bd377fa1d64 | 1bd377fa1d64 | yes |
| form_drafts | 9a976b96ad6f | 9a976b96ad6f | yes |
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
| taxpayers | 43ff99cac893 | 43ff99cac893 | yes |
| year_facts | 9156dfb8d7ed | 9156dfb8d7ed | yes |

- before: integrity_check "ok", user_version 24, rows 6, raw SHA-256 15ad3f5a5afa
- after: integrity_check "ok", user_version 24, rows 6, raw SHA-256 b6d2b1b7bfca
- step-6 keys, before: {}, after: {"hand_to_clients":"off"}
- restored by this build's Import: raw SHA-256 15ad3f5a5afa equal

## Shape 13: a debug build's bir_data.dev.db beside the real file (PASS)

| table | before | after | equal |
|---|---|---|---|
| amendment_snapshots | a37c993ebadc | a37c993ebadc | yes |
| announcements | 3a84b434900b | 3a84b434900b | yes |
| app_alerts | 95b16f9829d1 | 95b16f9829d1 | yes |
| bir_notices | f270695fd7ad | f270695fd7ad | yes |
| data_providers | 1bd377fa1d64 | 1bd377fa1d64 | yes |
| form_drafts | b60c51846f2d | b60c51846f2d | yes |
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
| taxpayers | 68ab208b36ba | 68ab208b36ba | yes |
| year_facts | 9156dfb8d7ed | 9156dfb8d7ed | yes |

- before: integrity_check "ok", user_version 24, rows 6, raw SHA-256 2c274a795e34
- after: integrity_check "ok", user_version 24, rows 6, raw SHA-256 fcbb21a610ad
- step-6 keys, before: {}, after: {"hand_to_clients":"off"}
- restored by this build's Import: raw SHA-256 2c274a795e34 equal

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
