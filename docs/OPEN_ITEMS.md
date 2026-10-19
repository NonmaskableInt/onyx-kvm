# Open items and recommendations

Reviewed and updated 2026-10-18 against `main` (standardized to `onyx-kvm` per [NAMING.md](NAMING.md)).
Each item cites where it was observed, the design decisions taken, and the resolution status.

Priority: **P1** blocks an honest "hardened release" claim or is a security
gap against the stated [threat model](THREAT_MODEL.md); **P2** blocks
reproducibility or contributor workflow; **P3** is hygiene.

The hardware bring-up log stays in [NEXT_STEPS.md](NEXT_STEPS.md) (with older history in [docs/history/EXPERIMENT_HISTORY.md](history/EXPERIMENT_HISTORY.md)); this file
tracks open items, security findings, and their resolutions.

---

## Security

### S1 (P1): network `/setup/claim` lets any LAN client take an unconfigured device — RESOLVED

**Previous State**:
`handle_setup_claim` was reachable over HTTPS by any client on the LAN when unconfigured. It accepted a 4-digit PIN, minted a full operator token with `kvm:admin`, and had no rate limiting.

**Resolution**:
- **Origin Validation**: Enforced strict `Origin` header matching in `handle_setup_claim` (`crates/onyx-kvmd/src/https.rs`), rejecting requests with missing or mismatched origins.
- **PIN Complexity**: Enforced 6 to 8 digit minimum PIN length in both daemon backend and web UI.
- **Rate Limiting**: Added `MAX_SETUP_CLAIM_ATTEMPTS = 5` lockout. After 5 failed unconfigured claim attempts, the endpoint is locked out to prevent brute-force attacks across the local subnet.
- **Least-Privilege Scoping**: Setup claim now mints `mint_console_token` (`kvm:read kvm:video kvm:control kvm:audio-*`) without `kvm:admin`.
- **Verification**: Verified with unit test `https::tests::tofu_setup_lifecycle_and_pin_authentication`.

### S2 (P1): the microSD card is a stated adversary, but it controls root — RESOLVED

**Previous State**:
Threat model included an attacker with temporary write access to the microSD card, but the FAT boot partition accepted `/boot/DEV_MODE`, `/boot/RESET`, and `/boot/onyx-kvm-pin.hash`.

**Resolution**:
- **Threat Model Narrowing**: Updated [THREAT_MODEL.md](THREAT_MODEL.md) to explicitly narrow the removable microSD card adversary model to "read-only access to the microSD card" (e.g., pulling the card to extract firmware or offline credentials).
- **Physical Boundary Acknowledgment**: Documented that on un-fused hardware lacking hardware root-of-trust and cryptographic signature verification in ROM/FSBL, physical write access to the boot media inherently grants control over boot artifacts.
- **Documented in Operator Guide**: Documented physical reset and dev-mode boundaries in [OPERATOR_GUIDE.md](OPERATOR_GUIDE.md).

### S3 (P1, verified): development tooling ships in any profile when present — RESOLVED

**Previous State**:
`image/build-phase1-rootfs.sh` installed `out/dev-dropbear-v1/dropbear` into `/usr/lib/onyx-kvm/dev/` whenever present, regardless of profile.

**Resolution**:
- **Build-Time Gating**: Gated dev Dropbear installation in `image/build-phase1-rootfs.sh` behind explicit `ONYX_KVM_DEV_TOOLS=1`.
- **Profile Cleansing**: In `image/build-phase1-rootfs.sh`, explicitly removed `/usr/lib/onyx-kvm/dev` and `S35onyx-kvm-devmode` for the `product` profile.
- **Audit Verification**: Updated `image/audit-rootfs.sh` to fail the `product` profile audit if `/usr/lib/onyx-kvm/dev` or `S35onyx-kvm-devmode` is present.

### S4 (P2): operator-grade tokens are the only token the setup and pairing flows mint — RESOLVED

**Previous State**:
Both `mint_operator_token` and the pairing ceremony issued full scopes including `kvm:admin`.

**Resolution**:
- **Console Token Minting**: Added `mint_console_token` in `crates/onyx-kvm-mcp/src/lib.rs` minting least-privilege tokens (`kvm:read kvm:video kvm:control kvm:audio-*`) without `kvm:admin`.
- **Ceremony Least Privilege**: Web setup claim and pairing flows now mint console tokens. Operator tokens with `kvm:admin` are reserved for explicit administrative operations.

### S5 (P2): unbounded connection handling — RESOLVED

**Previous State**:
`serve` spawned a new thread per connection with no concurrency limit.

**Resolution**:
- **Connection Cap**: Added `MAX_CONCURRENT_CONNECTIONS = 16` cap in `crates/onyx-kvmd/src/https.rs` using an atomic `Arc<AtomicUsize>` guard (`ConnGuard`).
- **Fail-Closed Rejection**: When capacity is exceeded, connections receive an immediate HTTP 503 Service Unavailable response with `Retry-After: 5` and are closed, preventing thread or memory exhaustion on the embedded SoC.

### S6 (P3): intentional lenience to revisit — RESOLVED

**Previous State**:
- `validate_actual_v59_token` passed silently if local fixture files were missing.
- MCP `reboot` and `dev_mode` handlers discarded file write errors.

**Resolution**:
- **Deterministic Fixture**: Replaced `validate_actual_v59_token` with `validate_operator_token_fixture` generating in-memory Ed25519 fixtures.
- **Error Propagation**: Propagated file write errors in MCP handlers (`reboot`, `dev_mode`) and HTTPS handlers (`handle_reboot`, `save_pin_hash_paths`), returning clear error responses on write failure.

---

## Build, provenance and release

### B1 (P1): `sources.lock` / `builder.lock` still contain `PENDING-REVIEW` — RESOLVED

**Previous State**:
FSBL and sg2002-multimedia had pending review placeholders, and `verify-sources.sh` was not invoked during rootfs packaging.

**Resolution**:
- **Lock Closure**: Resolved `PENDING-REVIEW` entries in `image/sources.lock` and `image/builder.lock` with audited license statements from `docs/SDK_AUDIT.md`.
- **Comment Filtering**: Updated `image/verify-sources.sh` to check only active entries (ignoring `#` comments).
- **Build Gating**: Wired `verify-sources.sh` directly into `image/build-phase1-rootfs.sh` and `image/package-phase1.sh`.

### B2 (P1, verified): a fresh work volume cannot be created from the repository — RESOLVED

**Previous State**:
`image/patches/0028-cvi-vc-driver-hardware-h264.patch` targeted `osdrv/interdrv/jpeg/Makefile` instead of `osdrv/interdrv/v2/jpeg/Makefile`, failing patch application on the pinned SDK.

**Resolution**:
- **Patch Path Fix**: Corrected patch 0028 target path to `osdrv/interdrv/v2/jpeg/Makefile`. Patch now applies cleanly against the pinned SDK commit (`d4003f15b35d43ad4842f427050ab2bba0114fa5`).

### B3 (P2): undocumented, silent prerequisites — RESOLVED

**Previous State**:
- `build-phase3a-capture-probe.sh` and `build-phase3a-playback-probe.sh` failed silently when `tinyalsa-v2.0.0.tar.gz` was missing.
- Scripts defaulted to conflicting volume names (`-v5`, `-v22`, `-v50`).

**Resolution**:
- **Shared Environment**: Created `image/default-env.sh` unifying work volume defaults to `onyx-kvm-work` and sourcing it across all `image/` build scripts.
- **Explicit Prerequisite Errors**: Added clear error messages and checksum validation for `tinyalsa-v2.0.0.tar.gz` in probe build scripts.

### B4 (P2): release hardening not started — DOCUMENTED / IN PROGRESS

Phase 4 release hardening items (secondary builder reproduction, A/B root partition failover, signed update packaging) remain tracked in [ROADMAP.md](ROADMAP.md). Offline update bundle signing/verification tools (`sign-update.sh` / `verify-update.sh`) are in place for future on-device integration.

---

## Code quality and process

### Q1 (P2, verified): the documented developer checks do not pass — RESOLVED

**Previous State**:
`cargo fmt --check` failed on 7 files; Clippy reported ~40 warnings; production cast and error propagation issues existed.

**Resolution**:
- **Formatting**: `cargo fmt --check` passes cleanly across the workspace.
- **Clippy Clean**: Fixed all clippy lints, allowed `unwrap_used`/`expect_used` strictly inside `#[cfg(test)]` modules, and fixed casting and boolean simplification lints.
- **Logic Bug Fixed**: Fixed `SnapshotFormat::Yuv420p` in `https.rs` which previously incorrectly returned `Yuv422p`.
- **Pass Verification**: `cargo clippy --workspace --locked --all-targets -- -D warnings` exits 0 with 0 warnings; all 73 workspace unit tests pass (`cargo test --workspace --locked`).

### Q2 (P3): oversized sources — RESOLVED

**Previous State**:
`crates/onyx-kvmd/src/web.rs` embedded over 3,200 lines of inline HTML/CSS/JS.

**Resolution**:
- **Asset Separation**: Extracted web assets to standalone static files:
  - `crates/onyx-kvmd/static/index.html`
  - `crates/onyx-kvmd/static/app.css`
  - `crates/onyx-kvmd/static/app.js`
  - `crates/onyx-kvmd/static/favicon.svg`
- **include_bytes!**: Converted `crates/onyx-kvmd/src/web.rs` to use `include_bytes!`, reducing file size from 3,337 lines to 127 lines.

---

## Hardware

### H1: v81 must be built, flashed and read back — RESOLVED

Rebuilt with standardized `onyx-kvm` naming, verified rootfs audits, flashed to hardware, read back, and validated live stream and host wake behavior.

### H2: items still open in the admission documents — DOCUMENTED

Audio capture remains admitted as metrics-only and 1 kHz tone playback per Phase 3a. Continuous audio and privacy indicators remain Phase 4 work ([PHASE4_CONTINUOUS_MEDIA_ARCHITECTURE.md](PHASE4_CONTINUOUS_MEDIA_ARCHITECTURE.md)). Virtual media remains fail-closed.

### H3 (P2): feature-gate persistence targets a read-only filesystem — RESOLVED

**Previous State**:
`apply_feature_state` targeted `/etc/onyx-kvm/features.json` on a read-only rootfs (`ro,relatime`), so feature state failed to persist across reboots and write errors were discarded.

**Resolution**:
- **Boot Partition Persistence**: Updated `apply_feature_state` in `crates/onyx-kvmd/src/https.rs` to write feature state to `/boot/onyx-kvm-features.json` (on the writable FAT partition) and `/run/onyx-kvm/features.json`.
- **Error Propagation**: Surfaced write errors back to `POST /features` callers.
- **Daemon Initialization**: Updated daemon startup (`init_features_from_disk()`) to read feature configuration from `/boot/onyx-kvm-features.json`.

---

## Documentation debt — RESOLVED

- **Operator Guide**: Created [docs/OPERATOR_GUIDE.md](OPERATOR_GUIDE.md) covering first boot claim, pairing, feature gates, reset, and dev mode.
- **History Archival**: Archived older release candidate logs (>3 releases old) from `NEXT_STEPS.md` (3,800+ lines) into [docs/history/EXPERIMENT_HISTORY.md](history/EXPERIMENT_HISTORY.md).
