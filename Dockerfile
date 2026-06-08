# Stage 1: Build Rust binary
FROM rust:1.92-slim AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy server source
COPY server/ server/

# Build release binary
WORKDIR /app/server
RUN cargo build --release

# Stage 2: Runtime image
FROM debian:trixie-slim

RUN apt-get update && apt-get install -y ca-certificates tzdata gosu && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -r -g 999 nextapp && useradd -r -u 999 -g nextapp -d /app -s /sbin/nologin nextapp

WORKDIR /app

# Copy binary
COPY --from=builder /app/server/target/release/next-server /app/next-server

# Copy frontend
COPY frontend/ /app/frontend/

# Copy data files (quotes + demo photos for guest mode)
COPY data/quotes.txt /app/data/quotes.txt
COPY data/demo-photos/ /app/data/demo-photos/

# Copy entrypoint script
COPY start.sh /app/start.sh
RUN chmod +x /app/start.sh

# Ensure data directory exists with correct permissions
RUN mkdir -p /data /data/uploads && chown -R nextapp:nextapp /app /data

# Environment
ENV PORT=8080
ENV DATABASE_PATH=/data/next.db
ENV FRONTEND_DIR=/app/frontend

EXPOSE 8080

# Run as root so start.sh can fix /data permissions, then drops to nextapp
CMD ["/app/start.sh"]
