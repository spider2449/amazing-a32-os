# A32 boot-critical evidence investigation

Baseline: kernel-4.9, 6f838bf48371057ffeb942ebf18f14f315356d19.

1. Inventory all available source trees and binary evidence; preserve stable artifacts.
2. Trace PMIC selection, runtime identification, PWRAP and actual MSDC power paths.
3. Compare MMC bindings, clocks, UART, watchdog, CPU and memory handoff.
4. Audit available LK/preloader code and explicitly record unavailable evidence.
5. Parse DER lengths and compare all three final blocks offline; verify only with existing keys.
6. Add fail-closed evidence helpers and meaningful synthetic tests; generate local JSON.
7. Publish source-linked migration documents and readiness matrix. Create DTS and build
   DTB only if every required gate passes; never infer electrical parameters.
8. Run existing and new tests, check stable hashes and prepare rejection, review and
   commit only scripts, tests and documentation (DTS only if justified).

No downloads, device operations, signing, boot image packaging, Rust additions,
stable artifact changes or proprietary binary commits are authorized.

Outcome: offline investigation and evidence documents completed; 65 tests pass.
PMIC MT6328 is HIGH, not CONFIRMED. Mandatory DTS evidence gate remains closed;
no DTS/DTB or new kernel concatenation was produced. Actual prepare and readiness
checker both exit 2. Both stable artifact hashes are unchanged. Remaining unknowns
are enumerated in docs/a32-minimal-dts-readiness.md.
