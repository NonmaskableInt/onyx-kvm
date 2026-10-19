# Source and binary provenance policy

> **Status:** three vendor components have no license grant (FSBL
> Cvitek/Bitmain-authored files, the linked middleware, and `sophgo-osdrv`), and
> the remaining vendor kernel modules are unreviewed. Images are development
> builds only until they are granted or replaced. See `SDK_AUDIT.md` and
> `OPEN_ITEMS.md` B1.

Every input to a release image must appear in `image/sources.lock` with an
immutable revision and cryptographic digest. Floating branches, release URLs
without hashes, and binaries copied from a factory image are prohibited.

Each unavoidable binary-only artifact requires:

- origin and exact hash;
- purpose and hardware interface;
- process privileges and sandbox policy;
- replacement plan;
- explicit inclusion in the SBOM and release notes.

The build must fail when a required digest is absent or mismatched. The final
release includes partition hashes, an SPDX SBOM, compiler versions, source
revisions, configuration files, and build provenance.
