#!/usr/bin/env python3
"""
BIR Bruteforce Test Harness
============================
Automated submission → email confirmation loop.

Uses a local SQLite DB to track every experiment run:
  - What payload was sent (JSON snapshot)
  - What changed vs the baseline
  - FTP result
  - Whether a BIR confirmation email was received (polled via IMAP)
  - Timestamps for everything

All data lives in bruteforce/bruteforce.db
All encrypted payloads + curl logs go to bruteforce/logs/
"""

import sqlite3
import json
import subprocess
import os
import sys
import time
import imaplib
import email as email_lib
import re
from datetime import datetime
from pathlib import Path

# ── Configuration ─────────────────────────────────────────────────────────────
ROOT = Path("/Volumes/goldcoders/reverse-engineer-ebir-forms")
ANALYZE_DIR = ROOT / "bir-analyze"
BRUTEFORCE_DIR = ROOT / "bir" / "bruteforce"
LOGS_DIR = BRUTEFORCE_DIR / "logs"
DB_PATH = BRUTEFORCE_DIR / "bruteforce.db"
BASE_JSON = ANALYZE_DIR / "modified.json"

FTP_HOST = "103.56.5.254"
FTP_USER = "uploadOnly"
FTP_PASS = "12birBIR"
FORM_TYPE = "2551Qv2018"

# IMAP settings for checking email
IMAP_HOST = "imap.gmail.com"
IMAP_PORT = 993
# We use App Password auth for the bruteforce checker
# These are loaded from environment or .env
IMAP_EMAIL = os.environ.get("IMAP_EMAIL", "codeitlikemiley@gmail.com")
IMAP_PASSWORD = os.environ.get("IMAP_APP_PASSWORD", "")

# How long to poll for email confirmation (seconds)
# BIR has ~20 minute processing delay, so we need a generous timeout
EMAIL_POLL_TIMEOUT = 1500  # 25 minutes
EMAIL_POLL_INTERVAL = 60   # Check every 60 seconds

MAX_FTP_RETRIES = 5

# ── Database Setup ────────────────────────────────────────────────────────────

def init_db():
    """Create the bruteforce tracking database."""
    conn = sqlite3.connect(str(DB_PATH))
    conn.execute("""
        CREATE TABLE IF NOT EXISTS experiments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_name TEXT NOT NULL UNIQUE,
            hypothesis TEXT,
            status TEXT NOT NULL DEFAULT 'pending',
            -- Payload details
            payload_json TEXT NOT NULL,
            changes_made TEXT NOT NULL,
            filename TEXT NOT NULL,
            -- FTP results
            ftp_attempts INTEGER DEFAULT 0,
            ftp_success INTEGER DEFAULT 0,
            ftp_error TEXT,
            -- Email confirmation
            email_received INTEGER DEFAULT 0,
            email_subject TEXT,
            email_body_snippet TEXT,
            email_received_at TEXT,
            -- Timestamps
            created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
            submitted_at TEXT,
            poll_started_at TEXT,
            poll_ended_at TEXT,
            -- Duration tracking
            total_duration_secs REAL
        )
    """)
    conn.execute("""
        CREATE TABLE IF NOT EXISTS seen_email_uids (
            uid TEXT PRIMARY KEY,
            run_name TEXT,
            seen_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
        )
    """)
    conn.commit()
    return conn


# ── Payload Builder ───────────────────────────────────────────────────────────

def load_base_payload():
    """Load the proven-working base payload from bir-analyze."""
    with open(str(BASE_JSON), "r") as f:
        return json.load(f)


def build_payload(overrides: dict, remove_keys: list = None) -> dict:
    """Build a payload from the base, applying overrides and removing keys."""
    payload = load_base_payload()
    
    # Always update the heartbeat
    now_str = datetime.now().strftime("%m/%d/%Y %H:%M:%S")
    payload["txtDateIssue"] = now_str
    
    # Apply overrides
    for k, v in overrides.items():
        payload[k] = v
    
    # Remove keys
    if remove_keys:
        for k in remove_keys:
            payload.pop(k, None)
    
    return payload


def build_filename(payload: dict, suffix: str = "") -> str:
    """Build the BIR-compliant filename from a payload."""
    tin = "".join([
        payload.get("frm2551Qv2018:txtTIN1", "000"),
        payload.get("frm2551Qv2018:txtTIN2", "000"),
        payload.get("frm2551Qv2018:txtTIN3", "000"),
        payload.get("frm2551Qv2018:txtBranchCode", "000")
    ])
    year = payload.get("frm2551Qv2018:txtYear", "2026")
    
    # Determine quarter from flags
    qtr = 1
    for i in range(1, 5):
        if payload.get(f"frm2551Qv2018:qtr_{i}") == "true":
            qtr = i
            break
    
    period_code = f"12{year}Q{qtr}"
    eml = payload.get("txtEmail", "test@example.com")
    
    return f"{tin}-{FORM_TYPE}-{period_code}{suffix}#{eml}#.xml"


# ── Encryption ────────────────────────────────────────────────────────────────

def encrypt_payload(payload: dict, output_path: str) -> bool:
    """Use the Rust bir-analyze tool to encrypt a payload."""
    temp_json = str(ANALYZE_DIR / "temp_bruteforce.json")
    with open(temp_json, "w") as f:
        json.dump(payload, f)
    
    result = subprocess.run(
        ["cargo", "run", "-q", "--", "generate-encrypted", temp_json, output_path],
        cwd=str(ANALYZE_DIR),
        capture_output=True,
        text=True
    )
    
    if result.returncode != 0:
        print(f"  ❌ Encryption failed: {result.stderr}")
        return False
    return True


# ── FTP Upload ────────────────────────────────────────────────────────────────

def upload_to_bir(encrypted_path: str, filename: str) -> (bool, str, int):
    """Upload encrypted payload to BIR FTP. Returns (success, error_msg, attempts)."""
    ftp_url = f"ftp://{FTP_HOST}/{FORM_TYPE}/{filename.replace('#', '%23')}"
    
    for attempt in range(1, MAX_FTP_RETRIES + 1):
        print(f"  📡 FTP attempt {attempt}/{MAX_FTP_RETRIES}...")
        result = subprocess.run(
            [
                "curl", "-sS", "-v",
                "--connect-timeout", "30",
                "--max-time", "60",
                "--no-epsv", "--ftp-pasv", "--ftp-skip-pasv-ip",
                "-u", f"{FTP_USER}:{FTP_PASS}",
                "-T", encrypted_path,
                ftp_url
            ],
            capture_output=True, text=True
        )
        
        # Log curl output
        log_path = str(LOGS_DIR / f"{Path(encrypted_path).stem}_curl_attempt{attempt}.log")
        with open(log_path, "w") as f:
            f.write(f"=== Attempt {attempt} at {datetime.now().isoformat()} ===\n")
            f.write(result.stderr)
            f.write("\n")
            f.write(result.stdout)
        
        if result.returncode == 0:
            # Verify 226 in the verbose output
            if "226" in result.stderr:
                return True, "", attempt
            else:
                return True, "Upload returned 0 but no 226 confirmation", attempt
        
        error_msg = result.stderr.strip().split("\n")[-1] if result.stderr else f"exit code {result.returncode}"
        print(f"    ⚠️  Attempt {attempt} failed: {error_msg}")
        
        if attempt < MAX_FTP_RETRIES:
            wait = 2 ** (attempt - 1)
            print(f"    ⏳ Waiting {wait}s before retry...")
            time.sleep(wait)
    
    return False, f"All {MAX_FTP_RETRIES} attempts failed", MAX_FTP_RETRIES


# ── Email Polling ─────────────────────────────────────────────────────────────

def check_for_bir_email(filename: str, since_time: datetime, db_conn=None) -> dict | None:
    """
    Connect to IMAP and search for a BIR confirmation email
    that references the given filename.
    
    Uses ALL (not UNSEEN) because the user may read emails in Gmail,
    marking them as Seen before we can poll. We track processed UIDs
    in the local DB to avoid double-counting.
    
    Returns dict with email details or None.
    """
    if not IMAP_PASSWORD:
        print("  ⚠️  No IMAP_APP_PASSWORD set — skipping email check")
        return None
    
    # Load already-seen UIDs from DB
    seen_uids = set()
    if db_conn:
        rows = db_conn.execute("SELECT uid FROM seen_email_uids").fetchall()
        seen_uids = {r[0] for r in rows}
    
    try:
        mail = imaplib.IMAP4_SSL(IMAP_HOST, IMAP_PORT)
        mail.login(IMAP_EMAIL, IMAP_PASSWORD)
        mail.select("INBOX")
        
        # Search ALL BIR emails since today (not UNSEEN — user may have read them)
        search_date = since_time.strftime("%d-%b-%Y")
        status, data = mail.search(None, f'(FROM "ebirforms-noreply@bir.gov.ph" SINCE {search_date})')
        
        if status != "OK" or not data[0]:
            mail.logout()
            return None
        
        msg_ids = data[0].split()
        
        for msg_id in msg_ids:
            uid_str = msg_id.decode() if isinstance(msg_id, bytes) else str(msg_id)
            
            # Skip already-processed emails
            if uid_str in seen_uids:
                continue
            
            status, msg_data = mail.fetch(msg_id, "(RFC822)")
            if status != "OK":
                continue
            
            raw_email = msg_data[0][1]
            msg = email_lib.message_from_bytes(raw_email)
            
            # Extract body
            body = ""
            if msg.is_multipart():
                for part in msg.walk():
                    if part.get_content_type() == "text/plain":
                        payload_bytes = part.get_payload(decode=True)
                        if payload_bytes:
                            body = payload_bytes.decode("utf-8", errors="replace")
                        break
            else:
                payload_bytes = msg.get_payload(decode=True)
                if payload_bytes:
                    body = payload_bytes.decode("utf-8", errors="replace")
            
            # Check if this email references our filename
            # BIR strips the #email# part, so the email shows:
            #   "File name: 010558054000-2551Qv2018-122026Q1.xml"
            # We match on the TIN-FORM-PERIOD stem
            base_stem = filename.split("#")[0]  # e.g., "010558054000-2551Qv2018-122026Q1"
            
            # Also extract the internal date from the email to verify timing
            email_date_str = msg.get("Date", "")
            
            if base_stem in body:
                subject = msg.get("Subject", "")
                
                # Record this UID as processed
                if db_conn:
                    db_conn.execute(
                        "INSERT OR IGNORE INTO seen_email_uids (uid, seen_at) VALUES (?, ?)",
                        (uid_str, datetime.now().isoformat())
                    )
                    db_conn.commit()
                
                mail.logout()
                return {
                    "subject": subject,
                    "body_snippet": body[:500],
                    "received_at": datetime.now().isoformat(),
                    "email_internal_date": email_date_str,
                }
        
        mail.logout()
        return None
        
    except Exception as e:
        print(f"  ⚠️  IMAP error: {e}")
        return None


def poll_for_confirmation(filename: str, since_time: datetime, db_conn=None, timeout: int = EMAIL_POLL_TIMEOUT) -> dict | None:
    """Poll IMAP every EMAIL_POLL_INTERVAL seconds until timeout or email found."""
    start = time.time()
    elapsed = 0
    check_num = 0
    
    while elapsed < timeout:
        check_num += 1
        remaining = int(timeout - elapsed)
        mins = remaining // 60
        secs = remaining % 60
        print(f"  📬 Email check #{check_num} (⏱ {mins}m{secs}s remaining)...")
        
        result = check_for_bir_email(filename, since_time, db_conn=db_conn)
        if result:
            return result
        
        time.sleep(EMAIL_POLL_INTERVAL)
        elapsed = time.time() - start
    
    return None


# ── Experiment Runner ─────────────────────────────────────────────────────────

def run_experiment(conn, run_name: str, hypothesis: str, overrides: dict, 
                   remove_keys: list = None, filename_suffix: str = ""):
    """Run a single experiment end-to-end."""
    
    print(f"\n{'='*70}")
    print(f"🧪 EXPERIMENT: {run_name}")
    print(f"   Hypothesis: {hypothesis}")
    print(f"{'='*70}")
    
    # Check if already run
    existing = conn.execute("SELECT status FROM experiments WHERE run_name = ?", (run_name,)).fetchone()
    if existing and existing[0] in ("confirmed", "no_email"):
        print(f"  ⏭  Already completed with status: {existing[0]}. Skipping.")
        return existing[0]
    
    # 1. Build payload
    payload = build_payload(overrides, remove_keys)
    filename = build_filename(payload, suffix=filename_suffix)
    
    # Compute changes description
    changes = {}
    base = load_base_payload()
    for k, v in overrides.items():
        old_val = base.get(k, "<missing>")
        if old_val != v:
            changes[k] = {"from": old_val, "to": v}
    if remove_keys:
        for k in remove_keys:
            if k in base:
                changes[k] = {"from": base[k], "to": "<removed>"}
    changes["txtDateIssue"] = {"from": base.get("txtDateIssue", ""), "to": payload["txtDateIssue"]}
    
    changes_json = json.dumps(changes, indent=2)
    payload_json = json.dumps(payload, indent=2)
    
    print(f"  📄 Filename: {filename}")
    print(f"  🔧 Changes from baseline:")
    for k, v in changes.items():
        print(f"     {k}: {v['from']} → {v['to']}")
    
    # Save payload to logs
    payload_log = str(LOGS_DIR / f"{run_name}_payload.json")
    with open(payload_log, "w") as f:
        f.write(payload_json)
    
    # 2. Encrypt
    encrypted_path = str(LOGS_DIR / f"{run_name}_encrypted.xml")
    print(f"  🔐 Encrypting payload...")
    if not encrypt_payload(payload, encrypted_path):
        conn.execute("""
            INSERT OR REPLACE INTO experiments (run_name, hypothesis, status, payload_json, changes_made, filename, ftp_error)
            VALUES (?, ?, 'failed', ?, ?, ?, 'Encryption failed')
        """, (run_name, hypothesis, payload_json, changes_json, filename))
        conn.commit()
        return "failed"
    
    # 3. Upload via FTP
    submit_time = datetime.now()
    print(f"  📡 Uploading to BIR FTP...")
    ftp_success, ftp_error, ftp_attempts = upload_to_bir(encrypted_path, filename)
    
    if not ftp_success:
        print(f"  ❌ FTP FAILED after {ftp_attempts} attempts: {ftp_error}")
        conn.execute("""
            INSERT OR REPLACE INTO experiments (run_name, hypothesis, status, payload_json, changes_made, filename,
                ftp_attempts, ftp_success, ftp_error, submitted_at)
            VALUES (?, ?, 'ftp_failed', ?, ?, ?, ?, 0, ?, ?)
        """, (run_name, hypothesis, payload_json, changes_json, filename,
              ftp_attempts, ftp_error, submit_time.isoformat()))
        conn.commit()
        return "ftp_failed"
    
    print(f"  ✅ FTP upload successful on attempt {ftp_attempts}")
    
    # 4. Poll for email confirmation
    poll_start = datetime.now()
    print(f"  📬 Starting email poll (timeout: {EMAIL_POLL_TIMEOUT}s)...")
    
    # Upsert the experiment as "polling"
    conn.execute("""
        INSERT OR REPLACE INTO experiments (run_name, hypothesis, status, payload_json, changes_made, filename,
            ftp_attempts, ftp_success, submitted_at, poll_started_at)
        VALUES (?, ?, 'polling', ?, ?, ?, ?, 1, ?, ?)
    """, (run_name, hypothesis, payload_json, changes_json, filename,
          ftp_attempts, submit_time.isoformat(), poll_start.isoformat()))
    conn.commit()
    
    email_result = poll_for_confirmation(filename, submit_time, db_conn=conn)
    poll_end = datetime.now()
    total_duration = (poll_end - submit_time).total_seconds()
    
    if email_result:
        print(f"  🎉 EMAIL RECEIVED!")
        print(f"     Subject: {email_result['subject']}")
        print(f"     Snippet: {email_result['body_snippet'][:200]}")
        
        conn.execute("""
            UPDATE experiments SET
                status = 'confirmed',
                email_received = 1,
                email_subject = ?,
                email_body_snippet = ?,
                email_received_at = ?,
                poll_ended_at = ?,
                total_duration_secs = ?
            WHERE run_name = ?
        """, (email_result['subject'], email_result['body_snippet'],
              email_result['received_at'], poll_end.isoformat(), total_duration, run_name))
        conn.commit()
        return "confirmed"
    else:
        print(f"  ⏰ No email received after {int(total_duration)}s")
        
        conn.execute("""
            UPDATE experiments SET
                status = 'no_email',
                email_received = 0,
                poll_ended_at = ?,
                total_duration_secs = ?
            WHERE run_name = ?
        """, (poll_end.isoformat(), total_duration, run_name))
        conn.commit()
        return "no_email"


# ── Status Dashboard ──────────────────────────────────────────────────────────

def print_dashboard(conn):
    """Print a summary of all experiments."""
    print(f"\n{'='*80}")
    print(f"📊 BRUTEFORCE EXPERIMENT DASHBOARD")
    print(f"{'='*80}")
    
    rows = conn.execute("""
        SELECT run_name, hypothesis, status, ftp_attempts, email_received,
               total_duration_secs, created_at, filename
        FROM experiments ORDER BY id
    """).fetchall()
    
    if not rows:
        print("  No experiments run yet.")
        return
    
    for r in rows:
        name, hyp, status, attempts, email_ok, duration, created, fname = r
        icon = {
            "confirmed": "✅",
            "no_email": "❌",
            "ftp_failed": "💀",
            "failed": "🔴",
            "polling": "⏳",
            "pending": "⬜",
        }.get(status, "❓")
        
        dur_str = f"{int(duration)}s" if duration else "—"
        print(f"  {icon} {name:<30} | {status:<12} | FTP:{attempts or 0} | Email:{email_ok or 0} | {dur_str} | {hyp or ''}")
    
    # Summary
    total = len(rows)
    confirmed = sum(1 for r in rows if r[2] == "confirmed")
    no_email = sum(1 for r in rows if r[2] == "no_email")
    failed = sum(1 for r in rows if r[2] in ("ftp_failed", "failed"))
    
    print(f"\n  Total: {total} | ✅ Confirmed: {confirmed} | ❌ No Email: {no_email} | 💀 Failed: {failed}")
    print(f"{'='*80}\n")


# ── Experiment Definitions ────────────────────────────────────────────────────

EXPERIMENTS = [
    {
        "run_name": "run01_baseline_q1_resubmit",
        "hypothesis": "Control: Resubmit Q1 original with only updated timestamp",
        "overrides": {},
        "remove_keys": None,
    },
    {
        "run_name": "run02_amended_return_q1",
        "hypothesis": "Amended flag: BIR may reject duplicate originals but accept amendments",
        "overrides": {
            "frm2551Qv2018:amendedRtn_1": "true",
            "frm2551Qv2018:amendedRtn_2": "false",
        },
    },
    {
        "run_name": "run03_fresh_q2",
        "hypothesis": "New period: Q2 should trigger email since it's a new filing",
        "overrides": {
            "frm2551Qv2018:qtr_1": "false",
            "frm2551Qv2018:qtr_2": "true",
            "frm2551Qv2018:qtr_3": "false",
            "frm2551Qv2018:qtr_4": "false",
        },
    },
    {
        "run_name": "run04_fresh_q3",
        "hypothesis": "New period: Q3 should also trigger email (confirms Q2 result)",
        "overrides": {
            "frm2551Qv2018:qtr_1": "false",
            "frm2551Qv2018:qtr_2": "false",
            "frm2551Qv2018:qtr_3": "true",
            "frm2551Qv2018:qtr_4": "false",
        },
    },
    {
        "run_name": "run05_q1_with_legacy_fields",
        "hypothesis": "Legacy fields: Maybe txtFinalFlag/txtEnroll are required for reprocessing",
        "overrides": {
            "txtFinalFlag": "0",
            "txtEnroll": "Y",
            "txtDateExpiry": "",
            "txtTaxAgentNo": "",
        },
    },
    {
        "run_name": "run06_amended_q1_with_legacy",
        "hypothesis": "Combined: Amended flag + legacy fields for maximum compatibility",
        "overrides": {
            "frm2551Qv2018:amendedRtn_1": "true",
            "frm2551Qv2018:amendedRtn_2": "false",
            "txtFinalFlag": "0",
            "txtEnroll": "Y",
            "txtDateExpiry": "",
            "txtTaxAgentNo": "",
        },
    },
]


# ── Main ──────────────────────────────────────────────────────────────────────

def main():
    os.makedirs(str(LOGS_DIR), exist_ok=True)
    
    # Load .env if present
    env_path = ROOT / "bir" / ".env"
    if env_path.exists():
        with open(str(env_path)) as f:
            for line in f:
                line = line.strip()
                if line and not line.startswith("#") and "=" in line:
                    k, v = line.split("=", 1)
                    os.environ.setdefault(k.strip(), v.strip())
        # Re-read after loading .env
        global IMAP_EMAIL, IMAP_PASSWORD
        IMAP_EMAIL = os.environ.get("IMAP_EMAIL", IMAP_EMAIL)
        IMAP_PASSWORD = os.environ.get("IMAP_APP_PASSWORD", IMAP_PASSWORD)
    
    conn = init_db()
    
    if len(sys.argv) > 1:
        cmd = sys.argv[1]
        if cmd == "dashboard":
            print_dashboard(conn)
            conn.close()
            return
        elif cmd == "run":
            # Run specific experiment by name
            target = sys.argv[2] if len(sys.argv) > 2 else None
            if target:
                exp = next((e for e in EXPERIMENTS if e["run_name"] == target), None)
                if exp:
                    run_experiment(conn, **exp)
                else:
                    print(f"Unknown experiment: {target}")
                    print(f"Available: {', '.join(e['run_name'] for e in EXPERIMENTS)}")
            else:
                print("Usage: bruteforce.py run <experiment_name>")
            print_dashboard(conn)
            conn.close()
            return
        elif cmd == "reset":
            # Reset a specific or all experiments
            target = sys.argv[2] if len(sys.argv) > 2 else None
            if target:
                conn.execute("DELETE FROM experiments WHERE run_name = ?", (target,))
            else:
                conn.execute("DELETE FROM experiments")
                conn.execute("DELETE FROM seen_email_uids")
            conn.commit()
            print("Reset complete.")
            conn.close()
            return
        elif cmd == "recheck":
            # Re-scan IMAP for delayed emails matching 'no_email' experiments
            print("\n🔄 Re-checking IMAP for delayed BIR confirmation emails...\n")
            rows = conn.execute("""
                SELECT run_name, filename, submitted_at FROM experiments
                WHERE status = 'no_email'
                ORDER BY id
            """).fetchall()
            
            if not rows:
                print("  No experiments with 'no_email' status to recheck.")
            else:
                for run_name, filename, submitted_at in rows:
                    print(f"  Checking {run_name} ({filename})...")
                    since = datetime.fromisoformat(submitted_at)
                    result = check_for_bir_email(filename, since, db_conn=conn)
                    if result:
                        print(f"  🎉 FOUND! Email for {run_name}")
                        print(f"     Snippet: {result['body_snippet'][:150]}")
                        conn.execute("""
                            UPDATE experiments SET
                                status = 'confirmed',
                                email_received = 1,
                                email_subject = ?,
                                email_body_snippet = ?,
                                email_received_at = ?,
                                poll_ended_at = ?
                            WHERE run_name = ?
                        """, (result.get('subject', ''), result['body_snippet'],
                              result['received_at'], datetime.now().isoformat(), run_name))
                        conn.commit()
                    else:
                        print(f"  ❌ Still no email for {run_name}")
            
            print_dashboard(conn)
            conn.close()
            return
    
    # Default: run ALL experiments sequentially
    print(f"""
╔══════════════════════════════════════════════════════════════════════╗
║  BIR BRUTEFORCE TEST HARNESS v1.0                                  ║
║  Running {len(EXPERIMENTS)} experiments sequentially                            ║
║  Email poll timeout: {EMAIL_POLL_TIMEOUT}s per experiment                        ║
║  Max FTP retries: {MAX_FTP_RETRIES}                                                ║
╚══════════════════════════════════════════════════════════════════════╝
""")
    
    for exp in EXPERIMENTS:
        result = run_experiment(conn, **exp)
        print(f"\n  → Result: {result}")
        
        if result == "ftp_failed":
            print("\n  🛑 FTP is down. Pausing remaining experiments.")
            break
    
    print_dashboard(conn)
    conn.close()


if __name__ == "__main__":
    main()
