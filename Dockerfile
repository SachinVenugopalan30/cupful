# SPDX-License-Identifier: AGPL-3.0-or-later
# syntax=docker/dockerfile:1

# 1. Frontend. Platform-independent output, so build once on the host's arch.
FROM --platform=$BUILDPLATFORM node:24.22.1-slim AS web
WORKDIR /src/web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

# 2. Server. Cross-compiles on the host's arch to static musl binaries with cargo-zigbuild:
#    zig is the C cross toolchain for the bundled SQLite, so no per-arch builder images or QEMU.
FROM --platform=$BUILDPLATFORM messense/cargo-zigbuild:0.24.0 AS server
ARG TARGETARCH
WORKDIR /src
COPY rust-toolchain.toml ./
COPY server/ server/
COPY --from=web /src/web/build web/build
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/server/target \
    case "$TARGETARCH" in \
      amd64) t=x86_64-unknown-linux-musl ;; \
      arm64) t=aarch64-unknown-linux-musl ;; \
      *) echo "unsupported arch $TARGETARCH" && exit 1 ;; \
    esac \
 && rustup target add "$t" \
 && cd server && SQLX_OFFLINE=true cargo zigbuild --release --locked --target "$t" \
 && cp "target/$t/release/cupful" /cupful \
 && mkdir /data

# 3. Runtime: static binary + CA certs, runs as uid 65532.
FROM gcr.io/distroless/static-debian13:nonroot
COPY --from=server /cupful /cupful
COPY --from=server --chown=65532:65532 /data /data
ENV CUPFUL_BIND=0.0.0.0:8080 CUPFUL_DATA_DIR=/data
VOLUME /data
EXPOSE 8080
ENTRYPOINT ["/cupful"]
CMD ["serve"]
