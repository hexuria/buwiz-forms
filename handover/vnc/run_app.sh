#!/bin/bash
# Serve the app at http://localhost:6080/vnc.html (Xvfb :99 -> x11vnc :5900 -> websockify/noVNC :6080).
RT=/home/agent/.cache/rtroot; SL=/tmp/claude-1000/-home-agent-workspace/75a3a4d5-f426-4ca2-af38-3fdce8d073d7/scratchpad/sysroot/usr/lib/aarch64-linux-gnu; RL=$RT/usr/lib/aarch64-linux-gnu
LOG=/tmp/claude-1000/-home-agent-workspace/75a3a4d5-f426-4ca2-af38-3fdce8d073d7/scratchpad/vnc/logs; mkdir -p $LOG
pgrep -x Xvfb >/dev/null || { Xvfb :99 -screen 0 1440x900x24 -nolisten tcp >$LOG/xvfb.log 2>&1 & sleep 1; }
pgrep -x x11vnc >/dev/null || x11vnc -display :99 -forever -shared -nopw -rfbport 5900 -localhost -bg -o $LOG/x11vnc.log >/dev/null 2>&1
pgrep -f 'websockify.*6080' >/dev/null || { websockify --web /usr/share/novnc 6080 localhost:5900 >$LOG/websockify.log 2>&1 & }
# Software Vulkan (lavapipe) from the unpacked Mesa; point its manifest at the unpacked driver.
ICD=$(ls $RT/usr/share/vulkan/icd.d/lvp_icd*.json | head -1)
sed "s#\"library_path\": *\"[^\"]*/#\"library_path\": \"$RL/#" $ICD > /tmp/claude-1000/-home-agent-workspace/75a3a4d5-f426-4ca2-af38-3fdce8d073d7/scratchpad/vnc/lvp_icd.json
pkill -x bir 2>/dev/null; sleep 0.5
DISPLAY=:99 MESA_SHADER_CACHE_DISABLE=true LD_PRELOAD=/tmp/claude-1000/-home-agent-workspace/75a3a4d5-f426-4ca2-af38-3fdce8d073d7/scratchpad/vnc/libnosve.so VK_ICD_FILENAMES=/tmp/claude-1000/-home-agent-workspace/75a3a4d5-f426-4ca2-af38-3fdce8d073d7/scratchpad/vnc/lvp_icd.json LD_LIBRARY_PATH=$RL:$SL \
  ZED_ALLOW_EMULATED_GPU=1 /home/agent/.cache/bir-app-bin/bir >$LOG/app.log 2>&1 &
sleep 8
echo "app pid: $(pgrep -x bir || echo none)"; tail -15 $LOG/app.log
(command -v ss >/dev/null && ss -ltn || netstat -ltn 2>/dev/null) | grep -E ":6080|:5900" || echo "ports: check with curl"
