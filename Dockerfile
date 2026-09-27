FROM rust:1.97.1-bookworm@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97 AS build
WORKDIR /build
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY src ./src
RUN cargo build --locked --release

FROM debian:bookworm-slim@sha256:3783cc01769c7b2b1b83a5c5ad96c815348e28ed7da68e2e3687004faa906251 AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 10001 --create-home research \
    && mkdir /workspaces && chown research:research /workspaces
COPY --from=build /build/target/release/mcp-deepresearch /usr/local/bin/mcp-deepresearch
USER research
WORKDIR /workspaces
ENV DEEPRESEARCH_MCP_LISTEN=0.0.0.0:8088 DEEPRESEARCH_WORKFLOW_LISTEN=0.0.0.0:9080
ENTRYPOINT ["mcp-deepresearch"]
CMD ["serve", "fixture", "/workspaces"]

FROM runtime AS live
USER root
ARG NPM_CONFIG_REGISTRY
RUN apt-get update && apt-get install -y --no-install-recommends nodejs npm \
    && npm install --global @openai/codex@0.157.0 \
    && npm cache clean --force \
    && rm -rf /var/lib/apt/lists/*
USER research
CMD ["serve", "live", "/workspaces"]
