#!/usr/bin/env bash
# D-014 spike: samples PSS of com.checkflat.app and its WebView renderer(s) until Ctrl-C.
# The renderer is the isolated `sandboxed_process` whose ProcessRecord ends in /u0a<appId>i<n>
# (D-006 step 5). Output (UTC, like spike.log): time  app_MB  renderer_MB  total_MB  renderer_pids
# Usage: spikes/pss.sh [-s <serial>] [interval_s]  > spikes/out/pss-<run>.tsv
set -u
ADB=(adb)
if [ "${1:-}" = "-s" ]; then ADB=(adb -s "$2"); shift 2; fi
INTERVAL="${1:-1}"
PKG=com.checkflat.app

# Android 10 prints userId=, newer versions appId=.
uid=$("${ADB[@]}" shell dumpsys package "$PKG" | grep -m1 -oE '(userId|appId)=[0-9]+' | cut -d= -f2)
[ -n "$uid" ] || { echo "package $PKG not installed" >&2; exit 1; }
tag="u0a$((uid % 100000 - 10000))i"

pss_kb() { # first TOTAL line of dumpsys meminfo <pid>: Android 10 "TOTAL <pss> ...", newer "TOTAL PSS: <pss>"
  "${ADB[@]}" shell dumpsys meminfo "$1" 2>/dev/null | tr -d '\r' |
    awk '/TOTAL PSS:/ {print $3; exit} /^ *TOTAL +[0-9]/ {print $2; exit}'
}

printf 'time\tapp_MB\trenderer_MB\ttotal_MB\trenderer_pids\n'
while true; do
  now=$(date -u +%H:%M:%S)
  app_pid=$("${ADB[@]}" shell pidof "$PKG" | tr -d '\r')
  r_pids=$("${ADB[@]}" shell dumpsys activity processes | tr -d '\r' |
    grep -o "ProcessRecord{[0-9a-f]* [0-9]*:[^}]*/${tag}[0-9]*}" | sed -E 's/ProcessRecord\{[0-9a-f]+ ([0-9]+):.*/\1/' | sort -u | tr '\n' ' ')
  app=0; [ -n "$app_pid" ] && app=$(pss_kb "$app_pid"); app=${app:-0}
  rend=0
  for p in $r_pids; do k=$(pss_kb "$p"); rend=$((rend + ${k:-0})); done
  printf '%s\t%.1f\t%.1f\t%.1f\t%s\n' "$now" "$(echo "$app/1024" | bc -l)" "$(echo "$rend/1024" | bc -l)" "$(echo "($app+$rend)/1024" | bc -l)" "$r_pids"
  sleep "$INTERVAL"
done
