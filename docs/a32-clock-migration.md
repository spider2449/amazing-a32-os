# A32 boot-critical clock migration

[Source provenance](a32-boot-critical-evidence-index.md). All graphs below
separate code dependencies from observed clock register state; **LK clock state
is UNKNOWN**. No clock register has been read on hardware.

## Dependency graph

```text
26 MHz reference (4.9 platform assumption; actual handoff unmeasured)
 ├─ CPU/AXI -> ARMPLL / bus muxes -> CPU execution and GIC bus access
 ├─ timer path -> 13 MHz arch timer + APXGPT (stock declared rate)
 ├─ MT_MUX_UART (CLK_CFG_2; two-input mux)
 │    -> MT_CG_PERI_UART0 -> UART0; APDMA dependency in normal probe
 ├─ MSDCPLL (4.9 nominal 800 MHz)
 │    -> MT_MUX_MSDC30_0 (CLK_CFG_3; index 2 = 400 MHz in 4.9)
 │         + sibling MT_MUX_MSDC50_0 -> MT_CG_PERI_MSDC30_0 -> MSDC0
 └─ MT_MUX_PMICSPI (CLK_CFG_4) + infrastructure PWRAP gates
      -> PWRAP -> PMIC DEW transport / PMIC internal regulator clocks

RGU watchdog -> counter/reset domain; exact retained clock source UNKNOWN
GIC -> interconnect / CPU interface; no stock DT clock phandle
```

The CPU/AXI and watchdog source arrows above denote required functional domains,
not a proven complete PLL topology. In particular no source proof connects every
CPU/bus mux to the selected reference rate. PMIC internal oscillators and watchdog
counter parent are not inferred from the SoC crystal.

4.9 selection: `S/drivers/misc/mediatek/base/power/mt6735/Makefile:36-41`
chooses `mt_clkmgr2.o` for `CONFIG_MACH_MT6735M` with legacy `CONFIG_MTK_CLKMGR`.
`mt_clkmgr2.c:467-505` defines ARMPLL/MAINPLL/MSDCPLL/UNIVPLL;
`:2020-2090` defines MSDC/UART/PMICSPI muxes; `:3637-3639` sets mux register bases;
`:3744,3749` links MSDC0 and UART0 gate objects to their muxes.
These are legacy imperative clock APIs, not a complete CCF phandle graph.
`msdc_cust.c:369-393` selects its mux; `msdc_cust.h:95-109` gives rates.
`uart/uart.c:2374-2408` requires named UART/APDMA clocks unless skip flag is set.
`pmic_wrap/mt6735/pwrap_hal.c:1545-1580` additionally maps CKSYS/INFRACFG_AO.

## Stock / reference comparison

| Consumer | Stock 3.18 DT | mt6735m 4.9 | k37 4.9 | Consequence |
|---|---|---|---|---|
| CPU | 1.1 GHz frequency metadata, no clock list | 1 GHz metadata | 1.1 GHz metadata | Metadata does not establish PLL setup or safe DVFS |
| GIC | MT6735 GIC, no clocks / clock-names | Same family | Inherited | Bus/CPU-interface clock must survive handoff; no provider phandle proof |
| Arch timer / APXGPT | 13 MHz, no consumer clock list | 13 MHz | Inherited | Keep stock rate; timer gate/source handoff unproven |
| UART0 | No clocks, clock-names or skip flag | `skip_pinmux_clk=1`, no clock list | Inherited | Reference assumes bootloader setup; stock does not prove that assumption for 4.9 |
| MSDC0 | `clk_src` u8 2, no clocks list | Vendor `clk_src` 2 | Reference pin/power policy | 4.9 index known; 3.18 source table unavailable |
| PWRAP | Legacy MMIO node, no clock list | Same legacy binding | Inherited | INIT_DONE2 check may reuse LK setup; transport state unproven |
| PMIC | Generic node, no consumer clock list | Legacy PMIC register driver | Board regulator overrides | Cannot derive internal PMIC clocks or rails from DT alone |
| Watchdog | TOPRGU, no clocks list | Same block | Inherited | Runtime mode and counter clock remain handoff risks |

Property comparisons are reproducible from stock FDT and the existing
[semantic comparison](a32-dt-semantic-comparison.md), including symbolic k37
overlay limitations. **There are no parent-clock phandles to resolve for these
stock consumers**; a graph of invented `clocks` entries would be misleading.

| Clock class requested | Assessment |
|---|---|
| 1. LK enabled, Linux may take over | PWRAP has an explicit INIT_DONE2 reuse path; UART skip path exists. Actual LK state UNKNOWN, so neither is certified safe |
| 2. 4.9 must enable/reconfigure | Normal UART probe requires clock acquisition; MSDC mux/gate and PWRAP init paths act on clock state. Exact minimum enable set PARTIAL |
| 3. Loss can hang early boot | CPU/AXI, interrupt delivery and system timer domains; loss of PWRAP can stall PMIC operations. This is a failure dependency, not proof LK gates them |
| 4. Peripheral effect | UART gates remove diagnostics; MSDC gates prevent storage. These can still block this boot plan even if CPU continues |

Minimum clocks readiness: **PARTIAL**. Obtain authentic stock clock initialization
and LK handoff paths or existing register evidence before defining the minimum set.
