FROM docker.io/library/rust:1.69-bookworm AS builder

ARG WASM_SNIP_VERSION_SPEC="^0.4"
ARG WASM_OPT_VERSION_SPEC="^0.112"

RUN rustup target install wasm32-unknown-unknown \
 && cargo install --version "${WASM_SNIP_VERSION_SPEC}" wasm-snip \
 && cargo install --version "${WASM_OPT_VERSION_SPEC}" wasm-opt \
 && cargo new --lib /fetcher

COPY Cargo.toml Cargo.lock /

RUN mv /Cargo.* /fetcher \
 && cd /fetcher \
 && cargo fetch \
 && cd / \
 && rm -rf /fetcher

COPY . /build/

RUN cd build/ && \
    make release

# Real build
FROM scratch
     
LABEL type=application/vnd.module.wasm.content.layer.v1+wasm 
COPY --from=builder /build/container/manifest.yaml /build/container/plugin.wasm ./
