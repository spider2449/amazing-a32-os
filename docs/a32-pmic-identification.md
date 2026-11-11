# A32 PMIC identification

Investigation date: 2026-11-11. Scope and provenance:
[evidence index](a32-boot-critical-evidence-index.md). `S` below means the pinned
local `phone-tools/kernel-4.9` source, not an A32 stock 3.18 source.

| Candidate | Evidence | Contradiction / limitation | Confidence |
|---|---|---|---|
| MT6328 | Stock LK contains MT6328 PWRAP read/write-test messages; stock kernel string inspection and 4.9 mt6735 PMIC/PWRAP implementation support this family; stock regulator names match its legacy table | No observed runtime CID; LK also prints MT6325; reference source is not A32 source | HIGH |
| MT6325 | LK image offset 232688: `[pmic_init] MT6325 CHIP Code = 0x%x` | Could be a copied diagnostic label; surrounding PWRAP messages explicitly say MT6328; no returned chip ID available | LOW |
| MT6350 | Present elsewhere in family source; MMC source has a `MT6350 no OC` comment | Comment is not detection; mt6735 PMIC headers and selected PWRAP slave use MT6328 | LOW |
| MT6351 | Other-platform source support | No A32-specific selection/CID evidence | UNKNOWN |
| MT6353 | Selectable newer PMIC architecture elsewhere in tree | Current resolved config disables `CONFIG_MTK_PMIC_CHIP_MT6353`; legacy mt6735 path is different | LOW |
| MT6397 | Generic kernel support | No A32-specific binding, selection or CID evidence | UNKNOWN |

Hardware identity remains **not CONFIRMED**. HIGH means the strongest candidate
from independent local stock artifacts plus code, not proof of electrical identity.

## Selection and generic binding

`S/drivers/misc/mediatek/Makefile:59,68,116` selects PWRAP and legacy power;
`power/Makefile:20` selects `CONFIG_MTK_PLATFORM` (resolved `"mt6735"`).
`power/mt6735/Makefile` includes `pmic.o`; `pmic.c:5082-5206` registers platform
devices/drivers, including `pmic_mt_device` and `mt_pmic_device`. This is compile
time platform selection, not a runtime choice between MT6328 and MT6353.

`pmic.c:1243` has `mediatek,mt6328` in an OF table, but
`pmic_regulator_ldo_init():1253` explicitly finds **`mediatek,mt_pmic`** and its
`ldo_regulators` children. `pmic.c:1384` finds the same generic node for the other
regulator path. Stock `/soc/mt_pmic_regulator` supplies that generic compatible,
`ldo_regulators` and `regulators_supply`. Thus a generic-compatible software path
exists in 4.9; generic wording does **not** imply PMIC-independent register access.
It writes MT6328-specific registers, including initialization at `pmic.c:3847+`.
Its suitability for A32 is PARTIAL until identification and initialization agree.

## Runtime identification code

| Item | Code evidence | Interpretation |
|---|---|---|
| PMIC register base | `S/drivers/misc/mediatek/include/mt-plat/mt6735/include/mach/upmu_hw.h`, MT6328_PMIC_REG_BASE | PMIC register space, not ARM physical MMIO |
| HWCID | `upmu_hw.h:53,1271-1273` | address base + `0x0200`, mask `0xffff`, shift 0 |
| SWCID | `upmu_hw.h:54,1274-1276` | address base + `0x0202`, mask `0xffff`, shift 0 |
| Expected revision IDs | `S/drivers/misc/mediatek/power/mt6735/pmic.h:23-25` | E1 `0x2810`, E2 `0x2820`, E3 `0x2830` |
| Read / revision branch | `pmic.c:1536-1544,3847+,4860-4864` | `pmic_get_register_value(PMIC_SWCID)`; E1 alters VMC voltage tables; CID printed during probe |
| Selection path | platform Makefile -> legacy driver -> SWCID-dependent initialization | Revision handling within assumed MT6328 family; no proven multi-PMIC autodetection |

Actual runtime SWCID is UNKNOWN. No PMIC reads were issued to a device.

## PWRAP compatibility

Stock `/bus/PWRAP@0x10001000`: compatible `mediatek,PWRAP`, base `0x10001000`,
size `0x1000`, GIC SPI 163, flags 4. `S/drivers/misc/mediatek/pmic_wrap/Makefile`
selects mt6735 by platform; `mt6735/pwrap_hal.h:40` defines `SLV_6328`.
`pwrap_hal.c:1617-1638` finds that exact generic compatible, maps DT resource 0
and maps the IRQ. `:1545-1580` also requires `mediatek,INFRACFG_AO` and
`mediatek,CKSYS`. Stock has these provider nodes.

`is_pwrap_init_done():1595+` checks `PMIC_WRAP_INIT_DONE2`; if already initialized
it returns without full initialization, otherwise calls `pwrap_init()`.
The latter uses MT6328 DEW read test, DIO, cipher and CRC registers
(`:703-1107`). This establishes a matching binding and handoff-capable code path,
not proof of LK register state or transport compatibility with an unidentified chip.
Do not substitute upstream `mt6397` PMIC/PWRAP bindings.

To close this blocker using offline evidence: recover actual CID from an existing
boot log/dump or identify the selected stock PMIC implementation and register map
in an authentic source snapshot. No new device action is part of this investigation.
