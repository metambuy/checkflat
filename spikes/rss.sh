#!/usr/bin/env bash
# Sprint 5 (D-024): samples the resident set of the desktop app while a report is generated,
# until Ctrl-C. macOS/Linux counterpart of pss.sh. The WebView content process is listed too
# (every WebKit content process on the machine — attribution to one app is not possible from
# ps), but report generation runs entirely in the app process.
# Output: time  app_MB  webcontent_MB   Usage: spikes/rss.sh [process name] [interval_s]
set -u
NAME="${1:-checkflat}"
INTERVAL="${2:-0.5}"
printf 'time\tapp_MB\twebcontent_MB\n'
while true; do
  app=$(pgrep -x "$NAME" | head -1)
  app_kb=0; [ -n "$app" ] && app_kb=$(ps -o rss= -p "$app" | tr -d ' ')
  web_kb=$(ps -axo rss=,comm= | awk '/WebKit.WebContent/ {s += $1} END {print s + 0}')
  printf '%s\t%.1f\t%.1f\n' "$(date +%H:%M:%S)" "$(echo "${app_kb:-0}/1024" | bc -l)" "$(echo "$web_kb/1024" | bc -l)"
  sleep "$INTERVAL"
done
