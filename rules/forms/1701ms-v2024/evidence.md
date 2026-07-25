# Evidence â€” 1701-MS August 2024

The exact HTA, official fillable PDF, official guide, two dummy plaintext saves, package executable, and linked shared scripts are pinned in manifest.json. Both PDFs have %PDF- magic. Both saves contain the same 201 unique keys; values and the profile email are not copied.

Static inspection found 300 controls (296 with IDs) and 21 unbounded indexed input families across nine modal row groups. The source contains 20 validation-related functions with 64 alert sites and 71 calculation-related functions; their names, ranges, hashes, alerts, and referenced controls are retained in the function-inventory fixtures.

The HTA hard-codes seven ATC choices because the shared tcCodes.xml contains no 1701MS record. It also references ../js/lib/2200C.js, which is absent from the extraction tree.