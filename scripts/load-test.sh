#!/usr/bin/env bash
# Load-test the Crabcast API and report p95 latency against the Phase 9
# SLO (target < 50 ms on a dev box; see ROADMAP §7).
#
# Usage:
#   CRABCAST_API=http://localhost:8080 scripts/load-test.sh          # public endpoints
#   CRABCAST_TOKEN=cb_... scripts/load-test.sh                        # also exercises /api/stations (Bearer)
#
# Needs `oha` (https://github.com/hatoo/oha) for percentile output;
# falls back to `hey`/`ab` for a rough p95 when oha is missing.
set -euo pipefail

API="${CRABCAST_API:-http://localhost:8080}"
TOKEN="${CRABCAST_TOKEN:-}"
DURATION="${DURATION:-10s}"
RATE="${RATE:-200}"   # requests/second

TARGETS=(/api/health /api/now-playing)
if [[ -n "$TOKEN" ]]; then
  TARGETS+=(/api/stations)
fi

command -v oha >/dev/null && TOOL=oha || { command -v hey >/dev/null && TOOL=hey || { command -v ab >/dev/null && TOOL=ab || TOOL=; }; }
if [[ -z "$TOOL" ]]; then
  echo "error: need oha, hey, or ab installed" >&2
  echo "install oha:  curl -L -o /usr/local/bin/oha https://github.com/hatoo/oha/releases/latest/download/oha-linux-amd64 && chmod +x /usr/local/bin/oha" >&2
  exit 1
fi
echo "load tester: $TOOL  (duration=$DURATION, rate=$RATE/s)"

for target in "${TARGETS[@]}"; do
  url="${API}${target}"
  echo "--- $target"
  case "$TOOL" in
    oha)
      args=(-z "$DURATION" -q "$RATE" -m GET --output-format json)
      [[ -n "$TOKEN" ]] && args+=(-H "Authorization: Bearer $TOKEN")
      oha "${args[@]}" "$url" | python3 -c '
import json, sys
d = json.load(sys.stdin)
lat = d.get("latencyPercentiles", {})
rps = d.get("rps", {}).get("mean", 0)
print("  rps={:.0f}/s  p50={:.2f}ms  p95={:.2f}ms  p99={:.2f}ms".format(
    rps, lat.get("p50", 0), lat.get("p95", 0), lat.get("p99", 0)))'
      ;;
    hey)
      args=(-z "$DURATION" -q "$RATE")
      [[ -n "$TOKEN" ]] && args+=(-H "Authorization: Bearer $TOKEN")
      hey "${args[@]}" -o csv "$url" | awk -F, 'BEGIN{n=0} NR>1 && $2>=0 {s+=$2; n++; if($2>p95max) p95max=$2} END{print "  samples="n"  avg_ms="(n?s/n:0)"  max_ms="p95max}' || true
      ;;
    ab)
      args=(-n 1000 -c 20 -q)
      [[ -n "$TOKEN" ]] && args+=(-H "Authorization: Bearer $TOKEN")
      ab "${args[@]}" "$url" 2>/dev/null | grep -E "Requests per second|95%" || true
      ;;
  esac
done

echo "done (target p95 < 50 ms per ROADMAP §7)"
