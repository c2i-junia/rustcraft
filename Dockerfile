FROM debian:bookworm-slim as builder

RUN apt-get update && apt-get install -y \
    curl \
    ca-certificates \
    git \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y

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

ENV PATH="/root/.cargo/bin:${PATH}"

RUN cargo install just

WORKDIR /app

RUN rustup override set nightly
RUN rustup component add rustc-codegen-cranelift-preview

COPY . .

RUN just generate-release-folder-server


FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    libssl-dev \
    libasound2 \
    libudev1 \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/release ./rustcraft-server-folder

EXPOSE 8000

CMD [ "./rustcraft-server-folder/bin/rustcraft-server", "--world", "new_world", "--port", "8000"]
