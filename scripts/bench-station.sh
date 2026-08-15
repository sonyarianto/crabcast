#!/usr/bin/env bash
# Station-scaling benchmark: creates N stations against a running Crabcast
# API and samples each station's crabsoup process (RSS, %CPU) plus API
# latency for a duration, printing a summary table. Use it to validate the
# "stations per VPS" and "idle CPU per station" SLOs in ROADMAP §7.
#
# Prereqs: a running API (make dev / cargo run), a CRABCAST_TOKEN with
# station_manager permission, and a playlist dir with real audio.
#
# Usage:
#   CRABCAST_API=http://localhost:8080 CRABCAST_TOKEN=cb_... \
#   PLAYLIST_DIR=/path/with/audio N=5 DURATION=600 scripts/bench-station.sh
set -euo pipefail

API="${CRABCAST_API:-http://localhost:8080}"
TOKEN="${CRABCAST_TOKEN:?set CRABCAST_TOKEN (create one in Settings -> API tokens)}"
PLAYLIST_DIR="${PLAYLIST_DIR:?set PLAYLIST_DIR to a directory with audio files}"
N="${N:-3}"
DURATION="${DURATION:-600}"      # seconds to sample
INTERVAL="${INTERVAL:-30}"       # seconds between samples
DATA_DIR="${DATA_DIR:-station-data}"  # server's CRABCAST_DATA_DIR (configs/<id>/)

[[ "$PLAYLIST_DIR" == /* ]] || PLAYLIST_DIR="$(realpath "$PLAYLIST_DIR")"
command -v curl >/dev/null || { echo "need curl"; exit 1; }

AUTH=(-H "Authorization: Bearer $TOKEN")

echo "creating $N stations (playlist dir: $PLAYLIST_DIR) ..."
ids=()
for i in $(seq 1 "$N"); do
  name="bench-$(date +%s)-$i"
  id=$(curl -s "${AUTH[@]}" -H 'Content-Type: application/json' \
    -X POST "$API/api/stations" \
    -d "{\"name\":\"$name\",\"playlist_dir\":\"$PLAYLIST_DIR\"}" \
    | python3 -c "import json,sys; print(json.load(sys.stdin).get('id',''))")
  if [[ -z "$id" ]]; then
    echo "error creating station $i" >&2
    exit 1
  fi
  ids+=("$id")
done
echo "created: ${ids[*]}"

# Sum RSS (KiB) and %CPU for every crabsoup process, attributed per station
# by the config path in its command line.
sample() {
  local station_id="$1"
  ps -eo rss=,pcpu=,args= | awk -v id="$station_id" -v pat="$DATA_DIR/configs/$station_id/crabsoup.lua" '
    index($0, pat) {
      rss += $1; cpu += $2
    }
    END { printf "%.1f %.1f", rss / 1024.0, cpu }
  '
}

echo "sampling every ${INTERVAL}s for ${DURATION}s ..."
samples=()
elapsed=0
while (( elapsed < DURATION )); do
  for id in "${ids[@]}"; do
    read -r mb cpu < <(sample "$id")
    samples+=("$id $mb $cpu")
  done
  sleep "$INTERVAL"
  elapsed=$((elapsed + INTERVAL))
done

echo
printf "%-40s %12s %12s\n" "station" "avg RSS (MiB)" "avg %CPU"
for id in "${ids[@]}"; do
  awk -v id="$id" '
    $1 == id { rss += $2; cpu += $3; n++ }
    END { printf "%-40s %12.1f %12.2f\n", id, rss/n, cpu/n }
  ' <<<"$(printf '%s\n' "${samples[@]}")"
done

if command -v oha >/dev/null; then
  echo
  echo "API /api/now-playing p95 over ${DURATION}s (run scripts/load-test.sh for more):"
  oha -z "${DURATION}s" -q 50 --json "$API/api/now-playing" | python3 -c '
import json, sys
d = json.load(sys.stdin)
print(f"  rps={d.get(\"rps\",\"?\")}  p95={d.get(\"percentiles\",{}).get(0.95,\"?\"):.1f}ms")'
fi

echo
echo "cleanup: delete the stations via the admin UI or:"
for id in "${ids[@]}"; do
  echo "  curl -X DELETE ${AUTH[*]} $API/api/stations/$id"
done
