# Evidence

- Installed HTA SHA-256: e7c7831e29cdd110a5cc325cb0bed7ee620684c21a483d59b25555c41d378a80; APPLICATIONNAME 1606.
- Official January 2018 PDF SHA-256: 374eca083888f36ae18612741d8473c61376db44cd281318def831c73dadabfe, valid PDF magic.
- Runtime help SHA-256: 152aa24f88b058d4b40ad89f2836eb8178ca2007446fbf41960611f047265c3b; content is 1606-specific, but HTA metadata incorrectly says APPLICATIONNAME 0605.
- Encrypted dummy final copy SHA-256: cda554d5014fbc6953aa128de55acb6ffcf5fab99fe6cc65e7f1a709576881e5; in-memory decrypted SHA-256 78b8a1b615fb2145bbd633b02dd77c1a6ce474329aa56fecb9b8c79c30e810ea; 99 unique keys; inventory SHA-256 01323df7e0eb8d81a5e9002ef834e8c19fda86c25870bf2fe73ff293fc627fbe; no values emitted.
- Runtime inventory: 120 controls, 101 serializer candidates, 99 unique static IDs, and no active indexed/add-more field families.
- No existing typed 1606 model was found under crates/bir-core/src/forms; repository behavior was therefore not used as substitute official evidence.