#!/bin/bash

# ==============================================================================
# eBIRForms Stealth Submission Engine
# ==============================================================================

# --- Logging Setup ---
# Use absolute path so LOG_FILE stays consistent after 'cd'
LOG_FILE="$(pwd)/dispatch.log"

# Function to strip ANSI color codes for the log file
strip_colors() {
    sed -E "s/\x1B\[([0-9]{1,3}(;[0-9]{1,3})*)?[mGK]//g"
}

# Clear old log and start fresh for this session
echo "--- SESSION START: $(date) ---" > "$LOG_FILE"
echo "Working Directory: $(pwd)" >> "$LOG_FILE"

# We will use a wrapper to log and print
log_info() {
    echo -e "$1"
    echo -e "$1" | strip_colors >> "$LOG_FILE"
}

# --- Colors ---
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color
BOLD='\033[1m'

# --- Configuration ---
TIN="010558054000"
FORM_TYPE="2551Qv2018"
EMAIL="codeitlikemiley@gmail.com"
PERIOD="122026Q1"
FTP_HOST="103.56.5.254"
FTP_USER="uploadOnly"
FTP_PASS="12birBIR"

SOURCE_XML="/Volumes/goldcoders/reverse-engineer-ebir-forms/savefile/010558054000-2551Qv2018-122026Q1.xml"
ROOT="/Volumes/goldcoders/reverse-engineer-ebir-forms"

# --- Header ---
log_info "\n${BOLD}${MAGENTA}⚡ BIR PAYLOAD DISPATCH SYSTEM v2.0${NC}"
log_info "${CYAN}────────────────────────────────────────────────────────────────${NC}"
log_info "${BLUE}Log File:${NC} $LOG_FILE"
log_info "${CYAN}────────────────────────────────────────────────────────────────${NC}"

# --- Setup ---
mkdir -p "$ROOT/build"
if [ -d "$ROOT/bir-analyze" ]; then
    cd "$ROOT/bir-analyze" || exit 1
else
    cd "$(dirname "$0")/.." || exit 1
fi

log_info "${BLUE}◈ ${NC}Decrypting and normalizing source IAF..."
# 1. Decrypt the binary source
# 2. Add newlines after every </div> to ensure one-field-per-line for the parser
cargo run -q -- decrypt "$SOURCE_XML" 2>> "$LOG_FILE" | \
    sed 's/<\/div>/<\/div>\n/g' > fixed.xml 2>> "$LOG_FILE"

log_info "${BLUE}◈ ${NC}Injecting temporal heartbeat..."
NOW=$(date +"%m/%d/%Y %H:%M:%S")
# Parse the fixed XML, inject date, and save to JSON
cargo run -q -- parse fixed.xml 2>> "$LOG_FILE" | jq --arg now "$NOW" '.["txtDateIssue"] = $now' > modified.json 2>> "$LOG_FILE"

log_info "${BLUE}◈ ${NC}Generating encrypted IAF envelope..."
OUTPUT_FILENAME="${TIN}-${FORM_TYPE}-${PERIOD}#${EMAIL}#.xml"
ENCODED_FILENAME=$(echo "$OUTPUT_FILENAME" | sed 's/#/%23/g')

# Log the payload structure to verify it is NOT empty anymore
echo "DEBUG: RAW PAYLOAD FIELDS (JSON)" >> "$LOG_FILE"
cat modified.json >> "$LOG_FILE" 2>&1
echo -e "\n----------------------------------------------------------------" >> "$LOG_FILE"

cargo run -q -- generate-encrypted modified.json "$ROOT/build/$OUTPUT_FILENAME" >> "$LOG_FILE" 2>&1

log_info "${BLUE}◈ ${NC}Transmitting to BIR Gateway... (Secure Tunnel)"
# We use the ENCODED_FILENAME in the URL so curl sends the full name including the email
curl -v --connect-timeout 30 --max-time 60 --no-epsv --ftp-pasv --ftp-skip-pasv-ip \
     -u "$FTP_USER:$FTP_PASS" \
     -T "$ROOT/build/$OUTPUT_FILENAME" \
     "ftp://$FTP_HOST/$FORM_TYPE/$ENCODED_FILENAME" >> "$LOG_FILE" 2>&1

RESULT=$?

log_info "${CYAN}────────────────────────────────────────────────────────────────${NC}"

if [ $RESULT -eq 0 ]; then
    log_info "${GREEN}${BOLD}✅ SUCCESS: Payload accepted by Remote Gateway.${NC}"
    log_info "${YELLOW}📬 Monitoring BIR server for confirmation email...${NC}"
    log_info ""
    log_info "${MAGENTA}${BOLD}SURPRISE, MOTHERFUCKER! THE TAXES ARE FILED. 😎${NC}"
    log_info "${CYAN}Target: $EMAIL${NC}"
else
    log_info "${RED}${BOLD}❌ DISPATCH FAILED (Error Code: $RESULT)${NC}"
    log_info "${YELLOW}⚠️  BIR Server at $FTP_HOST is not responding to handshakes.${NC}"
fi
log_info ""
