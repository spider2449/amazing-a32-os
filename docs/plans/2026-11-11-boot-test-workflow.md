# Linux 4.9 boot-test workflow

1. Verify actual build/config, image hashes, Linux banner and DTB composition.
2. Inspect local stock boot/recovery and OTA images; identify board and signing gaps.
3. Add a fail-closed, kernel-only boot repacker and artifact verifier without Rust or device access.
4. Test preservation, malformed/unsupported images, and current real artifacts.
5. Document reproducible commands and boot prerequisites; review diff and commit only source/docs.

Completed: artifact and stock image inspection, scripts, offline tests, documentation.
Current stock images have nonzero signing/vendor trailers; repacking stops instead
of silently keeping invalid signatures or discarding unidentified data. The k37
reference DT is also not an A32 boot-ready board description.
