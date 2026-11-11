# A32 minimum DTS readiness

2026-11-11, baseline `6f838bf48371057ffeb942ebf18f14f315356d19`.
Readiness is scoped to creating an evidence-backed boot-critical DT, not to
hardware testing or packaging. No board DTS has been created.

| Component | Required for boot | Evidence level | Ready | Remaining unknown |
|---|---|---|---|---|
| CPU | Yes | Stock DT + 4.9 PSCI setup code | PARTIAL | Stock selected SMP path, secure monitor/LK handoff |
| memory | Yes | CONFIRMED static 1008 MiB | PARTIAL | LK final reg fixup and top 16 MiB owner |
| GIC | Yes | Matching stock / 4.9 MT6735 binding and resources | YES | No new board-specific value needed; clock state tracked separately |
| timer | Yes | Stock 13 MHz / matching 4.9 timer family | PARTIAL | Source/gate and entry state |
| minimum clocks | Yes | 4.9 mux/gate implementation | PARTIAL | Minimum retained/enabled set and LK state |
| UART0 | Yes for diagnostics | Stock resource + exact 4.9 compatible | PARTIAL | Pinmux, real input clock and initialization path |
| pinctrl | Yes for UART/MMC | Stock MMC vendor phandles | PARTIAL | Stock parser and UART mapping |
| PWRAP | Yes | Exact compatible + INIT_DONE2 path | PARTIAL | Actual transport/chip compatibility and LK state |
| PMIC | Yes | MT6328 HIGH candidate | PARTIAL | Actual CID or proven generic-compatible initialization |
| regulators | Yes | Stock names/constraints/supply references | PARTIAL | Consumers and physical rail connectivity |
| MSDC0/eMMC power path | Yes | LK VEMC string + stock separate supply graph | NO | VEMC consumer and IO rail, control sequence |
| MSDC0/eMMC timing | Yes for storage | Raw stock u8 edges and 4.9 parser | NO | 3.18 semantics and proven conservative mode |
| watchdog handoff | Yes | Stock RGU resource + 4.9 30-second/kicker code | NO | LK enable/timeout/kick before kernel probe |
| chosen | Yes for diagnostics/routing | Static stock chosen | PARTIAL | Final LK cmdline/stdout behavior |
| reserved-memory | Yes | Fixed and dynamic stock declarations | PARTIAL | Final LK/secure/modem/framebuffer allocation |

GIC proof: `S/drivers/misc/mediatek/irq/mt6735/irq.c` matches
`mediatek,mt6735-gic`; stock distributor 0x10221000 / CPU interface 0x10222000
and third vendor reg 0x10200620 are retained evidence. This YES does not clear
the separate minimum-clock or secure-world blockers.

The machine-readable reviewed matrix is
[a32-minimal-dts-readiness.json](a32-minimal-dts-readiness.json).
`python3 scripts/check-a32-dts-readiness.py docs/a32-minimal-dts-readiness.json`
prints **DTS GATE: NOT READY**, exit **2**. Missing/duplicate/malformed rows,
missing evidence and unproven conservative timing all fail closed.
The checker evaluates documented assessments; it does not manufacture hardware proof.

Mandatory YES items: memory, GIC, timer, UART0, minimum clocks, PWRAP,
PMIC identity or proven generic path, eMMC power path and watchdog handoff.
Timing may be PARTIAL only with an explicitly proven stock conservative mode;
there is none yet. Therefore **gate FAILS**.

`arch/arm/boot/dts/mt6735m-amazing-a32.dts`: **not created**.
DTB build, semantic roundtrip and `out/zImage-dtb-a32`: **not performed / not
created**, because those actions depend on a passing gate. No reference regulator,
RAM, edge or handoff value was substituted. Existing stable artifacts are preserved.

DEVICE-TREE READY: **NO**

BOOT-IMAGE PACKAGING READY: **NO**

BOOTLOADER VERIFICATION UNDERSTOOD: **NO**

First non-persistent boot readiness: **NOT READY**
