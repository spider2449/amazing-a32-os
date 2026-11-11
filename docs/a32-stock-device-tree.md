# Amazing A32 stock DT and migration requirements

Offline evidence captured 2026-11-11 on `kernel-4.9`, starting at
`ed3343c9aaa9039993ed032cccda87b44abe7dbb`. No device operations, Rust changes,
new boot image, or changes to stable kernel/config/source were made.

## Extraction and identity

Input: `phone-tools/backup/boot-mmcblk0p7.img`, 16,777,216 bytes,
SHA256 `79ef60fc9b0d8330175de1fc8ac60cc839542c24cc4b2993cd3b9772633372d1`.
Offsets below are zero based, ends exclusive.

| Field | Value |
|---|---|
| Kernel start / size | 2,048 / 6,755,597 bytes |
| zImage header start / end | 0 / 6,695,352 |
| zImage end in boot image | 6,697,400 |
| FDT magic | `d0 0d fe ed` |
| FDT kernel-relative / absolute offset | 6,695,352 / 6,697,400 |
| FDT bytes / header totalsize | 60,245 / 60,245 |
| FDT absolute end | 6,757,645 |
| Remaining kernel bytes after DTB | 0 |
| Remaining image bytes after DTB | 10,019,571 (includes padding, ramdisk and tail) |
| Valid FDTs / invalid magic candidates in kernel | 1 / 0 |
| Container | Direct appended FDT; no intervening MTK header or DTBO table |
| Version / boot CPU | FDT v17 / 0 |
| Model | `MT6735M` |
| Root compatible | `mediatek,MT6735` |
| DTB SHA256 | `2bfe064a24aa130fd126b05d54ef3834b5fce674a22211254a115147fc34c3ba` |

These strings establish the vendor DT family, not the exact silicon SKU. Existing
repository board notes identify MT6737M. Stock CPU nodes identify four Cortex-A53
cores running an ARM32 kernel; they do not independently distinguish MT6735M from
MT6737M. Exact chip revision and PMIC part number remain UNKNOWN from this DT.
Stock boot, recovery and OTA all contain the identical DTB hash.

Reproduce in WSL at repository root (use the already built dtc, no download):

```sh
python3 scripts/extract-stock-dtb.py phone-tools/backup/boot-mmcblk0p7.img \
  --dtc target/kernel-4.9/build/scripts/dtc/dtc --legacy-unit-addresses
file out/stock-a32.dtb
sha256sum out/stock-a32.dtb
python3 scripts/a32-dt-inventory.py out/stock-a32.dtb docs/a32-stock-dt-inventory.md
python3 scripts/compare-a32-dt.py out/stock-a32.dtb \
  target/kernel-4.9/build/arch/arm/boot/dts/mt6735m.dtb \
  target/kernel-4.9/build/arch/arm/boot/dts/k37mv1_bsp_k49.dtb \
  docs/a32-dt-semantic-comparison.md
```

`file` recognizes FDT v17, structure 53,380 bytes, strings 6,809 bytes.
`fdtdump` is not installed; the existing built `dtc` supplies the independent
decode. Plain `dtc -I dtb -O dts` rejects legacy vendor unit names lacking reg.
The explicit flag disables only `unit_address_vs_reg`; no `-f` is used.
This does not validate Linux 4.9 bindings. The Python parser independently checks
FDT bounds, structure, reservation termination, unique nodes/properties/phandles.
If dtc is missing the extraction command exits 2 and downloads nothing.

## Hardware inventory

[Complete 472-node inventory](a32-stock-dt-inventory.md) records every node's raw
properties, compatible, reg, interrupt cells, clock properties when present,
status, pinctrl, supplies and resolved declared references. Large arrays are
represented by byte length and SHA256; their full values remain in local DTS.
Absence of status defaults to enabled; it is not evidence that hardware is fitted
or probed. Generic sensor and charger alternatives are especially not chip IDs.
No GPIO, IRQ, supply or status has been fabricated.

| Subsystem | Stock evidence and migration implication |
|---|---|
| `/compatible`, `/model` | `mediatek,MT6735`, `MT6735M`; retain family dispatch while distinguishing future board identity |
| CPUs | `/cpus/cpu@000..003`, Cortex-A53, regs 0..3, 1.1 GHz; spin-table release `0x40000200`; PSCI SMC node also exists; verify ARM32 SMP path rather than assume DT enable-method alone controls it |
| Memory | `/memory@00000000`: 64-bit address `0x40000000`, size `0x3f000000` (1008 MiB); end `0x7f000000`. This is DT-visible memory, not a runtime LK-patched RAM capture |
| Chosen | `console=tty0 console=ttyMT0,921600n1 root=/dev/ram initrd=0x44000000,0x1000000 loglevel=8 androidboot.hardware=mt6735` (original includes tabs). Boot header only says `bootopt=64S3,32N2,32N2` |
| GIC | `/interrupt-controller@10220000`, `mediatek,mt6735-gic`; distributor `0x10221000`, CPU interface `0x10222000`, third region `0x10200620`, each 0x1000 |
| Timer | `/soc/timer`, `arm,armv8-timer`, 13 MHz; PPI cells 13/14/11/10 flags 8. APXGPT `0x10004000`, SPI 152; CPUXGPT `0x10200000`, SPI 64..71 |
| UART | `/soc/apuart0..3` at `0x11002000..0x11005000`, `mediatek,mt6735-uart`; UART0 SPI 91 plus DMA SPIs 103/104; console likely UART0 |
| GPIO / pinctrl | `/soc/gpio` vendor usage mapping; `/bus/pinctrl@0x10211000` contains actual pin states, MMC drive/timing and peripheral GPIO assignments. Provider references are resolved in inventory |
| PMIC | `/bus/PWRAP@0x10001000`, `mediatek,PWRAP`, SPI 163; PMIC EINT 206 via `/bus/eintc@10005000`. Generic `mediatek,mt_pmic`; exact PMIC chip UNKNOWN |
| Regulators | `/soc/mt_pmic_regulator` buck/ldo nodes, boot/always-on flags and supply aggregator. `vemc_3v3` 1.8..3.3 V bounds, boot-on; bounds are not measured operating voltages |
| MMC / eMMC / SD | MSDC0 `0x11230000`, 0x10000, SPI 79 flags 8, 8-bit, nonremovable, host_function byte 0, clk_src byte 2, max 200 MHz, HS200/HS400 advertised. MSDC1 `0x11240000`, SPI 80, 4-bit SD; card-detect GPIO 11. Do not enable SD in first bring-up |
| NAND | `/bus/NFI@0x1100D000`, `mediatek,NFI`, SPI 96; generic SoC node present, physical NAND not established; do not enable it |
| I2C | `0x11007000`, `0x11008000`, `0x11009000`, `0x1100f000`, `mediatek,mt6735m-i2c`; camera, touch, generic sensors/ext-buck children. Binding and actual fitted children require audit |
| SPI | `/soc/spi@1100a000`, `mediatek,mt6735m-spi`, SPI 118; duplicate vendor register-map node also exists |
| USB | `/bus/usb20@11200000`, `mediatek,mt6735-usb20`, regions `0x11200000`/`0x11210000`, SPI 72; explicit pinctrl and vendor VBUS/IDDIG GPIO data; do not copy k37 VBUS policy |
| Watchdog | `/soc/toprgu@10212000`, vendor reset/watchdog block; preserve only after kicker/reset path audit |
| RTC | No dedicated RTC-compatible node found; PMIC-backed RTC wiring/driver remains UNKNOWN |
| Framebuffer / display | `/bus/MTKFB@5e200000` actually maps `0x7f000000` size 16 MiB; `/lcm` generic compatible does not identify panel. Preserve carveout even when display is disabled |
| GPU | `/bus/MALI@0x13040000`, Mali-T720 family compatible strings; noncritical, disable |
| Touchscreen | `/bus/i2c@11008000/cap_touch@5d`, generic `mediatek,cap_touch`; address is not proof of controller model; vendor `/bus/touch@` holds pinctrl/supplies |
| Keypad / buttons | `/soc/keypad@10003000`, mt6735/mt6735m-keypad, SPI 164 flags 2; power key 116, hardware power key 8; full key map and pins in inventory |
| LEDs | `/led@0..6`, including PMIC/PWM LCD backlight; alternative flash driver `/leddrvsgm3780` is not proof of fitted part |
| Vibrator | `/vibrator@0`; PMIC vibrator LDO and vendor voltage/timing properties; disable initially |
| Battery | `/bus/bat_meter`, `/bus/bat_notify`, `/bus/bat_comm`: calibration/charging policy, not generic upstream battery bindings |
| Charger | Both `/aw3216` (`mediatek,chg-aw3216`) and `/eta6195` (`mediatek,chg-eta6195`) with enable/current GPIO states. Actual fitted charger UNKNOWN; neither should be guessed |
| Thermal | `/bus/THERM_CTRL@0x1100B000`; vendor thermal policy and PMIC/clock dependencies need driver audit |
| Clocks | Legacy INFRACFG_AO `0x10000000`, PERICFG `0x10002000`, INFRACFG `0x10201000`, FHCTL `0x10209f00`, APMIXED-related register maps and `/rf_clock_buffer`; no standard `clocks` consumer properties found in stock DT |
| Power domains | `/bus/SLEEP@0x10006000` vendor SPM; no standard `power-domains` consumer properties found. Do not manufacture upstream provider handles |
| Reserved memory | ATF `0x43000000+0x30000`; ram_console `0x43f00000+0x10000`; pstore `0x43f10000+0xe0000`; minirdump `0x43ff0000+0x10000`; DT memreserve table empty |
| Modem | CCCI MD1 dynamic 0x3810000 bytes, alignment 0x2000000, allocation range `0x40000000+0xc0000000`; MD CLDMA and modem peripheral maps. Dynamic reservation has no fixed address in file |
| Connectivity | CONSYS reservation 1 MiB aligned 2 MiB; `/soc/consys@18070000`, WCN_AHB and pinctrl; reservation placement UNKNOWN until bootloader/runtime evidence |
| Wi-Fi | `/bus/wifi@180F0000`, `mediatek,wifi`, SPI 226; calibration/firmware not established by DT |
| Bluetooth | CONSYS plus `/bus/btcvsd@10000000`, `mediatek,audio_bt_cvsd`, SPI 228; not a standalone board UART declaration |
| GPS | `/bus/gps`, `mediatek,gps`, GPS LNA pin states; actual electrical routing requires audit |
| Audio | `/bus/audio@11220000` and many vendor PCM/DAI nodes; generic topology is not A32 codec proof; disable initially |
| Camera | `/bus/kd_camera_hw1@15008000` and I2C camera_main/sub nodes; generic addresses are not fitted sensor identities; power sequence UNKNOWN for 4.9 |
| Sensors | `/cust_accel@0`, `/cust_alsps@0`, generic I2C gsensor/alsps/gyro nodes; repository identifies MXC400X/AP3426 separately; DT alone does not prove these chips |

Interrupt numbers above are DT GIC SPI specifiers, not Linux virtual IRQ numbers.
UART0's SPI 91 corresponds to GIC hardware INTID 123; do not confuse it with a GPIO
EINT specifier such as PMIC 206. Parent buses use different address/size cell counts.

## Semantic comparison and actions

[Generated property comparison](a32-dt-semantic-comparison.md) resolves 45 k37
overlay fragment targets against base symbols and normalizes references to paths.
It compares parsed properties, bus layouts and physical controller identities,
not decompiled text or raw numeric phandle equality. k37 is an overlay, not a
standalone SoC DT; values below marked inherited describe a symbolic base+overlay
view, not the currently attached kernel tree. Current kernel attaches only base.

| Subsystem | Stock A32 | mt6735m 4.9 | k37 4.9 | Action |
|---|---|---|---|---|
| SoC dispatch | MT6735M / mediatek,MT6735 | Same | Inherited | KEEP family; exact SKU UNKNOWN |
| CPU / SMP | A53 x4, spin-table 0x40000200, PSCI node | Same broad structure | Explicit 1.1 GHz CPUs | UNKNOWN ARM32 LK/SMP handoff |
| RAM | 0x40000000+0x3f000000 | 0x40000000+0x40000000 | Inherited | DANGEROUS extra 16 MiB overlaps framebuffer carveout; PORT stock extent |
| Reserved-memory | ATF, MD1, CONSYS, console, pstore, minirdump | Adds dynamic 64 KiB SPM reserve | Inherited | PORT established ranges; UNKNOWN SPM requirement/placement |
| Modem reserve | 0x3810000, 32 MiB aligned, dynamic | Same | Inherited | KEEP reservation while modem stays disabled; UNKNOWN runtime placement |
| GIC | Same registers, mt6735-gic | Same | Inherited | KEEP using 4.9 driver |
| Timer | APXGPT mt6735-apxgpt, arch timer 13 MHz | APXGPT compatible changed to mediatek,APXGPT | Inherited | PORT compatible via 4.9 binding; KEEP proven timer frequency/IRQs |
| Console | ttyMT0 / 0x11002000 | ttyMT3 / 0x11005000 | Inherited | REWRITE candidate console; DANGEROUS silent UART mismatch |
| UART policy | No skip_pinmux_clk | skip_pinmux_clk=1 on all UARTs | Inherited | UNKNOWN LK pinmux/clock inheritance; PORT after audit |
| eMMC | mt6735m-mmc, nested 32-bit bus, extra reg 0x10000e84+2, no index | mediatek,msdc, root 64-bit reg, index byte 0 | Inherited; no eMMC target override | REWRITE 4.9 interface; DANGEROUS copying stock reg cells or missing index |
| MMC timing | mmc0 default drives byte 2, cmd_edge byte 1, vendor delay state | msdc0 default drives also 2, but cmd_edge byte 0; adds HS200 states | Inherited | DANGEROUS sampling-edge difference; UNKNOWN tuning translation; PORT only verified timing |
| eMMC supplies | No node vmmc-supply; separate PMIC aggregator | vmmc -> vemc_3v3; driver also gets vqmmc | Inherited | DANGEROUS unverified rail ownership, operating voltage and error paths |
| SD | CD GPIO 11, mt6735m-mmc | CD GPIO 5, new compatible/index/supplies | SD insertion EINT overlay | DANGEROUS wrong detect pin; disable first |
| PMIC | Generic mt_pmic/PWRAP, EINT206 | Same broad interface | Inherited plus board LDO defaults | UNKNOWN exact chip; DANGEROUS assuming matching PMIC from generic compatible |
| Regulators | Named bucks/LDOs, boot flags, board default states | Similar voltage ranges | Explicit peripheral LDO default-on/status changes | PORT only boot-critical verified rails; DANGEROUS enabling copied peripheral defaults |
| GPIO/pinctrl | A32 states, provider 0x10211000 | Different GPIO usage and msdc state names | Board touch/camera/USB/etc mappings | REWRITE board mapping; DANGEROUS global default pin changes |
| Display | 16 MiB @0x7f000000; generic lcm | Generic display enabled | Same FB region; config hx8392a 540x960, otm9608 binding include | DANGEROUS panel mismatch; disable display/LCM and preserve region |
| USB | USB2 base/PHY, SPI72, VBUS/IDDIG states | Similar controller, board policy differs | usbiddig pinctrl/EINT overrides | UNKNOWN VBUS wiring; disable until PORT |
| Charger | AW3216 and ETA6195 alternatives | No equivalent confirmed A32 charger selection | No A32 fitted-part proof | UNKNOWN fitted charger; DANGEROUS wrong enable/current pins |
| Battery | Vendor meter and charge tables | Reference vendor policy | Reference config power-off charging | PORT validated policy later; disable unsafe reference consumers |
| Touch | Generic cap_touch@5d | Generic touch | Explicit touch pins/EINT and vtouch->vgp1 | DANGEROUS assume GT1151 from same I2C address; disable |
| Keypad | Stock /soc/keypad, SPI164, power key 116 | Different /bus/keypad interface | Board keypad map override | PORT map to actual 4.9 driver, not path-only copy |
| Clocks/SPM | Legacy register maps, no clocks arrays | Legacy MTK_CLKMGR and SPM additions | RF clock buffer board settings | UNKNOWN safe subset and idle firmware; preserve before pruning |
| Interrupts | GIC UART0=91, MSDC0=79; GPIO EINTs board-specific | Core IRQs agree | Peripheral EINT remaps | KEEP core assignments; REWRITE peripheral IRQ/polarity only with evidence |
| Watchdog | TOPRGU 0x10212000 | Same block; kicker built | Inherited | UNKNOWN LK timer ownership and kicker dependencies; reset risk |

Likely failure modes are explicit risks, not observed device failures: wrong RAM
extent or GIC/clock handoff can hang early; ttyMT3 can hide UART0 logs; wrong MMC
binding/index/timing prevents eMMC enumeration; wrong supplies/default GPIOs can
cause regulator failure or PMIC faults; reference panel produces a black screen;
watchdog ownership/kicker mismatch can reset before logs. No such test was run.

## Board DTS decision: deferred

No `mt6735m-amazing-a32.dts`, Makefile entry, DTB build or `out/zImage-dtb-a32`
was created. This follows the requested exception for insufficient boot-critical
evidence. Copying base then changing model/RAM/console would still leave enabled
reference pin defaults, PMIC consumers, display, charger and clock/idle dependencies.

The initial board must use 4.9 bindings and only verified CPUs, RAM, GIC, timers,
clocks, UART, PMIC/regulators, eMMC and watchdog. Noncritical display, GPU, audio,
camera, sensors, touch, SD and connectivity must remain disabled. Merely adding
`status = "disabled"` is insufficient until each vendor driver has been checked
for global `of_find_compatible_node` and non-DT platform initialization paths.

Concrete blockers before a reliable board source:

1. Confirm actual PMIC chip, eMMC rail control/voltage, and required 4.9 supply
   consumers; generic DT compatibles and LDO voltage bounds do not prove these.
2. Translate and verify MSDC pin/timing subtrees including HS200 state, register
   width (`/bits/ 8`), additional top register lookup and supply error paths.
   Evidence: `drivers/mmc/host/mediatek/ComboA/mt6735/msdc_cust.c:890` requires
   index, lines 943..960 read pins, supplies and msdc0_top.
3. Determine bootloader DT fixups, actual reserved placement, ARM32 CPU release
   path, UART clock/pinmux ownership and watchdog/clock/idle minimum closure.
4. Audit vendor global-probe consumers and produce an isolated minimal config
   without reference display/charging/peripheral side effects.
5. Preserve signature failure gate; unresolved tail layer blocks packaging even
   after the DT is ready. See [format analysis](a32-boot-image-format.md).

Status: **NOT READY** for non-persistent boot. Compilation of a DT alone would
not resolve these blockers. Stable zImage-dtb remains 7,025,796 bytes, SHA256
`903407f54fac63359a73dc20df082e10d44bd0176e7bcb2dd0e72ceaa2a069fc`.
