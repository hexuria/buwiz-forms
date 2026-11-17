# Audit

Coverage: 309 field keys, 80 ATC records, 38 rules, 11 calculations, seven workflow transitions, and 24 negative fixtures.

Confirmed defects and hazards:

1. TIN validation checks only nonblankness, not exact lengths/checksum.
2. Email validation checks only nonblankness.
3. Save uses incorrect Item 5/6 wording for TIN/RDO, while the printed form and Validate use Items 6/7.
4. Save preflight permits otherwise invalid drafts.
5. Multiple decimal points may enter numeric controls before inconsistent blur coercion.
6. The help HTML title says 1601EQ although its visible heading/content is 1601-FQ.
7. The current Private catalog exposes 41 slots while the representative save serializes 40 selection slots; maximum-union evidence preserves slot 41 explicitly.
8. Final Copy is coupled to submission and network handling.

All behavior is preserved as official-source evidence with separate recommendations. No online submission or UI write path was executed.
