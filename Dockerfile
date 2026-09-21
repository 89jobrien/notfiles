# Dockerfile — build and run the notfiles e2e integration test suite
#
# Usage:
#   docker build -t notfiles-e2e .
#   docker run --rm notfiles-e2e

FROM rust:1.88-slim AS builder

WORKDIR /src

# ── Layer 1: cache deps ────────────────────────────────────────────────────────
# Copy workspace manifests + real library source so Cargo can resolve the dep
# graph and compile dependencies. Only stub the binary entry points (main.rs /
# test files) which aren't cached as separate artifacts. This layer is
# invalidated only when manifests, Cargo.lock, or library source changes.
COPY Cargo.toml Cargo.lock ./
COPY crates/notcore    crates/notcore
COPY crates/notfiles   crates/notfiles
COPY crates/nothooks   crates/nothooks
COPY crates/notsecrets crates/notsecrets
COPY crates/notnet     crates/notnet
COPY crates/notstrap   crates/notstrap

# Copy only manifests for the two broken crates — no source needed since
# we never compile them. Workspace resolution requires their Cargo.toml.
COPY crates/notgraph/Cargo.toml  crates/notgraph/Cargo.toml
COPY crates/notforge/Cargo.toml  crates/notforge/Cargo.toml
COPY tests/integration/Cargo.toml tests/integration/Cargo.toml

# Provide minimal src stubs for the broken crates and integration tests so
# Cargo can resolve workspace members without compiling broken source.
RUN for crate in notgraph notforge; do \
      mkdir -p crates/$crate/src; \
      touch crates/$crate/src/lib.rs; \
      printf 'fn main() {}\n' > crates/$crate/src/main.rs; \
    done && \
    mkdir -p tests/integration/tests && \
    printf '#[test] fn stub() {}\n' > tests/integration/tests/bootstrap.rs && \
    printf '#[test] fn stub() {}\n' > tests/integration/tests/cross_crate.rs

RUN cargo build -p notfiles --tests

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

WORKDIR /app

# The test binary locates `notfiles` via:
#   current_exe() → pop binary name → pop "deps" → push "notfiles"
# Required layout:
#   /app/notfiles          ← the CLI binary
#   /app/deps/<test-bin>   ← the test binary (current_exe lives here)
COPY --from=builder /src/target/debug/notfiles                    ./notfiles
COPY --from=builder /src/target/debug/deps/integration-*         ./deps/
# CARGO_MANIFEST_DIR is baked in as /src/crates/notfiles at compile time
COPY --from=builder /src/crates/notfiles/tests/fixtures           /src/crates/notfiles/tests/fixtures

# Drop non-ELF files (*.d dependency metadata files, zero-byte stubs)
RUN find deps -maxdepth 1 -type f | while read f; do \
      head -c 4 "$f" | grep -qP '^\x7fELF' || rm -f "$f"; \
    done

# Run all integration test binaries; propagate failure.
CMD ["sh", "-c", \
  "rc=0; \
   for bin in deps/integration-*; do \
     [ -x \"$bin\" ] || continue; \
     echo \"=== $bin ===\"; \
     \"$bin\"; rc=$((rc + $?)); \
   done; \
   exit $rc"]
