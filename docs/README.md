# Documentation index

Start with the first group. The evidence records are long and are meant to be
searched, not read front to back.

## Orientation

| Document | Read it for |
| --- | --- |
| [../README.md](../README.md) | Goals, repository layout, developer checks |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Processes, trust boundaries, HTTPS routes, MCP tools, image layout |
| [THREAT_MODEL.md](THREAT_MODEL.md) | Assets, adversaries, required controls |
| [NETWORK_POLICY.md](NETWORK_POLICY.md) | What may listen or connect |
| [ROADMAP.md](ROADMAP.md) | Phases and their current status |
| [OPEN_ITEMS.md](OPEN_ITEMS.md) | Unfinished work, security gaps, recommendations |
| [OPERATOR_GUIDE.md](OPERATOR_GUIDE.md) | First boot, pairing, feature gates, factory reset, dev mode |
| [NAMING.md](NAMING.md) | The `onyx-kvm` naming standard and the CleanKVM migration |
| [../image/README.md](../image/README.md) | Build workflow and image tooling |
| [../AGENTS.md](../AGENTS.md) | Contributor and hardware-workflow rules |

## Security boundaries

- [HID_SECURITY_BOUNDARY.md](HID_SECURITY_BOUNDARY.md): keyboard and pointer broker.
- [MEDIA_SECURITY_BOUNDARY.md](MEDIA_SECURITY_BOUNDARY.md): video and audio isolation.
- [PHASE4_CONTINUOUS_MEDIA_ARCHITECTURE.md](PHASE4_CONTINUOUS_MEDIA_ARCHITECTURE.md): continuous media transport.

## Provenance and sources

- [PROVENANCE.md](PROVENANCE.md): provenance requirements.
- [SDK_AUDIT.md](SDK_AUDIT.md): audit of the pinned Sipeed SDK.
- [PHASE3_SOURCE_INVENTORY.md](PHASE3_SOURCE_INVENTORY.md): video/audio source candidates.
- [LT6911_UPSTREAM_REQUIREMENTS.md](LT6911_UPSTREAM_REQUIREMENTS.md) and
  [LT6911_FIRMWARE_RECOVERY.md](LT6911_FIRMWARE_RECOVERY.md): HDMI bridge firmware.

## Acceptance and hardware evidence (dated records)

These keep the names that were true when they were written; see
[NAMING.md](NAMING.md).

- [HARDWARE_EVIDENCE.md](HARDWARE_EVIDENCE.md): stock-device collector procedure
  (results in [../evidence/](../evidence/)).
- [PHASE1_ACCEPTANCE.md](PHASE1_ACCEPTANCE.md), [PHASE2_ACCEPTANCE.md](PHASE2_ACCEPTANCE.md),
  [PHASE3_ACCEPTANCE.md](PHASE3_ACCEPTANCE.md): per-phase acceptance gates.
- [PHASE3_VIDEO_ADMISSION.md](PHASE3_VIDEO_ADMISSION.md): video bring-up decisions
  and measurements, including the closed MIPI RX pinmux investigation.
- [PHASE3_MIPI_SIGNAL_MEASUREMENT.md](PHASE3_MIPI_SIGNAL_MEASUREMENT.md): D-PHY measurement plan.
- [PHASE3A_AUDIO_ADMISSION.md](PHASE3A_AUDIO_ADMISSION.md): audio enumeration, capture, playback.
- [PRODUCT_COMPLETION_PLAN.md](PRODUCT_COMPLETION_PLAN.md): product-completion work items.
- [NEXT_STEPS.md](NEXT_STEPS.md): newest-first release-candidate log. Read the top
  entry before continuing hardware bring-up, as `AGENTS.md` requires.

## Where does a new fact go?

| Kind of fact | Put it in |
| --- | --- |
| A hardware hypothesis was closed or opened | `NEXT_STEPS.md`, and `PHASE3_VIDEO_ADMISSION.md` for material findings |
| A release candidate was built, flashed and read back | `NEXT_STEPS.md` (hashes, disk identity, read-back result) |
| A design or boundary changed | `ARCHITECTURE.md` or the relevant `*_SECURITY_BOUNDARY.md` |
| A gap or follow-up was found | `OPEN_ITEMS.md` |
| A build or tooling step changed | `image/README.md` |
