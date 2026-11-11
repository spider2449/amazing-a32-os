# Amazing A32 stock DT and Linux 4.9 migration

Scope: offline analysis only. No device commands, boot image creation, signature
removal, Rust changes, or replacement of the successful kernel artifact.

1. Verify local branch, source, backups, tools and stable artifact provenance.
2. Implement strict boot-layout and FDT parsers, extraction and tail reports;
   test valid, corrupt, truncated, multiple-FDT and unknown-tail inputs.
3. Extract stock DT and inventory hardware with raw properties and references.
4. Compare stock, mt6735m and k37 semantically against Linux 4.9 drivers.
5. Audit console, config and boot/recovery/OTA trailers with evidence.
6. Only create/build an A32 board DTS when boot-critical bindings and values are
   supported by evidence. Otherwise document exact blockers without inventing DTS.
7. Run tests, review diff and tracked paths, commit source/docs/tests only, and
   report READY or NOT READY with concrete blockers.

Completed: extraction, 472-node inventory, symbolic three-way comparison, console
and config audits, and boot/recovery/OTA tail analysis. The first DER signature
layer verifies offline in all three; final 256-byte layer and device trust remain
UNKNOWN. Board DTS/build deferred under the explicit insufficient-evidence rule.
Validation and resulting NOT READY decision: docs/a32-analysis-validation.md.
