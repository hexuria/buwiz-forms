# Migrating from old eBIRForms

## Use the migration

In the eBIRForms app (the NSH desktop app), open **Settings > Import / Export**.
Under **Import data** is **Import from BIR eBIRForms**:

- **Choose folder…** opens the computer's own folder dialog. Pick the old
  program's installation folder (on Windows normally `C:\eBIRForms`), or its
  `profile\` or `IAF_RDO_Copy\` folder. This works on Windows, macOS and
  Linux. If the old Windows installation is not on this computer, copy the
  `profile/`, `IAF_RDO_Copy/` (and, if you like, `savefile/`) folders from
  its `C:\eBIRForms\` directory to removable storage or another transfer
  location, then choose the copied root or either copied folder.
- On Windows, when the standard folder exists (`C:\eBIRForms`, also on the
  current system drive), it is listed as **Found on this computer** with its
  path and a **Scan** button.

Scanning reads profile XML and filed Final Copies without changing, moving, or
deleting any source files. The card then shows a review: how many taxpayers
and filed returns were found, how many saved drafts (`savefile/`) were counted
but will not be imported, and every file that was not read, with its name and
the reason. A folder that is not an eBIRForms folder, or one with no taxpayer
profile and no filed return in it, says so instead, and nothing can be
imported from it.

Select **Import** to bring in exactly what was reviewed, or **Cancel** to drop
it. If the folder (or the open database) changed after the scan, the import is
refused and asks for a new scan. Taxpayer profiles are created or filled only
where the corresponding saved field is empty; values already present in a
profile are preserved. Filed returns are stored as opaque historical records,
including their original bytes. They do not become drafts or submissions and
are never sent to BIR. After the import the card shows what happened
(taxpayers added, filled in, already there; filed returns added, already
there; files not read) and a short message confirms it. Importing the same folder
again adds nothing new.

Where the returns show: open **Filings**. Under the board (or, on a narrow
window, under the list on the **All** tab) the group **Filed in BIR eBIRForms**
lists the returns imported for the taxpayer you have open, across its branches,
newest period first: the form, the period, "Amended (v2)" for an amendment,
"E-mailed copy" for the copy that was e-mailed to BIR, and "Imported <date>".
They are records only: nothing in the group opens, edits, queues or sends a
return. The group is not drawn when the taxpayer has none, and it steps aside
while a search or a filter is on, since those speak of this app's own returns.
The host function is `list_legacy_filed_returns(tin)` (one branch's TIN; the
answer holds no file bytes and no field values).

The page never gives the host a path. The host functions are
`legacy_standard_folders`, `legacy_scan_folder` (the host's folder dialog),
`legacy_scan_standard(index)`, `legacy_import(scan)` and
`legacy_scan_cancel` in `crates/bir-app-api/src/legacy_import_module.rs`.

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
