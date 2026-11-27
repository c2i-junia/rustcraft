# The dedicated server, for hosting a world in a container:
#   docker run --read-only -v rustcraft-data:/data -p 8000:8000/udp -p 8001:8001/udp rustcraft-server
FROM debian:bookworm-slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    ca-certificates \
    clang \
    cmake \
    curl \
    git \
    libasound2-dev \
    libssl-dev \
    libudev-dev \
    libwayland-dev \
    libxkbcommon-dev \
    mold \
    pkg-config \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

ENV PATH="/root/.cargo/bin:${PATH}"
ENV RUSTUP_HOME=/root/.rustup CARGO_HOME=/root/.cargo

# The toolchain comes first, so that its layer survives changes to the sources
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain none --profile minimal
WORKDIR /app
COPY rust-toolchain.toml ./
RUN rustup toolchain install

COPY . .

# Like `just generate-release-folder-server`, but with the features of the server alone: the
# workspace unification of .cargo/config.toml would turn on the `dev` feature of the client, and
# link Bevy as a shared library the image does not have. The cache mounts keep the dependencies
# between builds.
RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/.cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release --no-default-features -p server --bin server \
    --config 'resolver.feature-unification="selected"' \
    && cp target/release/server /rustcraft-server


FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    libasound2 \
    libudev1 \
    libwayland-client0 \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

RUN useradd --uid 10001 --no-create-home --home-dir /data --shell /usr/sbin/nologin rustcraft \
    && mkdir /data && chown rustcraft /data

COPY --from=builder /rustcraft-server /usr/local/bin/rustcraft-server
# The content files, which define the blocks and items: the server cannot start without them
COPY data/content /usr/local/share/rustcraft/content

USER rustcraft
ENV HOME=/data
WORKDIR /data
VOLUME /data

# The game port and the status port, which is the game port + 1
EXPOSE 8000/udp 8001/udp

ENTRYPOINT ["rustcraft-server", "--game-folder-path", "/data", \
            "--assets-folder-path", "/usr/local/share/rustcraft"]
CMD ["--world", "world", "--port", "8000"]
