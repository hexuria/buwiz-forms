# BIR Bruteforce Test Harness

Automated submission → email confirmation testing system.

## Setup

Add these to your `.env` file (at `bir/.env`):

```
IMAP_EMAIL=codeitlikemiley@gmail.com
IMAP_APP_PASSWORD=your_gmail_app_password_here
```

## Usage

```bash
# Run ALL experiments sequentially (fully autonomous)
python3 bruteforce/bruteforce.py

# Run a specific experiment
python3 bruteforce/bruteforce.py run run01_baseline_q1_resubmit

# View the dashboard (results of all experiments)
python3 bruteforce/bruteforce.py dashboard

# Reset a specific experiment (to re-run it)
python3 bruteforce/bruteforce.py reset run01_baseline_q1_resubmit

# Reset ALL experiments
python3 bruteforce/bruteforce.py reset
```

## Experiments

| Run | Name | Hypothesis |
|-----|------|-----------|
| 1 | `run01_baseline_q1_resubmit` | Control: resubmit Q1 original with only updated timestamp |
| 2 | `run02_amended_return_q1` | BIR may reject duplicate originals but accept amendments |
| 3 | `run03_fresh_q2` | New period (Q2) should trigger email since it's new |
| 4 | `run04_fresh_q3` | Confirms Q2 result with another new period |
| 5 | `run05_q1_with_legacy_fields` | Maybe txtFinalFlag/txtEnroll are required |
| 6 | `run06_amended_q1_with_legacy` | Combined: amended + legacy fields |

## How It Works

1. Loads the proven-working base payload from `bir-analyze/modified.json`
2. Applies experiment-specific overrides
3. Encrypts via the Rust `bir-analyze` tool
4. Uploads via FTP with retry logic (5 attempts, exponential backoff)
5. Polls IMAP for BIR confirmation email (5 min timeout, 30s intervals)
6. Records everything to `bruteforce/bruteforce.db` (SQLite)
7. Logs all payloads + curl outputs to `bruteforce/logs/`

## Database

All results are tracked in `bruteforce/bruteforce.db`. You can query it directly:

```bash
sqlite3 bruteforce/bruteforce.db "SELECT run_name, status, email_received, total_duration_secs FROM experiments"
```

## How This Was Made (Reverse-Engineering Context)

This testing harness was built to reverse-engineer the BIR's email confirmation systems. 
The official eBIRForms desktop app submits tax returns as encrypted XML payloads via an archaic FTP server, and the BIR servers send a confirmation email once they process it.

We created this tool by:
1. **Intercepting Traffic**: Capturing the exact FTP payload and XML schema sent by the official app.
2. **Payload Modification**: Stripping down the intercepted XML to the absolute bare minimum required fields.
3. **Automated Polling**: Writing a script to dynamically mutate the XML (changing tax periods, flags, and timestamps), re-encrypt it using the custom `bir-analyze` rust tool, upload it to the FTP server, and then automatically connect to a test Gmail account via IMAP to wait for the official confirmation email.

This allows us to systematically deduce exactly which XML fields the BIR server actually validates before sending a confirmation receipt.
