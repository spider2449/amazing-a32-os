# A32 UART0 boot path

[Source provenance](a32-boot-critical-evidence-index.md). Stock is a static image
DT, not the post-LK tree.

| Field | Stock evidence | 4.9 code / migration |
|---|---|---|
| Address | `/soc/apuart0@11002000`, first reg `0x11002000,0x1000` | Vendor UART driver, not assumed upstream 8250 |
| Additional resources | `0x11000380,0x1000`; `0x11000400,0x80` | DMA-related resources must be audited, not dropped by resemblance to 8250 |
| IRQ | SPI 91 flags 8; additional SPIs 103 and 104 flags 8 | Vendor probe maps DT IRQs; do not use global IRQ numbers as SPI IDs |
| Compatible | `mediatek,mt6735-uart` | `S/drivers/misc/mediatek/uart/uart.c:2786` matches exactly |
| Driver / tty | Stock chosen `console=ttyMT0,921600n1`; stock Image has same compatible | `uart.c:2321` registers `ttyMT`; Makefile selects `uart.o` with CONFIG_MTK_SERIAL |
| Config | Stock exact config unavailable | Current resolved CONFIG_MTK_SERIAL=y, CONFIG_MTK_SERIAL_CONSOLE=y |
| Index | `cell-index=<0>` | `uart.c:2354` reads cell-index into pdev id; aliases do not replace this |
| IO / shift | Stock DT has no reg-shift or reg-io-width | `uart.c:2708` UPIO_MEM; platform_uart.h:237-244 explicit byte offsets 0,4,...0x14, i.e. 4-byte register stride. No parsed generic reg-shift property established |
| Clock | No clock/clock-names/frequency on UART0 | platform_uart.h:130-147 uses 26 MHz UART_SYSCLK for non-FPGA; actual LK rate UNKNOWN |
| Pinctrl | No UART0 pinctrl reference | 4.9 normal probe attempts pinctrl; A32 pin mapping/electrical setup UNKNOWN |
| Skip policy | Stock has no skip_pinmux_clk | 4.9 reference sets 1; this bypasses pin/clock acquisition and assumes retained initialization |
| Alias / stdout | No serial0 alias or stdout-path in stock | Can express routing intent after UART readiness; does not initialize hardware |

4.9 parser proof: `uart.c:2351-2361` reads cell-index and skip flag;
`:2374-2410` acquires `clk_uart_name[id]` plus `uart-apdma` for UART0 when
not skipping; `:2418-2428` requests pinctrl when not legacy/FPGA and not skipping.
The clock named `UART_SYSCLK` is a driver assumption used for divisors, not a
DT-measured frequency. Do not replace it with a 65 MHz comment from another path.
`uart/mt6735/platform_uart.c` contains baud generation, gate control and pinctrl
operations; `platform_uart.h:110-113` maps UART0 to MT_CG_PERI_UART0.

The stock decompressed Image contains `mediatek,mt6735-uart` and a stock build
path ending `kernel-3.18/drivers/misc/mediatek/uart/mt6735/platform_uart.c`.
This proves vendor lineage, but **does not provide the missing 3.18 parser code**.
Exact stock-driver equivalence is PARTIAL, while 4.9 compatible matching is proven.

Conditional minimal routing, after pin/clock/handoff proof:

```dts
aliases { serial0 = &a32_uart0; };
chosen { stdout-path = "serial0:921600n1"; };
```

`a32_uart0` is illustrative, not an existing label or a DTS change. DT aliases and
stdout syntax can identify this port without adding bootargs, but vendor console
selection still needs its actual console path / LK command line checked. Neither
property proves diagnostic output. Do not add these to a board file while gate is
closed. UART0 readiness: **PARTIAL** (pinmux and clock/handoff unresolved).
