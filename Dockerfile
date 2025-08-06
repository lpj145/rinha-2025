FROM docker.io/rust:1.88.0-bookworm AS builder
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    build-essential \
    ca-certificates \
    && rm -rf /var/lib/apt/lists

WORKDIR /app
COPY . .

# Configurar variáveis de ambiente para otimização Skylake
# ENV RUSTFLAGS="-C target-cpu=skylake -C target-feature=+avx2,+fma,+bmi1,+bmi2,+lzcnt,+popcnt -C opt-level=3 -C codegen-units=1 -C panic=abort"
# ENV CARGO_PROFILE_RELEASE_LTO=true
# ENV CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
# ENV CARGO_PROFILE_RELEASE_PANIC=abort

RUN cargo build --release

FROM gcr.io/distroless/cc-debian12
COPY --from=builder /app/target/release/rinha-2025 /usr/local/bin/

EXPOSE 9999
CMD ["rinha-2025"]