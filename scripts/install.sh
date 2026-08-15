#!/usr/bin/env bash
# One-command install for Debian/Ubuntu: installs Docker + compose, clones
# Crabcast, builds the production images, and starts the stack. See
# ROADMAP Phase 10.
#
# Usage:
#   curl -sL https://github.com/sonyarianto/crabcast/releases/latest/.../install.sh | bash
#   # or from a checkout:
#   ./scripts/install.sh [--bare-metal]
#
# Sets CRABCAST_SESSION_SECRET for you if unset (printed once).
set -euo pipefail

if [[ "${1:-}" == "--bare-metal" ]]; then
  echo "bare-metal install: see packaging/README.md (systemd units included)" >&2
  exit 0
fi

if [[ $(id -u) -ne 0 ]]; then
  echo "run as root (or with sudo)" >&2
  exit 1
fi

# --- docker + compose -------------------------------------------------------
if ! command -v docker >/dev/null; then
  echo ">> installing docker (get.docker.com)"
  curl -fsSL https://get.docker.com | sh
fi
if ! docker compose version >/dev/null 2>&1; then
  echo ">> installing docker compose plugin"
  apt-get update
  apt-get install -y docker-compose-plugin
fi

# --- checkout ---------------------------------------------------------------
if [[ ! -d .git ]] || [[ "$(basename "$PWD")" != "crabcast" ]]; then
  DEST=/opt/crabcast
  echo ">> cloning crabcast into $DEST"
  mkdir -p /opt
  if [[ ! -d "$DEST/.git" ]]; then
    git clone https://github.com/sonyarianto/crabcast "$DEST"
  fi
  cd "$DEST"
fi

# --- secrets ----------------------------------------------------------------
ENV_FILE=docker/.env
if [[ ! -f "$ENV_FILE" ]] || ! grep -q CRABCAST_SESSION_SECRET "$ENV_FILE"; then
  SECRET=$(head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n')
  umask 077
  touch "$ENV_FILE"
  grep -q '^CRABCAST_SESSION_SECRET=' "$ENV_FILE" || \
    printf 'CRABCAST_SESSION_SECRET=%s\nICECAST_SOURCE_PASSWORD=hackme\nICECAST_ADMIN_PASSWORD=admin\n' "$SECRET" >> "$ENV_FILE"
  echo ">> wrote docker/.env (edit the passwords!)"
fi

# --- build + start ----------------------------------------------------------
echo ">> building and starting the stack (this takes a few minutes)"
docker compose -f docker/compose.prod.yml --env-file "$ENV_FILE" up -d --build

echo
echo "Crabcast is up:"
echo "  web UI     http://localhost:3000"
echo "  API        http://localhost:8080/api/health"
echo "  Icecast    http://localhost:8000"
echo "First visit: create the admin account (bootstrap wizard), then add a"
echo "station pointing playlist_dir at a media folder mounted in the server"
echo "volume (docker exec -it <server> sh, files land in /media)."
