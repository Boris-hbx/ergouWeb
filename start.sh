#!/bin/sh
# Fix database file permissions on startup.
# The /data volume may have been initialized with root-owned files.
# This script runs as root to fix permissions, then starts the server as nextapp.
echo "[start.sh] Fixing /data permissions..."
chown -R nextapp:nextapp /data 2>/dev/null || true
chmod 664 /data/next.db* 2>/dev/null || true

# Insight Factory worker (T-217): host the Codex subscription identity on the
# persistent volume. CODEX_HOME=/data/.codex survives restarts; auth.json is
# injected from the Fly secret CODEX_AUTH_JSON_B64 (base64) so it never lands in
# an image layer. The worker runs as uid 999 (nextapp), so the dir must be owned
# by nextapp. NEVER echo the auth.json contents.
CODEX_HOME_DIR="${CODEX_HOME:-/data/.codex}"
mkdir -p "$CODEX_HOME_DIR"
if [ -n "${CODEX_AUTH_JSON_B64:-}" ]; then
    if printf '%s' "$CODEX_AUTH_JSON_B64" | base64 -d > "$CODEX_HOME_DIR/auth.json" 2>/dev/null; then
        chmod 600 "$CODEX_HOME_DIR/auth.json"
        echo "[start.sh] Codex auth.json injected from secret"
    else
        echo "[start.sh] WARNING: failed to decode CODEX_AUTH_JSON_B64 (worker will report auth_missing)"
        rm -f "$CODEX_HOME_DIR/auth.json"
    fi
else
    echo "[start.sh] CODEX_AUTH_JSON_B64 not provided; Codex worker will report auth_missing until injected"
fi
chown -R nextapp:nextapp "$CODEX_HOME_DIR" 2>/dev/null || true

# Pre-deploy backup: snapshot before new code runs
if [ -f /data/next.db ]; then
    mkdir -p /data/backups
    STAMP=$(date +%Y%m%d-%H%M%S)
    cp /data/next.db "/data/backups/pre-deploy-${STAMP}.db" 2>/dev/null && \
        echo "[start.sh] Pre-deploy backup: pre-deploy-${STAMP}.db" || \
        echo "[start.sh] Pre-deploy backup failed (non-fatal)"
    # Keep only last 10 pre-deploy backups
    ls -t /data/backups/pre-deploy-*.db 2>/dev/null | tail -n +11 | xargs rm -f 2>/dev/null
fi

echo "[start.sh] Starting next-server as nextapp..."
exec gosu nextapp /app/next-server
