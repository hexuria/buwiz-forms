# Evidence

- BIR-Form1701v2018.hta: SHA-256 $((Get-FileHash -LiteralPath C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}\forms\BIR-Form1701v2018.hta -Algorithm SHA256).Hash.ToLowerInvariant()); application name 1701v2018, HTA version 1.0, printed header **January 2018 (ENCS)**.
- Reviewed plaintext save: SHA-256 168c7b3273d30a10f28f4653847519b876d5a88e77ed82911718a80f65c7827; exactly 837 unique keys and 	xtVersion=051414. Values are not copied.
- Reviewed encrypted companion: SHA-256 3771c99c191ef5e84b1b5e4c51499911bfbec6002febc3c53dca3f08730e92e3; in-memory DCPcrypt/zlib replay proves decrypted SHA-256 95ee42ed78f104335f50168a40e207f8af71ddf8eced9ddd0db1ad42d6366800, 838 unique keys, and the extra rm1701:txtPg1I9Address2. No decrypted values are written or copied.
- Main/attachment/consolidated January 2018 PDFs are pinned in manifest.json and independently locked by orm_1701.rs.
- The 837 plaintext keys exactly match the count of static serializer candidates. Two RDO fields are runtime-injected; two static candidates differ from the plaintext snapshot. The differences are retained in the runtime-control fixture.
- ddAttachment() replaces the literal template token TYPE with EXn or SPn, appends the controls into rmMain, and therefore makes 115 unbounded families serializable.