#!/usr/bin/env python3
import json
import subprocess
import argparse
import os
from datetime import datetime

def main():
    parser = argparse.ArgumentParser(description="BIR Resubmission Experiment Harness")
    parser.add_argument("--run-name", required=True, help="Name of the run for logging")
    parser.add_argument("--amended", action="store_true", help="Set to True for amended return")
    parser.add_argument("--period-qtr", type=int, default=1, help="Quarter (1-4)")
    parser.add_argument("--filename-suffix", default="", help="Suffix for filename to test collision")
    parser.add_argument("--legacy-fields", action="store_true", help="Include legacy fields")

    args = parser.parse_args()

    analyze_dir = "/Volumes/goldcoders/reverse-engineer-ebir-forms/bir-analyze"
    base_json_path = os.path.join(analyze_dir, "modified.json")
    
    with open(base_json_path, "r") as f:
        payload = json.load(f)

    # 1. Update temporal heartbeat
    now_str = datetime.now().strftime("%m/%d/%Y %H:%M:%S")
    payload["txtDateIssue"] = now_str
    
    # 2. Update Amended Flag
    if args.amended:
        payload["frm2551Qv2018:amendedRtn_1"] = "true"
        payload["frm2551Qv2018:amendedRtn_2"] = "false"
    else:
        payload["frm2551Qv2018:amendedRtn_1"] = "false"
        payload["frm2551Qv2018:amendedRtn_2"] = "true"

    # 3. Update Quarter selection
    for i in range(1, 5):
        payload[f"frm2551Qv2018:qtr_{i}"] = "true" if i == args.period_qtr else "false"

    # 4. Handle Legacy Fields
    if args.legacy_fields:
        payload["txtFinalFlag"] = "0"
        payload["txtEnroll"] = "Y"
        payload["txtDateExpiry"] = ""
    else:
        payload.pop("txtFinalFlag", None)
        payload.pop("txtEnroll", None)
        payload.pop("txtDateExpiry", None)

    # Resolve filename
    tin = "".join([
        payload.get("frm2551Qv2018:txtTIN1", "000"),
        payload.get("frm2551Qv2018:txtTIN2", "000"),
        payload.get("frm2551Qv2018:txtTIN3", "000"),
        payload.get("frm2551Qv2018:txtBranchCode", "000")
    ])
    form_type = "2551Qv2018"
    year = payload.get("frm2551Qv2018:txtYear", "2026")
    period_code = f"12{year}Q{args.period_qtr}"
    email = payload.get("txtEmail", "test@example.com")
    
    output_filename = f"{tin}-{form_type}-{period_code}{args.filename_suffix}#{email}#.xml"
    
    logs_dir = "/Volumes/goldcoders/reverse-engineer-ebir-forms/bir/scripts/logs"
    os.makedirs(logs_dir, exist_ok=True)
    
    # Log payload
    log_json_path = os.path.join(logs_dir, f"{args.run_name}_payload.json")
    with open(log_json_path, "w") as f:
        json.dump(payload, f, indent=2)

    # Prep temp json for rust encryptor
    temp_json = os.path.join(analyze_dir, "temp_experiment.json")
    with open(temp_json, "w") as f:
        json.dump(payload, f)

    xml_output_path = os.path.join(logs_dir, f"{args.run_name}_encrypted.xml")

    print(f"[{args.run_name}] Generating encrypted payload for {output_filename}...")
    subprocess.run(["cargo", "run", "-q", "--", "generate-encrypted", temp_json, xml_output_path], cwd=analyze_dir, check=True)

    ftp_url = f"ftp://103.56.5.254/{form_type}/{output_filename.replace('#', '%23')}"
    print(f"[{args.run_name}] Uploading to {ftp_url}...")
    
    curl_cmd = [
        "curl", "-sS", "-v", "--connect-timeout", "30", "--max-time", "60", "--no-epsv", "--ftp-pasv", "--ftp-skip-pasv-ip",
        "-u", "uploadOnly:12birBIR",
        "-T", xml_output_path,
        ftp_url
    ]
    
    result = subprocess.run(curl_cmd, capture_output=True, text=True)
    
    # Log curl output
    log_curl_path = os.path.join(logs_dir, f"{args.run_name}_curl.log")
    with open(log_curl_path, "w") as f:
        f.write(result.stderr)
        f.write("\n")
        f.write(result.stdout)
        
    if result.returncode == 0:
        print(f"✅ Success! Uploaded {output_filename}. Waiting for email...")
    else:
        print(f"❌ Failed to upload. Return code {result.returncode}")
        print("Check logs for details.")

if __name__ == "__main__":
    main()
