#!/bin/bash
# Isolated review instance: display :98, VNC :5901, noVNC http://localhost:6081/vnc.html,
# own HOME (own database) so it never touches the :99 instance or other sessions.
RT=/home/agent/.cache/rtroot; SL=/tmp/claude-1000/-home-agent-workspace/75a3a4d5-f426-4ca2-af38-3fdce8d073d7/scratchpad/sysroot/usr/lib/aarch64-linux-gnu; RL=$RT/usr/lib/aarch64-linux-gnu
RH=/home/agent/.cache/bir-review-home; LOG=/tmp/claude-1000/-home-agent-workspace/75a3a4d5-f426-4ca2-af38-3fdce8d073d7/scratchpad/vnc/logs; mkdir -p $RH $LOG
pgrep -f 'Xvfb :98' >/dev/null || { Xvfb :98 -screen 0 1440x900x24 -nolisten tcp >$LOG/xvfb98.log 2>&1 & sleep 1; }
pgrep -f 'x11vnc.*:98' >/dev/null || x11vnc -display :98 -forever -shared -nopw -rfbport 5901 -localhost -bg -o $LOG/x11vnc98.log >/dev/null 2>&1
pgrep -f 'websockify.*6081' >/dev/null || { websockify --web /usr/share/novnc 6081 localhost:5901 >$LOG/websockify98.log 2>&1 & }
pkill -f "HOME=$RH" 2>/dev/null; for p in $(pgrep -x bir); do grep -qz "HOME=$RH" /proc/$p/environ 2>/dev/null && kill $p; done; sleep 0.5
env HOME=$RH XDG_DATA_HOME=/home/agent/.local/share DISPLAY=:98 MESA_SHADER_CACHE_DISABLE=true LD_PRELOAD=/tmp/claude-1000/-home-agent-workspace/75a3a4d5-f426-4ca2-af38-3fdce8d073d7/scratchpad/vnc/libnosve.so \
  VK_ICD_FILENAMES=/tmp/claude-1000/-home-agent-workspace/75a3a4d5-f426-4ca2-af38-3fdce8d073d7/scratchpad/vnc/lvp_icd.json LD_LIBRARY_PATH=$RL:$SL /home/agent/.cache/bir-app-bin/bir >$LOG/app98.log 2>&1 &
sleep 8
for p in $(pgrep -x bir); do grep -qz "HOME=$RH" /proc/$p/environ 2>/dev/null && echo "review app pid $p"; done
