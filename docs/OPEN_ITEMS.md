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

### S1 (P1): network `/setup/claim` lets any LAN client take an unconfigured device — ACCEPTED RISK

Decision: this is a low-cost appliance, and whoever boots and sets up an unconfigured device owns it. The claim keeps its guards (6 to 8 digit PIN, `Origin` matched against the configured origin list, console token without `kvm:admin`, 5 malformed attempts per daemon run), covered by `https::tests::tofu_setup_lifecycle_and_pin_authentication` with a multi-origin config. `Origin` is client-supplied and the first valid claim wins; neither is a defence against a hostile LAN client. Operators should claim new devices on an isolated network or use the UART ceremony ([OPERATOR_GUIDE.md](OPERATOR_GUIDE.md)).

### S2 (P1): the microSD card is a stated adversary, but it controls root — ACCEPTED RISK

Decision: physical access to the device owns the device, especially with an external SD card. The [threat model](THREAT_MODEL.md) is narrowed to read-only card access. Consequently `/boot/RESET`, `/boot/DEV_MODE` and `/boot/onyx-kvm-features.json` are honoured without further authentication: anyone who can write the card can reset the device or enable the microphone and speaker at boot. Revisit when verified boot (Phase 4) exists.

### S3 (P1, verified): development tooling ships in any profile when present — RESOLVED

- Rootfs: the dev Dropbear payload and `S35onyx-kvm-devmode` are gated behind `ONYX_KVM_DEV_TOOLS=1` and removed for the `product` profile.
- Code: `kvm.dev.exec` and `kvm.system.dev_mode` are behind the `dev-tools` cargo feature of `onyx-kvm-mcp` (forwarded by `onyx-kvmd`). `image/build-rust-daemon.sh` enables it only when `ONYX_KVM_DEV_TOOLS=1`; the default build omits the code.
- Audit: `image/audit-rootfs.sh` (profile is its 16th argument) fails a product rootfs that contains the dev directory, the init script, or an `onyx-kvmd` built with the feature.
- Physical access to the device owns the device (S2), so `/boot/DEV_MODE` is not treated as an authentication boundary on development images.

### S4 (P2): operator-grade tokens are the only token the setup and pairing flows mint — RESOLVED

`mint_console_token` (in `crates/onyx-kvmd/src/https.rs`, beside `mint_operator_token`) issues `kvm:read kvm:video kvm:audio-capture kvm:audio-output kvm:control` without `kvm:admin`. Both `/setup/claim` and the PIN branch of `/pair` use it. Admin tokens come only from the UART pairing ceremony (a token staged by `pair-device.sh`), and the daemon's fallback mint when that ceremony staged no token.

### S5 (P2): unbounded connection handling — RESOLVED (with known limits)

`serve` caps concurrent connections at 16 (`MAX_CONCURRENT_CONNECTIONS`, `ConnGuard`). Connections over the cap are closed without a response: the TLS handshake has not happened, so an HTTP 503 cannot be sent. Known limit: a handful of idle sockets can occupy slots until the read timeout expires, so the cap bounds resources but does not prevent a LAN client from degrading availability.

### S6 (P3): intentional lenience to revisit — RESOLVED

**Previous State**:
- `validate_actual_v59_token` passed silently if local fixture files were missing.
- MCP `reboot` and `dev_mode` handlers discarded file write errors.

**Resolution**:
- **Deterministic Fixture**: Replaced `validate_actual_v59_token` with `validate_operator_token_fixture` generating in-memory Ed25519 fixtures.
- **Error Propagation**: Propagated file write errors in MCP handlers (`reboot`, `dev_mode`) and HTTPS handlers (`handle_reboot`, `save_pin_hash_paths`), returning clear error responses on write failure.

---

## Build, provenance and release

### B1 (P1): `sources.lock` / `builder.lock` still contain `PENDING-REVIEW` — REVIEWED; release blocked by missing license grants

The review is recorded in [SDK_AUDIT.md](SDK_AUDIT.md) ("Shipped-source license review"). Outcome: the FSBL third-party libraries have identified licenses, but the Cvitek/Bitmain-authored FSBL files, the linked middleware, and the sophgo-osdrv tree carry no license grant. Those entries are marked `NO-LICENSE-GRANT`; the apt snapshot field now states that the builder is fixed by image digest and package manifest. `image/verify-sources.sh` (wired into `build.sh`, `build-phase1-rootfs.sh` and `package-phase1.sh`) fails on either marker. Development builds set `ONYX_KVM_ALLOW_UNRESOLVED_SOURCES=1`, which prints a warning; such images are not releasable. Closing this needs a vendor license grant or replacement of those components (Phase 4). The kernel-module review adds the VPU firmware `coda980.bin` to the list; six SDK modules are GPL by declaration only (see SDK_AUDIT.md). Four components in total lack a grant: vendor FSBL files, linked middleware, `sophgo-osdrv` and `coda980.bin`.

### B2 (P1, verified): a fresh work volume cannot be created from the repository — RESOLVED

Patch 0028 targets the separate sophgo-osdrv tree, not the pinned SDK. `apply-phase1-config.sh` skips it, as it does 0001, and `image/apply-osdrv-patch.sh` applies it to a checkout of `sophgo/osdrv` at the revision pinned in `sources.lock` (verified, idempotent, offline; round trip tested). An earlier edit changed its `jpeg/Makefile` path to `v2/jpeg`, which is the SDK layout, and was reverted. Patches 0002 to 0027 apply to the SDK.

### B3 (P2): undocumented, silent prerequisites — RESOLVED

**Previous State**:
- `build-phase3a-capture-probe.sh` and `build-phase3a-playback-probe.sh` failed silently when `tinyalsa-v2.0.0.tar.gz` was missing.
- Scripts defaulted to conflicting volume names (`-v5`, `-v22`, `-v50`).

**Resolution**:
- **Shared Environment**: Created `image/default-env.sh` unifying work volume defaults to `onyx-kvm-work` and sourcing it across all `image/` build scripts.
- **Explicit Prerequisite Errors**: Added clear error messages and checksum validation for `tinyalsa-v2.0.0.tar.gz` in probe build scripts.

### B5 (P2): the shipped video modules are not reproducible from the repository — MOSTLY RESOLVED

`image/build-phase3-video-modules.sh OUTPUT_DIR` now builds all nine modules and copies `coda980.bin`, offline, from the pinned SDK (patches 0002 to 0027) and the pinned `sophgo-osdrv` tree (patch 0028), and writes `SHA256SUMS` and `BUILD-INFO.txt`. Kernel build metadata is pinned so repeated builds match. Compared with `out/phase3-video-modules-v51` (2026-10-19):

- Byte-identical: `soph_sys`, `soph_snsr_i2c`, `soph_vi`, `mars_vcodec`, `soph_jpeg`.
- Identical except the embedded kernel build stamp: `soph_base`, `soph_vpss`, `cvi_vc_driver`.
- **Different: `soph_mipi_rx`.** The shipped module has fewer diagnostic parameters than the repository patch stack produces (it lacks `cif_diagnostic_full_readback`, `cif_diagnostic_stock_rx0n_mclk1` and others, but has the patch 0027 wording). Its exact source state is not recorded. The rebuilt module adds diagnostic parameters that default off, but the final verification build must confirm video capture on hardware with it.

Recipe notes: `mars_vcodec` is built from the osdrv `vcodec` sources plus the SDK's `vcodec/hal/mars`; `soph_jpeg` and `cvi_vc_driver` come from osdrv with `CVIARCH_L=mars`; the SDK's prebuilt `soph_jpeg.ko` is not used.

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
- **Pass Verification**: `cargo clippy --workspace --locked --all-targets -- -D warnings` exits 0 with 0 warnings; all workspace unit tests pass (`cargo test --workspace --locked`).

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

### H1: final image built, flashed and read back — PARTIAL (v82 flashed; boot and live behaviour pending)

v82 was built, flashed to `/dev/disk4` and read back with embedded-artifact verification (recorded in [NEXT_STEPS.md](NEXT_STEPS.md)). Still open: boot, re-pairing and live video capture on hardware. Original plan: once all changes land, build and install a new version for final verification, then record disk identity, embedded FIP/FIT/rootfs read-back and live behaviour.

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

## Environment

- Stale container resources: the pre-rename `cleankvm-*` Podman volumes and the `cleankvm-builder` tag were removed on 2026-10-19 after the reusable inputs (`onyx-kvm-sdk`, `onyx-kvm-host-tools`, `onyx-kvm-buildroot-dl-v50`) were verified to match file counts and sizes. `image/migrate-legacy-names.sh` remains for other machines.

## Documentation debt — RESOLVED

- **Operator Guide**: Created [docs/OPERATOR_GUIDE.md](OPERATOR_GUIDE.md) covering first boot claim, pairing, feature gates, reset, and dev mode.
- **History Archival**: Archived older release candidate logs (>3 releases old) from `NEXT_STEPS.md` (3,800+ lines) into [docs/history/EXPERIMENT_HISTORY.md](history/EXPERIMENT_HISTORY.md).
