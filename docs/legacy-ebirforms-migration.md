# Migrating from old eBIRForms

## Use the migration

On Windows, the Import Data screen looks for the standard `C:\eBIRForms`
installation (including the current system drive) and offers **Scan**. On
Windows, macOS, and Linux, use **Choose folder…** to select the installation
root, `profile\`, or `IAF_RDO_Copy\` folder. If the old Windows installation is
not available on your Mac or Linux machine, copy the `profile/` and
`IAF_RDO_Copy/` folders from its `C:\eBIRForms\` directory to removable storage
or another transfer location, then choose the copied root or either copied
folder.

Scanning reads profile XML and filed Final Copies without changing, moving, or
deleting any source files. Review the summary, then select **Migrate**. Taxpayer
profiles are created or filled only where the corresponding saved field is
empty; values already present in a profile are preserved. Filed returns are
stored as opaque historical records, including their original bytes. They do
not become drafts or submissions and are never sent to BIR. `savefile/` holds
plain drafts, so those files are counted in the preview but are not migrated.

The importer recognizes the `profile/`, `IAF_RDO_Copy/`, and `savefile/`
directories case-insensitively. Other installation folders, including
`IAF_RDC_Copy`, `IAF_Proj_Copy`, `*_Archive`, `temp`, `failed`, `success`, and
`logfile`, are ignored. The source installation can remain in place after the
migration.

## Legacy file formats

The official application is installed at `C:\eBIRForms\`; its HTAs resolve
`IAF_RDO_Copy\`, `profile\`, and `savefile\` relative to that root. Profile
files are named `<TIN9><branch>.xml`, where the branch is three to five digits.
They are plain text, with fields serialized as repeated-key elements such as
`<div>fn=VALUEfn=</div>`. The profile writer is `saveXML` in
`package/BIRForms.hta` (approximately lines 738–770). The official reader
extracts each value between the initial `KEY=` and its repeated closing
`KEY=`. It writes `fn`, `lob`, and `regAddr` using JavaScript `escape()`;
the importer decodes both `%XX` and `%uXXXX`. Confirmation fields are ignored.

Filed Final Copies use names of the form
`<TIN9><branch>-<FormIdentity>-<period>[V<n>][#<email>#].xml`. For example,
`123456788000-1601Cv2018-062025.xml` is an original, while
`123456788000-1601Cv2018-062025V2.xml` is amendment version 2. An optional
`#email#` suffix identifies an Internet-submission copy. The form identity is
resolved using the application's form capability registry when available;
unknown identities are still retained.

The form's `saveEncryptedProfile` routine (for example,
`forms/BIR-Form1601Cv2018.hta`, approximately lines 2031–2110) writes the
repeated-key `<div>` data followed by `All Rights Reserved BIR 2012.0` (older
files may use `All Rights Reserved BIR 2014.0`). `Encrypt.exe` then zlib
compresses and AES-encrypts the file in place. This migration uses the
cross-platform Rust `bir_core::crypto::decrypt_and_decompress` implementation;
it does not launch `Encrypt.exe`. An unencrypted Final Copy is recognized by
its `</div>` content, matching the official importer in `package/BIRForms.hta`
(approximately lines 1290–1330). A valid Final Copy must contain one of the
two trailer strings.

When a return has no corresponding profile file, the official importer derives
a profile from the return (`createProfileXML` in `package/BIRForms.hta`,
approximately line 1394 onward). It reads `txtTIN1/2/3`, `txtBranchCode`,
`txtTaxpayerName` (or `taxpayerName` on forms 2200P/S/T), `txtRDOCode`,
`txtLineBus`, `txtAddress`, `txtZipCode`, `txtTelNum`, and `txtEmail`. Form
field prefixes vary; migration matches the suffix after the final `:`. When
multiple returns can supply a profile, the latest period is used.
