# Evidence

- Installed HTA SHA-256: f29a02e0a80fb4a72cc90046e2773d05ac631df2ec06845f4c885eabcabafca1; APPLICATIONNAME 1706.
- Official January 2018 PDF SHA-256: 5237ba69d5fae6a26dceffc8f39dfcab32fe7d57081bfba74dcf5c5550c1afa3, valid PDF magic.
- Runtime help SHA-256: ddeba297664de08d8862616d1161ad38eea563a62f8f088bb125e9054d472bbe; content is 1706-specific, but HTA metadata incorrectly says APPLICATIONNAME 0605.
- Encrypted dummy final copy SHA-256: 4764678faecfca0c8830d7f5262604683372629707226b9221a54123712626ce; in-memory decrypted SHA-256 eee8ff6ac46b4008186daaf8501186dc34f027f6470a40b2044035480a6c3f6d; 122 unique keys; inventory SHA-256 163cff842e04aa0df389997c1649ac1537467f21eafc8782166c40642a192ff3; no values emitted.
- Runtime inventory: 145 controls, 124 serializer candidates, 122 unique static IDs, and no active indexed/add-more field families.
- No existing typed 1706 model was found under crates/bir-core/src/forms; repository behavior was therefore not used as substitute official evidence.