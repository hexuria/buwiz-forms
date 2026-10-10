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

Both steps are byte-identical to the official eBIRForms app:

- `*.official.xml` is what the official `saveXMLsubmit()` writes for the same
  values (`rules/tools/official-xml/oracle.js` runs the official HTA loop in
  jsdom). The test requires `*.plain.xml` to equal it.
- Running the official `Encrypt.exe` under emulation on `*.plain.xml` gives
  exactly the `*.iaf.xml` bytes; the test pins those hashes.

The plaintext follows each form's official layout
(`crates/bir-core/data/official-xml/<form_id>.json`): page order, `escape()` on
the fields the official code escapes, 2551Q's ten fields written twice,
1601C's Address 2 folded into `txtAddress`, the form's whitespace and the
`All Rights Reserved BIR 2012.0` trailer.
