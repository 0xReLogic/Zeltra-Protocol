FROM debian:bookworm-slim

# Install standard tools, openssl for TLS connections to RPC, and ca-certificates
RUN apt-get update && apt-get install -y \
    openssl \
    ca-certificates \
    sqlite3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# The compiled binary will be copied to root as nimbus-node-bin before build
COPY ./nimbus-node-bin /usr/local/bin/nimbus-node

CMD ["/usr/local/bin/nimbus-node"]
