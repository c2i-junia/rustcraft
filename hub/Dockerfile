# The hub and its realm agent. The build context is the repository root:
#   docker build --file hub/Dockerfile --target hub --tag rustcraft-hub .
#   docker build --file hub/Dockerfile --target realm-agent --tag rustcraft-realm-agent .
# The web admin (hub/admin), served by the hub under /admin/
FROM oven/bun:1.4.4 AS admin-ui

WORKDIR /admin
COPY hub/admin/package.json hub/admin/bun.lock ./
RUN bun install --frozen-lockfile
COPY hub/admin ./
RUN bun run build


FROM debian:bookworm-slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    clang \
    curl \
    mold \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

ENV PATH="/root/.cargo/bin:${PATH}"
ENV RUSTUP_HOME=/root/.rustup CARGO_HOME=/root/.cargo

# The toolchain comes first, so that its layer survives changes to the sources
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain none --profile minimal
WORKDIR /app
COPY rust-toolchain.toml ./
RUN rustup toolchain install

# Only what the hub builds from. The root manifest is there because `crates/protocol` inherits
# its lints from the game's workspace.
COPY Cargo.toml ./
COPY .cargo .cargo
COPY crates/protocol crates/protocol
COPY hub hub

WORKDIR /app/hub
RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/.cargo/git \
    --mount=type=cache,target=/app/hub/target \
    cargo build --release --locked --workspace \
    && cp target/release/rustcraft-hub target/release/rustcraft-realm-agent /


# Before `hub`, which stays the image of a build without --target
FROM gcr.io/distroless/cc-debian12 AS realm-agent

COPY --from=builder /rustcraft-realm-agent /usr/local/bin/rustcraft-realm-agent

# Root, as the owner of the host's docker.sock (root:docker, 0660) whatever the gid of the host's
# docker group: holding that socket is as good as root on the host anyway. The group is the
# hub's (nonroot, 65532), which the agent's socket (0660) gets, so that the hub can connect.
USER 0:65532
ENTRYPOINT ["rustcraft-realm-agent", "--config", "/etc/rustcraft/realm-agent.toml"]


FROM gcr.io/distroless/cc-debian12 AS hub

COPY --from=builder /rustcraft-hub /usr/local/bin/rustcraft-hub
COPY --from=admin-ui /admin/build /usr/share/rustcraft-hub/admin

USER nonroot
ENTRYPOINT ["rustcraft-hub", "serve"]
