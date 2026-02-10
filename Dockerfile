FROM rust:1.85-bookworm

RUN rustup component add clippy

WORKDIR /app

# Copy workspace manifests
COPY Cargo.toml Cargo.lock ./
COPY sdk/Cargo.toml sdk/Cargo.toml
COPY mcp/Cargo.toml mcp/Cargo.toml

# Copy all source
COPY src/ src/
COPY sdk/src/ sdk/src/
COPY mcp/src/ mcp/src/
COPY runtime/ runtime/
COPY stdlib/ stdlib/
COPY tests/ tests/
COPY data/ data/

CMD ["cargo", "test", "--workspace"]
