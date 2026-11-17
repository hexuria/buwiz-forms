# /// script
# requires-python = ">=3.11"
# dependencies = [
#     "pyjwt[crypto]",
#     "requests",
# ]
# ///

import argparse
import os
import re
import sys
import time
from pathlib import Path

import jwt
import requests


def get_latest_build():
    issuer_id = os.environ.get("APP_STORE_ISSUER_ID")
    key_id = os.environ.get("APP_STORE_KEY_ID")
    p8_path = os.environ.get("APP_STORE_P8_PATH")
    bundle_id = "dev.goldcoders.bir"

    if not all([issuer_id, key_id, p8_path]):
        print(
            "Error: Missing App Store Connect API credentials in environment variables.",
            file=sys.stderr,
        )
        print(
            "Please set APP_STORE_ISSUER_ID, APP_STORE_KEY_ID, and APP_STORE_P8_PATH.",
            file=sys.stderr,
        )
        sys.exit(1)

    try:
        with open(p8_path, "r") as f:
            private_key = f.read()
    except Exception as e:
        print(f"Error reading private key at {p8_path}: {e}", file=sys.stderr)
        sys.exit(1)

    # 1. Generate JWT Token
    headers = {
        "alg": "ES256",
        "kid": key_id,
        "typ": "JWT"
    }

    payload = {
        "iss": issuer_id,
        "iat": int(time.time()),
        "exp": int(time.time()) + 1200, # 20 minutes max
        "aud": "appstoreconnect-v1"
    }

    token = jwt.encode(payload, private_key, algorithm="ES256", headers=headers)

    # 2. Get App ID from Bundle ID
    auth_header = {"Authorization": f"Bearer {token}"}
    
    apps_res = requests.get(
        f"https://api.appstoreconnect.apple.com/v1/apps?filter[bundleId]={bundle_id}", 
        headers=auth_header
    )
    apps_res.raise_for_status()
    apps_data = apps_res.json()
    
    if not apps_data["data"]:
        print(
            f"Error: App with bundle ID {bundle_id} not found in App Store Connect.",
            file=sys.stderr,
        )
        sys.exit(1)
        
    app_id = apps_data["data"][0]["id"]
    
    # 3. Get latest build
    builds_res = requests.get(
        f"https://api.appstoreconnect.apple.com/v1/builds?filter[app]={app_id}&sort=-version&limit=1",
        headers=auth_header
    )
    builds_res.raise_for_status()
    builds_data = builds_res.json()
    
    if not builds_data["data"]:
        print("No previous builds found. Starting from build 1.", file=sys.stderr)
        return 0
        
    latest_build = builds_data["data"][0]["attributes"]["version"]
    return int(latest_build)


def bump_justfile_build(new_build: int):
    justfile_path = Path("justfile")
    if not justfile_path.exists():
        print("Error: justfile not found in current directory.")
        sys.exit(1)
        
    content = justfile_path.read_text()
    
    # Regex to find BUILD_NUMBER := env_var_or_default("BUILD_NUMBER", "X")
    pattern = re.compile(r'(BUILD_NUMBER\s*:=\s*env_var_or_default\("BUILD_NUMBER",\s*")(\d+)("\))')
    
    if not pattern.search(content):
        # Fallback to simple assignment
        pattern = re.compile(r'(BUILD_NUMBER\s*:=\s*")(\d+)(")')
        if not pattern.search(content):
            print("Error: Could not find BUILD_NUMBER assignment in justfile.")
            sys.exit(1)
        
    new_content = pattern.sub(rf'\g<1>{new_build}\g<3>', content)
    justfile_path.write_text(new_content)
    print(f"Successfully updated justfile BUILD_NUMBER to {new_build}")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--print-next",
        action="store_true",
        help=(
            "print only the next App Store build number without modifying "
            "tracked source"
        ),
    )
    args = parser.parse_args(argv)

    print("Fetching latest build number from App Store Connect...", file=sys.stderr)
    latest = get_latest_build()
    next_build = latest + 1
    if args.print_next:
        print(next_build)
        return 0

    print(f"Latest build is {latest}. Bumping to {next_build}...", file=sys.stderr)
    bump_justfile_build(next_build)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
