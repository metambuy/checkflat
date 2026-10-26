#!/usr/bin/env bash
# Sprint 6b (D-029): samples the PSS of com.checkflat.app (the app process only, not the WebView
# renderer) as fast as dumpsys allows. The loop runs on the device, so no adb round trip is paid
# per sample (≈ 8 samples/s on the P30, D-027). This catches the 1–2 s spike at the end of a report
# that spikes/pss.sh's ≈ 2 s sampling can miss; run it next to pss.sh, which gives the renderer.
# Output (until Ctrl-C): time_utc  uptime_s  app_MB
# Usage: spikes/pss-app-fast.sh [-s <serial>]  > spikes/out/pss-fast-<run>.tsv
set -u
ADB=(adb)
if [ "${1:-}" = "-s" ]; then ADB=(adb -s "$2"); shift 2; fi
PKG=com.checkflat.app

printf 'time\tuptime_s\tapp_MB\n'
# Android 10 prints "TOTAL <pss> …", newer versions "TOTAL PSS: <pss> …": the first number is the PSS.
"${ADB[@]}" shell "while true; do p=\$(pidof $PKG); if [ -n \"\$p\" ]; then echo \"\$(date -u +%H:%M:%S) \$(cut -d' ' -f1 /proc/uptime) \$(dumpsys meminfo \$p | grep -m1 TOTAL)\"; else sleep 0.2; fi; done" |
  tr -d '\r' |
  awk '{ for (i = 3; i <= NF; i++) if ($i ~ /^[0-9]+$/) { printf "%s\t%s\t%.1f\n", $1, $2, $i / 1024; fflush(); break } }'
