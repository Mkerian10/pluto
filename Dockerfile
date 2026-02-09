FROM rust:1.85-bookworm
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
COPY runtime/ runtime/
COPY stdlib/ stdlib/
COPY tests/ tests/
RUN cargo build
CMD ["cargo", "test"]
