# A32 kernel configuration audit

No config was modified. The current config remains a compile-only reference.
Stock boot, recovery and OTA zImages decompress to 17,736,896-byte ARM Images;
none contains an `IKCFG_ST` embedded-config marker. No stock kernel .config was
found among local configuration/backup files. Stock column is UNKNOWN, not n.
The local camera `top_config.txt` is not a kernel configuration. No device query was used.

Current resolved .config SHA256: `3962ad7e641f9b884c8bf5a2640dac7f77ed6285bd7928f3ec8408c145820bd2`.
k37 defconfig source SHA256: `6a5e7b78b202124d352cd0719de53625134f2d326deacde94197ad18974999ce`.

k37 is the source defconfig, not a second resolved build. UNSPECIFIED means Kconfig
defaults/selects must be evaluated; it does not mean disabled. Current is resolved.

| Option | Stock 3.18 | Current 4.9 resolved | k37 source defconfig |
|---|---|---|---|
| CONFIG_ARM | UNKNOWN | `y` | `UNSPECIFIED` |
| CONFIG_CPU_V7 | UNKNOWN | `y` | `UNSPECIFIED` |
| CONFIG_CPU_V7M | UNKNOWN | `ABSENT` | `UNSPECIFIED` |
| CONFIG_ARCH_MEDIATEK | UNKNOWN | `n` | `UNSPECIFIED` |
| CONFIG_MACH_MT6735 | UNKNOWN | `n` | `UNSPECIFIED` |
| CONFIG_MACH_MT6735M | UNKNOWN | `y` | `y` |
| CONFIG_ARM_GIC | UNKNOWN | `ABSENT` | `UNSPECIFIED` |
| CONFIG_MTK_GIC | UNKNOWN | `ABSENT` | `UNSPECIFIED` |
| CONFIG_MTK_GIC_EXT | UNKNOWN | `n` | `UNSPECIFIED` |
| CONFIG_ARM_ARCH_TIMER | UNKNOWN | `y` | `UNSPECIFIED` |
| CONFIG_ARM_PSCI | UNKNOWN | `y` | `y` |
| CONFIG_SMP | UNKNOWN | `y` | `y` |
| CONFIG_OF | UNKNOWN | `y` | `UNSPECIFIED` |
| CONFIG_OF_FLATTREE | UNKNOWN | `y` | `UNSPECIFIED` |
| CONFIG_OF_EARLY_FLATTREE | UNKNOWN | `y` | `UNSPECIFIED` |
| CONFIG_OF_RESERVED_MEM | UNKNOWN | `y` | `UNSPECIFIED` |
| CONFIG_ARM_APPENDED_DTB | UNKNOWN | `n` | `UNSPECIFIED` |
| CONFIG_MTK_DTBO_FEATURE | UNKNOWN | `y` | `UNSPECIFIED` |
| CONFIG_MTK_SERIAL | UNKNOWN | `y` | `y` |
| CONFIG_MTK_SERIAL_CONSOLE | UNKNOWN | `y` | `UNSPECIFIED` |
| CONFIG_SERIAL_8250 | UNKNOWN | `n` | `UNSPECIFIED` |
| CONFIG_SERIAL_8250_CONSOLE | UNKNOWN | `ABSENT` | `UNSPECIFIED` |
| CONFIG_SERIAL_8250_MT6577 | UNKNOWN | `ABSENT` | `UNSPECIFIED` |
| CONFIG_SERIAL_EARLYCON | UNKNOWN | `ABSENT` | `UNSPECIFIED` |
| CONFIG_SERIAL_MTK_EARLY_CONSOLE | UNKNOWN | `n` | `UNSPECIFIED` |
| CONFIG_EARLY_PRINTK | UNKNOWN | `ABSENT` | `UNSPECIFIED` |
| CONFIG_DEBUG_LL | UNKNOWN | `n` | `UNSPECIFIED` |
| CONFIG_DEBUG_LL_INCLUDE | UNKNOWN | `"mach/debug-macro.S"` | `UNSPECIFIED` |
| CONFIG_MMC | UNKNOWN | `y` | `y` |
| CONFIG_MMC_BLOCK | UNKNOWN | `y` | `UNSPECIFIED` |
| CONFIG_MMC_MTK_PRO | UNKNOWN | `y` | `y` |
| CONFIG_MMC_MTK | UNKNOWN | `n` | `UNSPECIFIED` |
| CONFIG_MTK_PMIC | UNKNOWN | `y` | `UNSPECIFIED` |
| CONFIG_MTK_PMIC_NEW_ARCH | UNKNOWN | `n` | `UNSPECIFIED` |
| CONFIG_MTK_PMIC_WRAP_HAL | UNKNOWN | `y` | `y` |
| CONFIG_REGULATOR | UNKNOWN | `y` | `y` |
| CONFIG_REGULATOR_MTK | UNKNOWN | `n` | `UNSPECIFIED` |
| CONFIG_PINCTRL | UNKNOWN | `y` | `UNSPECIFIED` |
| CONFIG_PINCTRL_MTK | UNKNOWN | `y` | `UNSPECIFIED` |
| CONFIG_PINCTRL_MT6735 | UNKNOWN | `y` | `UNSPECIFIED` |
| CONFIG_GPIOLIB | UNKNOWN | `y` | `UNSPECIFIED` |
| CONFIG_OF_GPIO | UNKNOWN | `y` | `UNSPECIFIED` |
| CONFIG_COMMON_CLK | UNKNOWN | `y` | `UNSPECIFIED` |
| CONFIG_MTK_CLKMGR | UNKNOWN | `y` | `y` |
| CONFIG_WATCHDOG | UNKNOWN | `y` | `y` |
| CONFIG_WATCHDOG_CORE | UNKNOWN | `n` | `UNSPECIFIED` |
| CONFIG_MTK_WD_KICKER | UNKNOWN | `y` | `UNSPECIFIED` |
| CONFIG_MTK_FB | UNKNOWN | `y` | `y` |
| CONFIG_MTK_LCM | UNKNOWN | `y` | `y` |
| CONFIG_CUSTOM_KERNEL_LCM | UNKNOWN | `"hx8392a_dsi_cmd_qhd"` | `"hx8392a_dsi_cmd_qhd"` |
| CONFIG_LCM_HEIGHT | UNKNOWN | `"960"` | `"960"` |
| CONFIG_LCM_WIDTH | UNKNOWN | `"540"` | `"540"` |
| CONFIG_MTK_KERNEL_POWER_OFF_CHARGING | UNKNOWN | `y` | `y` |
| CONFIG_CMDLINE | UNKNOWN | `"console=tty0 console=ttyMT3,921600n1 root=/dev/ram vmalloc=496M slub_max_order=0 slub_debug=O "` | `"console=tty0 console=ttyMT3,921600n1 root=/dev/ram vmalloc=496M slub_max_order=0 slub_debug=O "` |
| CONFIG_CMDLINE_FROM_BOOTLOADER | UNKNOWN | `y` | `UNSPECIFIED` |
| CONFIG_CMDLINE_EXTEND | UNKNOWN | `n` | `UNSPECIFIED` |
| CONFIG_CMDLINE_FORCE | UNKNOWN | `n` | `UNSPECIFIED` |

## Interpretation and necessary changes

- This vendor tree builds MT6735M via `CONFIG_MACH_MT6735M=y`;
  `CONFIG_ARCH_MEDIATEK=n` and `CONFIG_MACH_MT6735=n` are not alone evidence
  of a missing SoC driver. Enabling the generic platform just to satisfy a name
  would be unjustified. Audit its platform Kconfig and selected vendor objects.
- UART is vendor `MTK_SERIAL` + `MTK_SERIAL_CONSOLE`, using ttyMT names.
  8250 is disabled. Do not enable a competing full UART driver merely for logs.
  The narrow future debug candidate is `SERIAL_MTK_EARLY_CONSOLE=y`, which
  selects SERIAL_EARLYCON; review its register access and inherited UART state
  before an isolated debug build. It was not enabled in the stable config.
- OF, flat-tree, reserved memory, MMC/block, vendor MMC_MTK_PRO, pinctrl, GPIO,
  PMIC/PWRAP, regulator and clock manager are already present. These options do
  not establish that A32 voltages or DT bindings are correct. In particular
  REGULATOR_MTK=n does not mean the separate vendor PMIC regulator framework
  is absent. The legacy power directory is selected by MTK_PMIC.
- WATCHDOG and MTK_WD_KICKER are enabled while generic WATCHDOG_CORE is not.
  Audit the vendor timer/kicker and LK watchdog handoff before deciding changes.
- ARM_APPENDED_DTB=n is not permission to toggle it blindly. MediaTek LK handoff
  and vendor DTBO strategy must be established; concatenation alone proves no
  run-time DT address. MTK_DTBO_FEATURE=y explains the separate k37 overlay.
- The reference command line selects ttyMT3; stock DT selects ttyMT0. Future
  A32 debug args must explicitly select the verified UART0 candidate without
  changing Android boot header cmdline. Current CMDLINE_FROM_BOOTLOADER means
  actual LK supplied args remain decisive and UNKNOWN from static backups.
- A first isolated A32 config must disable reference MTK_FB/LCM and unverified
  touch/camera/audio/sensor/charger/modem/connectivity consumers only after
  checking dependencies and vendor global probing. The known reference panel
  hx8392a / 540x960 is not A32 hardware. Do not claim DT status alone suppresses
  every vendor initialization path. See the existing board-gap audit for the
  remaining reference peripheral selections.
- No necessary, independently proven config correction was identified that can
  safely be applied alone before board binding/PMIC/clock dependency resolution.
  Consequently there are zero config changes, and no mass driver enablement.

Stock .config absence blocks an exact stock/current option-by-option diff.
A source defconfig comparison cannot substitute for a recovered stock config.
