#!/bin/bash
# Link bir-desktop's `bir` binary against the unpacked runtime libs.
set -e
source /tmp/claude-1000/-home-agent-workspace/75a3a4d5-f426-4ca2-af38-3fdce8d073d7/scratchpad/env.sh; export PATH=$HOME/.cargo/bin:$PATH  # bypass the mbx shim while the disk is nearly full
RT=/home/agent/.cache/rtroot; SL=/tmp/claude-1000/-home-agent-workspace/75a3a4d5-f426-4ca2-af38-3fdce8d073d7/scratchpad/sysroot/usr/lib/aarch64-linux-gnu; RL=$RT/usr/lib/aarch64-linux-gnu
# Make the sysroot's dev symlinks (libfoo.so -> libfoo.so.N) resolve by linking the runtime .so.N files in.
for f in $RL/*.so.* /usr/lib/aarch64-linux-gnu/*.so.*; do b=$(basename $f); [ -e $SL/$b ] || ln -sf $f $SL/$b; done
# zig's linker only finds unversioned libfoo.so; point those at the runtime libfoo.so.N files.
LK=/tmp/claude-1000/-home-agent-workspace/75a3a4d5-f426-4ca2-af38-3fdce8d073d7/scratchpad/vnc/linklibs; mkdir -p $LK
for d in $RL /usr/lib/aarch64-linux-gnu; do for f in $d/lib*.so.[0-9]*; do b=$(basename $f); n=${b%%.so.*}.so; [ -e $LK/$n ] || ln -s $f $LK/$n; done; done
export RUSTFLAGS="$RUSTFLAGS -L $LK -L $SL -L $RL -C link-arg=-lzstd -C link-arg=-ljitterentropy"
export CARGO_TARGET_DIR=/home/agent/.cache/buwiz-target
cd ${1:-/tmp/claude-1000/-home-agent-workspace/75a3a4d5-f426-4ca2-af38-3fdce8d073d7/scratchpad/forms}
cargo build -p bir-desktop --bin bir 2>&1 | grep -E '^(error|warning: unused)|-->|Finished|could not' | head -30
cp $CARGO_TARGET_DIR/debug/bir /home/agent/.cache/bir-app-bin/bir.new && mv /home/agent/.cache/bir-app-bin/bir.new /home/agent/.cache/bir-app-bin/bir && ls -la /home/agent/.cache/bir-app-bin/bir && echo BUILD_DONE
