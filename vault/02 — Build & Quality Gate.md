---
tags: [nav/build, progressive-disclosure/L1]
---

# Build & Quality Gate

## Quick Commands

```bash
# Build
CARGO_TARGET_DIR=/tmp/cargo-maintenance-v2 cargo build --release

# Quality Gate Chain (MANDATORY order — all must pass)
CARGO_TARGET_DIR=/tmp/cargo-maintenance-v2 cargo check 2>&1 | tail -20 && \
cargo clippy -- -D warnings 2>&1 | tail -20 && \
cargo clippy -- -D warnings -W clippy::pedantic 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-maintenance-v2 cargo test --lib --release 2>&1 | tail -30

# Run
./target/release/maintenance_engine_v2 start --port 8080

# Health
curl http://localhost:8080/api/health

# Database integrity
for db in data/databases/*.db; do
  echo "$(basename $db): $(sqlite3 $db 'PRAGMA integrity_check;')"
done
```

## Quality Standards

| Rule | Enforcement |
|------|-------------|
| 0 unsafe code | `#![forbid(unsafe_code)]` |
| 0 unwrap/expect | clippy deny |
| 0 clippy warnings | `-D warnings -W clippy::pedantic` |
| 0 compiler warnings | default deny |
| 50+ tests per layer | CI gate |
| Result<T> everywhere | No panic paths |
| No chrono/SystemTime | Use Timestamp + Duration |

## Gate Order

1. `cargo check` — compilation
2. `cargo clippy -- -D warnings` — standard lints
3. `cargo clippy -- -D warnings -W clippy::pedantic` — pedantic lints
4. `cargo test --lib --release` — unit tests

**Never skip a gate. Never proceed to the next layer until the current one passes all 4.**

---

See [[HOME]] | [[05 — Design Constraints]]
