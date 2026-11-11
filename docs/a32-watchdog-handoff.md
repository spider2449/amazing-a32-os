# A32 watchdog and bootloader handoff

[Source provenance](a32-boot-critical-evidence-index.md).

| Finding | Evidence | Status |
|---|---|---|
| RGU base `0x10212000`, size `0x1000` | Stock `/soc/toprgu@10212000`, compatible `mediatek,mt6735-rgu`, SPI 128 flags 2 | SAFE: static resource identification only |
| 4.9 binding matches | `S/drivers/watchdog/mediatek/wdt/mt6735/mtk_wdt.c:53-54` | SAFE: source match only |
| LK handles watchdog state | LK binary offset 232928 `WDT NONRST=0x%x`; offsets 287236/287332 watchdog strings | UNKNOWN: no executed setup/timeout recovered |
| Preloader setup | No preloader source found in available checkout/cache | UNKNOWN |
| Stock kernel handling | Stock Image has `mtk_wdt_probe`; no 3.18 driver source | UNKNOWN exact sequence |
| Kernel probe delay | IRQ mapping/registration precedes timeout reset/kick | RISK: bootloader counter could expire before probe |
| Current kicker | Resolved CONFIG_MTK_WD_KICKER=y | RISK: watchdog remains enabled; kicker scheduling matters |

## 4.9 sequence from source

`mt_wdt.h:18-24` uses mapped `toprgu_base`:
MODE +0, LENGTH +4, RESTART +8, STATUS +0xc.
MODE write key `0x22000000`; enable bit 0; IRQ bit 3; dual-mode bit 6;
RESTART key `0x1971` (`:34-54`). These are register facts, not commands executed.

`mtk_wdt_probe():623-724`:

1. Maps DT RGU and IRQ, then requests IRQ/FIQ; failures return before takeover.
2. Optionally initializes SPM watchdog under CONFIG_KICK_SPM_WDT.
3. Sets `g_last_time_time_out_value=30`, programs timeout, kicks WD_TYPE_NORMAL.
4. With CONFIG_MTK_WD_KICKER configures dual-mode, IRQ and external reset,
   watchdog enabled; otherwise explicitly disables watchdog.
5. Updates NONRST and boot-stage interval markers.

`mtk_wdt_set_time_out_value():97-113` uses value *64 <<5 plus LENGTH key;
that source's unit is seconds. This proves a **30-second kernel setting**, not
LK's timeout. No immediate entry-time disable is proven. Driver probe is later
than the interval during which early boot can hang.

`S/drivers/watchdog/mtk_wdt.c` is a separate upstream-style driver; it must not be
used to describe the selected vendor mt6735 implementation. Vendor watchdog
Makefiles and CONFIG_MTK_WD_KICKER, not a generic name match, determine behavior.

Handoff readiness: **NO**. LK enable bit, timeout, last kick, selected watchdog
counter and pre-probe coverage remain UNKNOWN. A matching RGU base is insufficient
to certify reset-loop avoidance. No watchdog mode or kernel config was changed.
