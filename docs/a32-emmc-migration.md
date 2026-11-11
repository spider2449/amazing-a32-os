# A32 eMMC migration and regulator graph

Source conventions: [evidence index](a32-boot-critical-evidence-index.md).
**No edge or voltage is selected for a new DTS.** Stock 3.18 driver source was
not found locally; its parser semantics cannot be proven from property names.

## Stock controller and power graph

Stock `/mtk-msdc.0/msdc0@11230000`: compatible `mediatek,mt6735m-mmc`,
base `0x11230000`, size `0x10000`; extra reg tuple `0x10000e84, 2`;
GIC SPI 79 / flags 8; 8-bit bus; non-removable and bootable.
These are raw DT facts, not an assertion that all extra register resources are
consumed identically. `max-frequency=200000000`, `clk_src=[02]`,
`host_function=[00]`; HS, DDR 1.8 V, HS200 1.8 V and HS400 1.8 V flags exist.

```text
MSDC0
 ├─ vmmc-supply / vendor power reference: ABSENT -> VEMC consumer edge UNKNOWN
 ├─ vqmmc-supply / vendor IO reference: ABSENT -> IO consumer edge UNKNOWN
 ├─ pinctl -> phandle 2 -> /bus/pinctrl@0x10211000/mmc0@default
 └─ register_setting -> phandle 3 -> .../mmc0@register_default

/soc/mt_pmic_regulator/regulators_supply (separate consumer)
 ├─ vemc_3v3-supply -> phandle 0x16 -> ../ldo_regulators/ldo_vemc_3v3
 └─ vio18-supply -> phandle 0x1e -> ../ldo_regulators/ldo_vio18
       -> generic PMIC driver -> MT6328-specific PMIC register API
       -> PWRAP WACS -> physical PMIC identity HIGH candidate, not CONFIRMED
```

All stock `*-supply` properties and vendor `pinctl`/`register_setting` references
are expanded in ignored `out/a32-regulator-graph.json`; no indirect supply chain
from MSDC0 was found. The PMIC supply node must not be silently joined to MSDC0.
Stock LK offset 199024 contains `[MSDC] config VEMC to 3V in lk`: useful HIGH
evidence of an intended LK VEMC path, but a string alone does not establish the
executed register write or IO rail connection.

| Stock rail | Constraint / flags | What remains unknown |
|---|---|---|
| `vemc_3v3` | 1,800,000..3,300,000 uV; boot-on; no always-on | Actual voltage and MSDC consumer link; name does not force 3.3 V |
| `vio18` | exactly 1,800,000 uV; boot-on; no always-on | Physical MSDC0 IO connection and kernel control/handoff |

4.9 `S/drivers/mmc/host/mediatek/Makefile:49-53` selects `ComboA/mt6735`;
`msdc_cust.c:948-949` calls `regulator_get(...,"vmmc")` and `"vqmmc"`.
`msdc_emmc_power():216-230` requests `VOL_3000` on vmmc and sets 1.8 V pad
selection; this is **reference behavior**, not permission to adopt it on A32.
`S/arch/arm/boot/dts/cust_mt6735_msdc.dtsi:34` adds VEMC vmmc in the reference;
stock does not. SD controller supplies at lines 67-68 are not eMMC supplies.

## Binding translation

`S/drivers/mmc/host/mediatek/ComboA/mt6735/msdc_cust.c:801-846` follows vendor
`pinctl` and reads each child `drive-strength` using `of_property_read_u8`.
`:852-867` follows `register_setting` and reads the three edges as **u8**.
`:922` reads `clk_src` as u8. Encoding `<1>` would be four bytes starting with
zero; preserve `/bits/ 8` only after semantics are proven.

| 3.18 property | Meaning established from stock | 4.9 equivalent / meaning | Action |
|---|---|---|---|
| `register_setting/cmd_edge=[01]` | 3.18 enum/parser UNKNOWN | Same property; 0 rising, 1 falling command response sample | BLOCKED; do not choose 0 or 1 |
| `rdata_edge=[01]` | 3.18 sample semantics UNKNOWN | Same property; 0 rising, 1 falling read data sample | BLOCKED |
| `wdata_edge=[01]` | 3.18 sample semantics UNKNOWN | Same property; 0 rising, 1 falling write CRC/status sample path | BLOCKED |
| `clk_src=[02]` | Index 2; stock source frequency UNKNOWN | Index 2 = 400 MHz in mt6735 table | Do not assume same table in 3.18 |
| `pinctl` and child `drive-strength=[02]` | Five cmd/dat/clk/rst/ds fields, raw code 2 | Vendor u8 register code, not generic mA pinctrl binding | Preserve evidence only; no conversion to 2 mA |
| `dat0rddly`..`dat7rddly`, `datwrddly`, `cmdrrddly`, `cmdrddly` = 0 | Present in stock | `msdc_get_register_settings` reads only edges | Do not claim delay values migrated; audit tuning path |
| `ett-hs200-*`, `ett-hs400-*` | Stock tuning register tuples | No equivalent established by this parser | BLOCKED, no direct tuple copy |
| `cap-mmc-highspeed` | Present | MMC core flag | Capability equivalence alone insufficient for electrical proof |
| `mmc-ddr-1_8v`, `mmc-hs200-1_8v`, `mmc-hs400-1_8v` | Present, not measured operation | MMC core flags / `mmc_of_parse` | No enabled high-speed mode selected |
| `sampling` | No property with this name in stock MSDC0 | Sampling implemented by edges and tuning | Do not synthesize |

4.9 enum proof: `S/include/dt-bindings/mmc/mt6735-msdc.h:50-52` defines
`MSDC_SMPL_RISING=0`, `FALLING=1`, `SEPARATE=2`.
`ComboA/sd.c:496-580` writes command `MSDC_IOCON_RSPL`, read `R_D_SMPL_SEL` /
`MSDC_PB0_RD_DAT_SEL` and write CRC sample fields, with distinct DDR/HS400 paths.
For clock modes 2/3 the read selector is forced to 0; `wdata_edge` selects CRC
status sampling rather than write-data launch. Thus `cmd_edge` is not simply
the clock output edge. Reference `cust_mt6735_msdc.dtsi:112-114` uses rising (0),
while stock stores 1 for all three. This discrepancy remains unresolved.

4.9 clock proof: `msdc_cust.h:95-109` defines an 800 MHz MSDCPLL and
MSDC0 sources 26/800/400/200/182/136/156/416/48/91/624 MHz;
`msdc_cust.c:334-346` indexes them. `:379` selects default source index 2 via
`clkmux_sel`. These numbers describe 4.9 implementation only.

Reset, pinmux, pull resistors and drive writes are implemented in
`ComboA/mt6735/msdc_cust.c:585-795`, `msdc_io.h` and `msdc_cust.h`; they are not fully specified by the
five stock drive codes. Board reset sequencing, actual pulls, LK-retained state,
IO rail and 3.18 parser equivalence remain UNKNOWN.

Recommended migration value: **UNRESOLVED**. A conservative configuration with
proven stock 3.18 semantics has not been established; merely deleting HS400 or
lowering frequency does not prove safe power/timing. DTS gate stays closed.
