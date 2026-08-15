# Bare-metal install (systemd)

For Debian/Ubuntu hosts running Crabcast **without Docker**. The Docker
route is simpler — see `../scripts/install.sh` and `../docker/compose.prod.yml`.

## 1. Build & install the API

```sh
# Prereqs: Rust toolchain, the crabsoup binary on PATH (CRABSOUP_BIN)
cargo build --release --manifest-path server/Cargo.toml
sudo install -m 0755 server/target/release/crabcast-server /usr/local/bin/crabcast-server
sudo install -m 0755 /path/to/crabsoup /usr/local/bin/crabsoup   # the engine

# Data user + dirs
sudo useradd -r -m -d /var/lib/crabcast crabcast
sudo mkdir -p /var/lib/crabcast/{data,media} /etc/crabcast
sudo chown -R crabcast:crabcast /var/lib/crabcast
```

## 2. Environment

```sh
sudo tee /etc/crabcast/env >/dev/null <<'EOF'
DATABASE_URL=sqlite:/var/lib/crabcast/crabcast.db
BIND_ADDR=127.0.0.1:8080
CRABCAST_DATA_DIR=/var/lib/crabcast/data
CRABCAST_MEDIA_DIR=/var/lib/crabcast/media
CRABCAST_SESSION_SECRET=<openssl rand -hex 32>
CRABCAST_ALERT_WEBHOOK_URL=
# Optional: share the SSE/event bus across API replicas (horizontal scale)
CRABCAST_REDIS_URL=redis://127.0.0.1:6379
EOF
sudo chmod 600 /etc/crabcast/env
```

## 3. Build the web app (standalone)

```sh
npm ci --prefix web && npm run build --prefix web
sudo mkdir -p /opt/crabcast
sudo cp -r web /opt/crabcast/web        # includes .next/standalone
sudo chown -R crabcast:crabcast /opt/crabcast
```

## 4. Install the units

```sh
sudo install -m 0644 packaging/crabcast-server.service /etc/systemd/system/
sudo install -m 0644 packaging/crabcast-web.service   /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now crabcast-server crabcast-web
```

Optional: Icecast on the same host (`apt install icecast2`), then point
each station's Icecast host at `127.0.0.1`.

## Upgrades

Rebuild both artifacts, then `sudo systemctl restart crabcast-server
crabcast-web`. Migrations run automatically at boot; back up
`/var/lib/crabcast` first (see the backup API in the admin UI).
