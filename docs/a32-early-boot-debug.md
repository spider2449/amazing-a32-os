# Amazing A32 early boot console evidence

Offline findings only; no Android command line was changed and no device was
contacted. UART electrical pins, test points, voltage level and LK console state
remain UNKNOWN. No physical UART connection is implied by a DT console string.

| Item | Stock | Current Linux 4.9 reference |
|---|---|---|
| Boot header cmdline | `bootopt=64S3,32N2,32N2` | Not repacked |
| DT `/chosen/bootargs` serial console | `console=ttyMT0,921600n1` | `console=ttyMT3,921600n1` |
| Text console | `console=tty0` | `console=tty0` |
| Kernel CONFIG_CMDLINE | UNKNOWN; no embedded stock config | `console=tty0 console=ttyMT3,921600n1 root=/dev/ram vmalloc=496M slub_max_order=0 slub_debug=O ` |
| Most likely controller | UART0 at `0x11002000` | UART3 at `0x11005000` |
| UART compatible | `mediatek,mt6735-uart` | Same vendor compatible |
| DT baud / framing | 921600 / 8n1 | 921600 / 8n1 |
| Actual UART register baud / LK override | UNKNOWN | UNKNOWN |
| LK initialization | Possible, not confirmed from backups | `skip_pinmux_clk=1` explicitly relies on preinitialized clocks/pins |

Stock UART0 reg tuples are `0x11002000+0x1000`, `0x11000380+0x1000`,
`0x11000400+0x80`; interrupt tuples are GIC SPI 91, 103, 104, all flags 8.
UART0..3 bases are 0x11002000, 0x11003000, 0x11004000, 0x11005000.
The header does not specify console; actual LK-supplied final bootargs and
runtime DT fixups are UNKNOWN. Current `CMDLINE_FROM_BOOTLOADER=y` is another
reason static CONFIG_CMDLINE is not proof of actual console selection.

## Candidate arguments, not applied

The narrow regular-console candidate, supported by stock DT and vendor ttyMT
driver naming, is:

```text
console=ttyMT0,921600n1 loglevel=8 ignore_loglevel
```

This is not a proposed complete Android cmdline. Root/initrd parameters depend
on the eventual boot handoff; stock DT's initrd size is not the measured compressed
ramdisk size. Nothing here is appended permanently to the boot header.

There is a dedicated MediaTek early console in the pinned 4.9 source:

- `drivers/tty/serial/mtk_earlycon.c:135` declares `uartmtk`.
- Its reads use status offset 0x14 and writes use offset 0, with MMIO32 support.
- `init_port()` register/baud programming is compiled out with `#if 0`.
  Its fallback baud function returns 921600, which is not a measurement.
- Transmit polling has no timeout. An unclocked or wrong UART can hang early.
- `drivers/tty/serial/Kconfig:1638` selects SERIAL_EARLYCON through
  SERIAL_MTK_EARLY_CONSOLE. Both are absent/disabled in the current build.

Conditional future candidate after confirming UART0 clock, pins, register layout
and a separate debug config enabling `CONFIG_SERIAL_MTK_EARLY_CONSOLE=y`:

```text
earlycon=uartmtk,mmio32,0x11002000,921600
```

This candidate preserves inherited hardware baud because this implementation
does not program a divisor. Adding it to the current kernel would not provide
this early console: the driver is not built. No config was changed in this phase.

The upstream-style alternative `earlycon=mtk8250,mmio32,0x11002000,921600`
exists in `drivers/tty/serial/8250/8250_mtk.c:875`, but is gated by
SERIAL_8250_CONSOLE. That path matches `mediatek,mt6577-uart`, while this board
uses the separate vendor UART driver and SERIAL_8250=n. It is not the preferred
candidate and must not be enabled as a competing full driver without review.

`earlyprintk` string: **UNKNOWN for an operational A32 debug setup**.
The generic ARM parser at `arch/arm/kernel/early_printk.c:39..46` accepts
`earlyprintk` and calls the debug-LL `printch()` path; it does not parse a UART
address/baud string. Current DEBUG_LL=n and EARLY_PRINTK is absent, while
DEBUG_LL_INCLUDE names `mach/debug-macro.S`. A generic
`earlyprintk=serial,...` suggestion would be unsupported here. A future bare
`earlyprintk` requires verifying and building the matching debug-LL implementation
and its actual UART base; no working value is claimed now.

Before using any candidate, resolve LK initialization and final DT/cmdline
handoff offline where possible. Device testing remains prohibited and NOT READY.
