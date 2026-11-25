# The hub and its tools. The build context is the repository root:
#   docker build --file hub/Dockerfile --target hub --tag rustcraft-hub .
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
    cargo build --release --locked \
    && cp target/release/rustcraft-hub /rustcraft-hub


FROM gcr.io/distroless/cc-debian12 AS hub

COPY --from=builder /rustcraft-hub /usr/local/bin/rustcraft-hub

USER nonroot
ENTRYPOINT ["rustcraft-hub", "serve"]
