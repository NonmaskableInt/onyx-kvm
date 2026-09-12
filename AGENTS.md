# CleanKVM contributor instructions

These instructions apply to the entire repository.

## Objective

Build a source-auditable, independently packaged NanoKVM Lite image with a
Rust control plane and authenticated MCP interface. Video, HID, networking,
microphone capture, and audio output are separate admission boundaries and
must remain fail-closed until their individual hardware and security gates
pass.

## Security boundary

- Never execute a binary or init script taken from a stock/vendor image.
- Inspect stock artifacts offline and treat their contents as untrusted data.
- Do not restore `/dev/mem`, broad physical-memory mapping, forced module
  loading, unsigned boot overrides, vendor update agents, or default remote
  credentials.
- Do not add LT6911 register writes without corresponding-source evidence and
  a documented, bounded hardware hypothesis.
- Keep audio and microphone support disabled until Phase 3a is explicitly
  admitted. Do not couple audio enablement to video bring-up.
- Keep generated images, SDK checkouts, build outputs, credentials, device
  identities, certificates, and private keys outside Git. Follow `.gitignore`.

## Source and build workflow

- `crates/` contains the Rust control plane, MCP, HID broker, media protocol,
  evidence collector, and memory test.
- `image/` contains pinned-source verification, configuration, patches,
  offline build scripts, image assembly, and hardware diagnostics.
- `docs/` contains threat, architecture, provenance, acceptance, and hardware
  admission records.
- `dl/`, `out/`, and `target/` are generated/local-only. Deliberate changes to
  the ignored SDK tree must also be represented as a tracked patch under
  `image/patches/`.
- Build network access is allowed only in the explicit source-fetch stage.
  Compilation and packaging must use verified cached inputs with networking
  disabled.
- Use `cargo test --workspace --locked` for the Rust workspace. Run the
  relevant config, rootfs, image, and read-back verifiers for image changes.
- Preserve unrelated user changes. Use focused commits and push only after
  tests or proportional verification pass.

## Hardware workflow

- Resolve removable-media identity before every flash. Never assume a disk
  number from a prior insertion.
- Verify embedded FIP, FIT, and rootfs artifacts from card read-back. A whole
  image hash may differ after macOS changes FAT bookkeeping; this does not
  replace embedded-artifact verification.
- Keep UART diagnostics bounded, non-autoload, and cleanup-verifying. Record
  initial/final taint, loaded modules, nodes, and output-file disposition.
- Record material hardware findings in `docs/PHASE3_VIDEO_ADMISSION.md` and the
  project wiki.

## Current work

Read `docs/NEXT_STEPS.md` before continuing hardware bring-up. Update it when
evidence closes a hypothesis or changes the next bounded experiment.
