set -x
source /tmp/claude-1000/-home-agent-workspace/75a3a4d5-f426-4ca2-af38-3fdce8d073d7/scratchpad/env.sh; export PATH=$HOME/.cargo/bin:$PATH  # bypass the mbx shim while the disk is nearly full
cd /tmp/claude-1000/-home-agent-workspace/75a3a4d5-f426-4ca2-af38-3fdce8d073d7/scratchpad/forms
cargo fmt --all --check >/dev/null 2>&1 && echo FMT_OK || { cargo fmt --all; git diff --stat; echo FMT_CHANGED; }
cargo test -p bir-core --lib 2>&1 | grep -E "test result|FAILED|panicked"
cargo test -p bir-core --test sample_payloads_test 2>&1 | grep -E "test result|FAILED|panicked"
cargo test -p bir-core --tests 2>&1 | grep -E "test result|FAILED|panicked|^error"
touch crates/bir-print/build.rs
cargo test -p bir-print 2>&1 | grep -E "test result|FAILED|panicked|^error"
LD_LIBRARY_PATH=/home/agent/.cache/rtroot/usr/lib/aarch64-linux-gnu:/tmp/claude-1000/-home-agent-workspace/75a3a4d5-f426-4ca2-af38-3fdce8d073d7/scratchpad/sysroot/usr/lib/aarch64-linux-gnu cargo test -p bir-desktop --lib 2>&1 | grep -E "test result|FAILED|panicked|^error"
cargo clippy -p bir-core -- -D warnings 2>&1 | grep -E "^(error|warning: unused)|Finished" | tail -3
cargo clippy -p bir-desktop --all-targets --features dev-tools,agent -- -D warnings 2>&1 | grep -E "^error|Finished" | tail -5
cargo clippy -p bir-desktop -- -D warnings 2>&1 | grep -E "^error|Finished" | tail -5
export CARGO_TARGET_DIR=/home/agent/.cache/codegen-main
touch crates/bir-rules-codegen/src/main.rs
echo "unchecked gate rows: $(cargo run -q -p bir-rules-codegen -- status --boundaries-only 2>&1 | grep -c '\[ \]')"
cargo run -q -p bir-rules-codegen -- check 2>&1 | tail -1
cargo test -p bir-rules-codegen 2>&1 | grep -E "test result|FAILED|panicked"
git status --short | head
echo DONE
