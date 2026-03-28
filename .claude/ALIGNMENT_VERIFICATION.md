# Alignment Verification — Maintenance Engine V2

> **Last Verified:** 2026-03-28 | **Result:** 47/47 PASS | **Session:** 068

---

## Triple Alignment Check

Three pairs must stay in sync:

### Pair A: SOURCE (src/) ↔ CLAUDE.md

Every module in `.claude/CLAUDE.md` must have a corresponding `.rs` source file, and vice versa.

```bash
# A1: Count source files (exclude mod.rs)
find src -name "*.rs" ! -name "mod.rs" | wc -l
# Expected: 58 (47 modules + lib.rs + main.rs + engine.rs + database.rs + 7 tools)

# A2: Check for undocumented .rs files
find src -name "*.rs" ! -name "mod.rs" | sed "s|src/||" | while read f; do
  grep -qF "$f" .claude/CLAUDE.md || echo "UNDOCUMENTED: $f"
done
# Expected: zero output
```

### Pair B: CLAUDE.md ↔ MODULE_MATRIX (ai_specs)

Module IDs in CLAUDE.md must match MODULE_MATRIX.md rows.

```bash
# B1: Extract module IDs from both files
comm -23 \
  <(grep -oP 'M\d{2}' .claude/CLAUDE.md | sort -u) \
  <(grep -oP 'M\d{2}' ai_specs/MODULE_MATRIX.md | sort -u)
# Expected: empty (after MODULE_MATRIX updated to include M40-M47)

# B2: Check for PLANNED modules that have source files
grep 'PLANNED' ai_specs/MODULE_MATRIX.md | grep -oP 'M\d{2}' | while read mid; do
  find src -name "*.rs" -exec head -1 {} \; | grep -q "$mid" && echo "STALE PLANNED: $mid"
done
# Expected: zero output
```

### Pair C: MODULE_MATRIX ↔ ai_docs

Every module in MODULE_MATRIX must have a documentation file.

```bash
# C1: Check each module has a doc file
grep -oP 'M\d{2}' ai_specs/MODULE_MATRIX.md | sort -u | while read mid; do
  ls ai_docs/modules/${mid}_*.md >/dev/null 2>&1 || echo "NO DOC: $mid"
done
# Expected: zero output (M40-M47 docs created session 068)

# C2: Verify module count consistency
echo "context.json modules: $(python3 -c 'import json; print(json.load(open(\".claude/context.json\"))[\"project\"][\"modules\"])')"
echo "MODULE_MATRIX rows: $(grep -cP '^\| M\d{2}' ai_specs/MODULE_MATRIX.md)"
echo "ai_docs/modules files: $(ls ai_docs/modules/M*.md 2>/dev/null | wc -l)"
# Expected: all three numbers agree
```

## Quick Verification (One-Liner)

```bash
ME=/home/louranicas/claude-code-workspace/the_maintenance_engine
PASS=0; FAIL=0
for m in M01 M02 M03 M04 M05 M06 M07 M08 M09 M10 M11 M12 M13 M14 M15 M16 M17 M18 M19 M20 M21 M22 M23 M24 M25 M26 M27 M28 M29 M30 M31 M32 M33 M34 M35 M36 M37 M38 M39 M40 M41 M42 M43 M44 M45 M46 M47; do
  const=$(rg -q "pub const ${m}:" "$ME/src/m1_foundation/shared_types.rs" 2>/dev/null && echo "OK" || echo "FAIL")
  doc=$(ls "$ME/ai_docs/modules/${m}_"*.md >/dev/null 2>&1 && echo "OK" || echo "FAIL")
  if [ "$const" = "OK" ] && [ "$doc" = "OK" ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "FAIL: $m (const=$const doc=$doc)"; fi
done
echo "Result: $PASS PASS, $FAIL FAIL"
```

## ModuleId Registration Check

```bash
# Verify ModuleId::ALL array size matches deployed count
rg 'pub const ALL: \[Self; (\d+)\]' src/m1_foundation/shared_types.rs
# Expected: [Self; 47]

# Verify layer() match covers all IDs
rg -c 'Some\(' src/m1_foundation/shared_types.rs
# Expected: 7 match arms (one per layer)
```

## Quality Gate Verification

```bash
CARGO_TARGET_DIR=/tmp/cargo-maintenance cargo check 2>&1 | tail -3
CARGO_TARGET_DIR=/tmp/cargo-maintenance cargo clippy -- -D warnings 2>&1 | tail -3
CARGO_TARGET_DIR=/tmp/cargo-maintenance cargo clippy -- -D warnings -W clippy::pedantic 2>&1 | tail -3
CARGO_TARGET_DIR=/tmp/cargo-maintenance cargo test --lib --release 2>&1 | tail -5
# All 4 stages must show zero errors and zero warnings
```

## Verification History

| Date | Session | Result | Modules | Tests | Notes |
|------|---------|--------|---------|-------|-------|
| 2026-03-28 | 068 | 47/47 PASS | 47 | 2,288 | M43-M47 registered, docs migrated/created |
| 2026-02-28 | — | 42/42 PASS | 42 | 1,536 | L2 refactor complete |
| 2026-02-20 | — | 42/42 PASS | 42 | 1,536 | HRS-001 M40-M42 added |
| 2026-01-31 | — | 36/36 PASS | 36 | 1,528 | Initial integration |
