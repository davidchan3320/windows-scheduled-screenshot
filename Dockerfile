# syntax=docker/dockerfile:1
FROM rust:1.90.0-slim-bookworm AS toolchain

RUN apt-get update \
    && apt-get install -y --no-install-recommends clang lld llvm python3 python3-venv ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN python3 -m venv /opt/xwin \
    && /opt/xwin/bin/pip install --no-cache-dir cargo-xwin==0.23.1 pefile==2024.8.26 \
    && rustup target add x86_64-pc-windows-msvc
ENV PATH="/opt/xwin/bin:${PATH}" \
    XWIN_ARCH=x86_64 \
    XWIN_CACHE_DIR=/opt/xwin-cache
WORKDIR /workspace

# Keep the prepared SDK in a layer: external caches do not export cache mounts.
# --update checks the pins even when a previous local SDK cache exists.
ENV XWIN_SDK_VERSION=10.0.26100 \
    XWIN_CRT_VERSION=14.44.17.14
RUN --mount=type=cache,target=/opt/xwin-cache,sharing=locked \
    cargo xwin cache xwin --update \
    && mkdir /opt/windows-toolchain \
    && cp -a /opt/xwin-cache/. /opt/windows-toolchain/
ENV XWIN_CACHE_DIR=/opt/windows-toolchain

FROM toolchain AS build
COPY . .
# Restored target-cache timestamps can be newer than checked-out source files.
# Rebuild this package on context changes while retaining compiled dependencies.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/workspace/target,sharing=locked \
    cargo clean --release --target x86_64-pc-windows-msvc --package scheduled-screenshot \
    && cargo xwin build --release --locked --target x86_64-pc-windows-msvc \
    && mkdir /release \
    && cp target/x86_64-pc-windows-msvc/release/ScheduledScreenshot.exe /release/ \
    && python3 scripts/package-release.py /release

FROM scratch AS release
COPY --from=build /release/ /
