#!/usr/bin/env bash
set -u
repo=/c/Users/Administrator/repos/buwiz-forms
logdir=/c/Users/Administrator/build-logs/buwiz-forms-setup
cd "$repo" || exit 1

export BUWIZ_NO_KEYCHAIN=1
export EBIR_TEST_ENV=1
export BIR_AUTH_BYPASS=1
export BIR_SFTP_DRY_RUN=1
unset BIR_SFTP_LIVE
unset BIR_DATABASE_PATH

export RUSTUP_TOOLCHAIN=1.99.0
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=0
export RUST_MIN_STACK=268435456
export GIT_CONFIG_GLOBAL=/dev/null
export GIT_TERMINAL_PROMPT=0
export CARGO_NET_GIT_FETCH_WITH_CLI=true
export CARGO_TARGET_DIR='C:\Users\Administrator\BuwizNsh\target-nsh'
export OPENSSL_DIR='C:\Program Files\OpenSSL-Win64'
export OPENSSL_LIB_DIR='C:\Program Files\OpenSSL-Win64\lib\VC\x64\MD'
export LIBCLANG_PATH='C:\Program Files\LLVM\bin'
export PATH="/c/Strawberry/perl/bin:/c/Users/Administrator/.cargo/bin:/c/Program Files/PowerShell/7:/c/ProgramData/chocolatey/bin:$PATH"

run_step() {
    name=$1
    shift
    rm -f "$logdir/$name.exit"
    "$@" >"$logdir/$name.log" 2>&1
    echo $? >"$logdir/$name.exit"
}

run_step nsh-check just nsh-check
run_step nsh-build just nsh-build
run_step cargo-test-bir-core cargo test --locked -p bir-core
