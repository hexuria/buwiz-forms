# 🛠️ Verification Scripts

This folder contains utilities to verify the eBIRForms reverse-engineering logic (Parsing, Encryption, and Transport).

## 📄 verify_submission.sh

This is the **Gold Standard** verification script. If this script works and you receive an email, it proves that the following are correctly implemented:
1.  **Parser:** Reading the non-standard XML format.
2.  **Encryption:** The replicated AES-128 / DCPcrypt2 logic.
3.  **Transport:** The FTP connection and directory structure requirements.

### Usage
```bash
chmod +x verify_submission.sh
./verify_submission.sh
```

### How it works
1.  Reads a raw `savefile/` XML.
2.  Converts it to structured JSON.
3.  Updates the `txtDateIssue` field to the current second (to avoid duplicate data rejection).
4.  Encrypts and Compresses the modified data into a new `.xml` payload.
5.  Uploads it to the BIR FTP using a unique filename containing a `+timestamp` suffix.
