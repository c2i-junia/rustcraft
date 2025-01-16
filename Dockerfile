FROM rust:1.84-slim as builder

RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    cmake \
    clang \
    mold \
    libasound2-dev \
    libudev-dev \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

RUN cargo install just

WORKDIR /app

RUN rustup override set nightly
RUN rustup component add rustc-codegen-cranelift-preview

COPY . .

RUN just generate-release-folder-server


FROM debian:bullseye-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl-dev \
    libasound2 \
    libudev1 \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/release ./rustcraft-server-folder

EXPOSE 8000

CMD [ "./rustcraft-server-folder/bin/rustcraft-server", "--world", "new_world", "--port", "8000"]
