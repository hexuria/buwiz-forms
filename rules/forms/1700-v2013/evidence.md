# Evidence

- Exact HTA SHA-256: d025bb0743123dc0dfdf8251da18c42cc7827c5c571f5b06ff8f39a62f8437ba; APPLICATIONNAME 1700, version 4.7, printed June 2013 ENCS.
- Official help SHA-256: 343726a2a1463905151e3de1f8025f8763c2998f6a8afee8917db53b5b4f7ca8; it identifies the same revision, instructions, tax table, deadline, and attachments.
- The reviewed plaintext save has 311 unique keys and field-inventory SHA-256 4821dd338ebd6c3a73d706db1ff73f7cc7e6115a15d65893f25d6e760a699904.
- The encrypted companion SHA-256 6fbedb576e641f0a66a84bdf3f3bc273f3beeb2c9e5a76494cb6c10460c208ac replays in memory to decrypted SHA-256 1cbbf61b9038b03e41f81edb3976b1e4360353aad605a86589cbefc36687cc51, the identical 311-key inventory, and emits no values.
- Shared js/lib/1700.js SHA-256: 49b5603bed5a87f94c6d9bfeb46da7399f1bbbe7ffd429cd81dc41bc46404fee; it provides population/transport mapping while validation/calculations are inline in the HTA.
- Validation ordering: Save uses four checks and displays only the first collected error. Validate calls mandatoryFields and then validateAll; both stop on the first failing branch. Blur/modal handlers execute independently.
- The separate January 2018 1700v2018 HTA/PDF is not evidence for this June 2013 package.
