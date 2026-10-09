# Sample payloads (dummy data)

These are what our serializers produce for a fictitious taxpayer (TIN
123-456-788-00000). The app uploads exactly these bytes:
`background_cron` builds them with `try_to_bir_xml_payload` and encrypts them
with `crypto::compress_and_encrypt`.

| File | Contents |
|---|---|
| `*.plain.xml` | Plaintext BIR pseudo-XML: `<div>key=valuekey=</div>` per field |
| `*.iaf.xml` | The same payload encrypted, as uploaded (AES + zlib, `BIR_IAF_PASSPHRASE`) |

Decrypt an IAF file with:

```sh
cargo run -p bir-core --bin dump_xml -- crates/bir-core/tests/sample-payloads/1601C-062025.iaf.xml
```

`tests/sample_payloads_test.rs` fails if a serializer change alters these
files. If the change is intended, regenerate them with:

```sh
UPDATE_SAMPLE_PAYLOADS=1 cargo test -p bir-core --test sample_payloads_test
```

These are not byte-identical to what the official eBIRForms app writes.
Official `saveXML` emits fields in form order, `escape()`s only the name, line
of business and address (folding Address 2 into the `txtAddress` div), leaves
other values raw, and appends a trailer. Ours emits keys sorted and
percent-encodes every value.
