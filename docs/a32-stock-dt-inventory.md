# DT property inventory

Input: `out/stock-a32.dtb`; 60245 bytes; SHA256 `2bfe064a24aa130fd126b05d54ef3834b5fce674a22211254a115147fc34c3ba`.

Generated from validated FDT, corresponding to dtc decompilation. Raw cell arrays
retain parent bus encodings; do not interpret every reg tuple as 32-bit address/size.
Absent status means enabled by DT convention, subject to parent status and driver binding.
Missing properties are UNKNOWN, never synthesized. Large properties are hashed.
Reference resolution uses provider cell counts; vendor custom bindings still require driver audit.

Memreserve entries: `[]`.

## `/`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| model | `["MT6735M"]` |  |
| compatible | `["mediatek,MT6735"]` |  |
| interrupt-parent | `<0x1>` | /interrupt-controller@10220000 |
| #address-cells | `<0x2>` |  |
| #size-cells | `<0x2>` |  |

## `/chosen`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| bootargs | `["console=tty0 console=ttyMT0,921600n1 root=/dev/ram \t\t\tinitrd=0x44000000,0x1000000 loglevel=8 androidboot.hardware=mt6735"]` |  |

## `/mtk-msdc.0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["simple-bus"]` |  |
| #address-cells | `<0x1>` |  |
| #size-cells | `<0x1>` |  |
| ranges | `<0x0 0x0 0x0 0xffffffff>` |  |

## `/mtk-msdc.0/msdc0@11230000`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek,mt6735m-mmc"]` |  |
| reg | `<0x11230000 0x10000 0x10000e84 0x2>` |  |
| interrupts | `<0x0 0x4f 0x8>` |  |
| status | `["okay"]` |  |
| clk_src | `[02]` |  |
| bus-width | `<0x8>` |  |
| max-frequency | `<0xbebc200>` |  |
| cap-mmc-highspeed | `true (boolean)` |  |
| msdc-sys-suspend | `true (boolean)` |  |
| mmc-ddr-1_8v | `true (boolean)` |  |
| mmc-hs200-1_8v | `true (boolean)` |  |
| mmc-hs400-1_8v | `true (boolean)` |  |
| non-removable | `true (boolean)` |  |
| pinctl | `<0x2>` | /bus/pinctrl@0x10211000/mmc0@default |
| register_setting | `<0x3>` | /bus/pinctrl@0x10211000/mmc0@register_default |
| host_function | `[00]` |  |
| bootable | `true (boolean)` |  |

## `/mtk-msdc.0/msdc1@11240000`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek,mt6735m-mmc"]` |  |
| reg | `<0x11240000 0x10000 0x10000e84 0x2>` |  |
| interrupts | `<0x0 0x50 0x8>` |  |
| status | `["okay"]` |  |
| clk_src | `[02]` |  |
| bus-width | `<0x4>` |  |
| max-frequency | `<0xbebc200>` |  |
| msdc-sys-suspend | `true (boolean)` |  |
| cap-sd-highspeed | `true (boolean)` |  |
| sd-uhs-sdr12 | `true (boolean)` |  |
| sd-uhs-sdr25 | `true (boolean)` |  |
| sd-uhs-sdr50 | `true (boolean)` |  |
| sd-uhs-sdr104 | `true (boolean)` |  |
| sd-uhs-ddr50 | `true (boolean)` |  |
| pinctl | `<0x4>` | /bus/pinctrl@0x10211000/mmc1@default |
| pinctl_sdr104 | `<0x5>` | /bus/pinctrl@0x10211000/mmc1@sdr104 |
| pinctl_sdr50 | `<0x6>` | /bus/pinctrl@0x10211000/mmc1@sdr50 |
| pinctl_ddr50 | `<0x7>` | /bus/pinctrl@0x10211000/mmc1@ddr50 |
| register_setting | `<0x8>` | /bus/pinctrl@0x10211000/mmc1@register_default |
| host_function | `[01]` |  |
| cd_level | `[00]` |  |
| cd-gpios | `<0x9 0xb 0x0>` | /bus/pinctrl@0x10211000 args=[11, 0] |

## `/mtk-msdc.0/default`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek, msdc1_ins-eint"]` |  |
| interrupt-parent | `<0xa>` | /bus/eintc@10005000 |
| interrupts | `<0xb 0x8>` |  |
| debounce | `<0xb 0x0>` |  |
| status | `["okay"]` |  |

## `/atf_logger`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,atf_logger"]` |  |
| interrupts | `<0x0 0xf9 0x1>` |  |

## `/utos`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["microtrust,utos"]` |  |
| interrupts | `<0x0 0xf4 0x1 0x0 0xf5 0x1 0x0 0xf6 0x1 0x0 0xfa 0x1 0x0 0xfb 0x1 0x0 0xfc 0x1 0x0 0xfd 0x1 0x0 0xff 0x1>` |  |

## `/psci`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["arm,psci"]` |  |
| method | `["smc"]` |  |
| cpu_suspend | `<0x84000001>` |  |
| cpu_off | `<0x84000002>` |  |
| cpu_on | `<0x84000003>` |  |
| affinity_info | `<0x84000004>` |  |

## `/tkcore`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["trustkernel,tkcore"]` |  |
| interrupts | `<0x0 0xf8 0x0>` |  |

## `/cpus`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| #address-cells | `<0x1>` |  |
| #size-cells | `<0x0>` |  |

## `/cpus/cpu@000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| device_type | `["cpu"]` |  |
| compatible | `["arm,cortex-a53"]` |  |
| reg | `<0x0>` |  |
| enable-method | `["spin-table"]` |  |
| cpu-release-addr | `<0x0 0x40000200>` |  |
| clock-frequency | `<0x4190ab00>` |  |
| linux,phandle | `<0xb>` |  |
| phandle | `<0xb>` |  |

## `/cpus/cpu@001`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| device_type | `["cpu"]` |  |
| compatible | `["arm,cortex-a53"]` |  |
| reg | `<0x1>` |  |
| enable-method | `["spin-table"]` |  |
| cpu-release-addr | `<0x0 0x40000200>` |  |
| clock-frequency | `<0x4190ab00>` |  |
| linux,phandle | `<0xc>` |  |
| phandle | `<0xc>` |  |

## `/cpus/cpu@002`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| device_type | `["cpu"]` |  |
| compatible | `["arm,cortex-a53"]` |  |
| reg | `<0x2>` |  |
| enable-method | `["spin-table"]` |  |
| cpu-release-addr | `<0x0 0x40000200>` |  |
| clock-frequency | `<0x4190ab00>` |  |
| linux,phandle | `<0xd>` |  |
| phandle | `<0xd>` |  |

## `/cpus/cpu@003`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| device_type | `["cpu"]` |  |
| compatible | `["arm,cortex-a53"]` |  |
| reg | `<0x3>` |  |
| enable-method | `["spin-table"]` |  |
| cpu-release-addr | `<0x0 0x40000200>` |  |
| clock-frequency | `<0x4190ab00>` |  |
| linux,phandle | `<0xe>` |  |
| phandle | `<0xe>` |  |

## `/memory@00000000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| device_type | `["memory"]` |  |
| reg | `<0x0 0x40000000 0x0 0x3f000000>` |  |

## `/reserved-memory`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| #address-cells | `<0x2>` |  |
| #size-cells | `<0x2>` |  |
| ranges | `true (boolean)` |  |

## `/reserved-memory/atf-reserved-memory@43000000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt6735-atf-reserved-memory", "mediatek,mt6735m-atf-reserved-memory", "mediatek,mt6753-atf-reserved-memory"]` |  |
| no-map | `true (boolean)` |  |
| reg | `<0x0 0x43000000 0x0 0x30000>` |  |

## `/reserved-memory/reserve-memory-ccci_md1`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,reserve-memory-ccci_md1"]` |  |
| no-map | `true (boolean)` |  |
| size | `<0x0 0x3810000>` |  |
| alignment | `<0x0 0x2000000>` |  |
| alloc-ranges | `<0x0 0x40000000 0x0 0xc0000000>` |  |

## `/reserved-memory/consys-reserve-memory`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,consys-reserve-memory"]` |  |
| no-map | `true (boolean)` |  |
| size | `<0x0 0x100000>` |  |
| alignment | `<0x0 0x200000>` |  |

## `/reserved-memory/ram_console-reserved-memory@43f00000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,ram_console"]` |  |
| reg | `<0x0 0x43f00000 0x0 0x10000>` |  |

## `/reserved-memory/minirdump-reserved-memory@43ff0000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek, minirdump"]` |  |
| reg | `<0x0 0x43ff0000 0x0 0x10000>` |  |

## `/reserved-memory/pstore-reserved-memory@43f10000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,pstore"]` |  |
| reg | `<0x0 0x43f10000 0x0 0xe0000>` |  |

## `/interrupt-controller@10220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt6735-gic"]` |  |
| #interrupt-cells | `<0x3>` |  |
| #address-cells | `<0x0>` |  |
| interrupt-controller | `true (boolean)` |  |
| reg | `<0x0 0x10221000 0x0 0x1000 0x0 0x10222000 0x0 0x1000 0x0 0x10200620 0x0 0x1000>` |  |
| mediatek,wdt_irq | `<0xa0>` |  |
| linux,phandle | `<0x1>` |  |
| phandle | `<0x1>` |  |

## `/interrupt-controller@10220000/gic-cpuif@0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["arm,gic-cpuif"]` |  |
| cpuif-id | `<0x0>` |  |
| cpu | `<0xb>` | /cpus/cpu@000 |

## `/interrupt-controller@10220000/gic-cpuif@1`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["arm,gic-cpuif"]` |  |
| cpuif-id | `<0x1>` |  |
| cpu | `<0xc>` | /cpus/cpu@001 |

## `/interrupt-controller@10220000/gic-cpuif@2`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["arm,gic-cpuif"]` |  |
| cpuif-id | `<0x2>` |  |
| cpu | `<0xd>` | /cpus/cpu@002 |

## `/interrupt-controller@10220000/gic-cpuif@3`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["arm,gic-cpuif"]` |  |
| cpuif-id | `<0x3>` |  |
| cpu | `<0xe>` | /cpus/cpu@003 |

## `/soc`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["simple-bus"]` |  |
| #address-cells | `<0x1>` |  |
| #size-cells | `<0x1>` |  |
| ranges | `<0x0 0x0 0x0 0xffffffff>` |  |

## `/soc/cpuxgpt@10200000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt6735-cpuxgpt"]` |  |
| reg | `<0x10200000 0x1000>` |  |
| interrupts | `<0x0 0x40 0x4 0x0 0x41 0x4 0x0 0x42 0x4 0x0 0x43 0x4 0x0 0x44 0x4 0x0 0x45 0x4 0x0 0x46 0x4 0x0 0x47 0x4>` |  |

## `/soc/apxgpt@10004000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt6735-apxgpt"]` |  |
| reg | `<0x10004000 0x1000>` |  |
| interrupts | `<0x0 0x98 0x8>` |  |
| clock-frequency | `<0xc65d40>` |  |

## `/soc/pmu`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["arm,cortex-a7-pmu"]` |  |
| interrupts | `<0x0 0x8 0x8 0x0 0x9 0x8 0x0 0xa 0x8 0x0 0xb 0x8>` |  |
| interrupt-affinity | `<0xb 0xc 0xd 0xe>` |  |

## `/soc/timer`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["arm,armv8-timer"]` |  |
| interrupts | `<0x1 0xd 0x8 0x1 0xe 0x8 0x1 0xb 0x8 0x1 0xa 0x8>` |  |
| clock-frequency | `<0xc65d40>` |  |

## `/soc/mt_pmic_regulator`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_pmic"]` |  |

## `/soc/mt_pmic_regulator/buck_regulators`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_pmic_buck_regulators"]` |  |

## `/soc/mt_pmic_regulator/buck_regulators/buck_vpa`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| regulator-name | `["vpa"]` |  |
| regulator-min-microvolt | `<0x7a120>` |  |
| regulator-max-microvolt | `<0x37b1d0>` |  |
| regulator-ramp-delay | `<0xc350>` |  |
| regulator-enable-ramp-delay | `<0xb4>` |  |

## `/soc/mt_pmic_regulator/buck_regulators/buck_vproc`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| regulator-name | `["vproc"]` |  |
| regulator-min-microvolt | `<0x927c0>` |  |
| regulator-max-microvolt | `<0x154456>` |  |
| regulator-ramp-delay | `<0x186a>` |  |
| regulator-enable-ramp-delay | `<0xb4>` |  |
| regulator-always-on | `true (boolean)` |  |
| regulator-boot-on | `true (boolean)` |  |

## `/soc/mt_pmic_regulator/buck_regulators/buck_vcore1`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| regulator-name | `["vcore1"]` |  |
| regulator-min-microvolt | `<0x927c0>` |  |
| regulator-max-microvolt | `<0x154456>` |  |
| regulator-ramp-delay | `<0x186a>` |  |
| regulator-enable-ramp-delay | `<0xb4>` |  |
| regulator-always-on | `true (boolean)` |  |
| regulator-boot-on | `true (boolean)` |  |

## `/soc/mt_pmic_regulator/buck_regulators/buck_vsys22`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| regulator-name | `["vsys22"]` |  |
| regulator-min-microvolt | `<0x124f80>` |  |
| regulator-max-microvolt | `<0x1e6c16>` |  |
| regulator-ramp-delay | `<0x186a>` |  |
| regulator-enable-ramp-delay | `<0xb4>` |  |
| regulator-always-on | `true (boolean)` |  |
| regulator-boot-on | `true (boolean)` |  |

## `/soc/mt_pmic_regulator/buck_regulators/buck_vlte`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| regulator-name | `["vlte"]` |  |
| regulator-min-microvolt | `<0x927c0>` |  |
| regulator-max-microvolt | `<0x154456>` |  |
| regulator-ramp-delay | `<0x186a>` |  |
| regulator-enable-ramp-delay | `<0xb4>` |  |
| regulator-always-on | `true (boolean)` |  |
| regulator-boot-on | `true (boolean)` |  |

## `/soc/mt_pmic_regulator/ldo_regulators`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_pmic_ldo_regulators"]` |  |

## `/soc/mt_pmic_regulator/ldo_regulators/ldo_vaux18`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| regulator-name | `["vaux18"]` |  |
| regulator-min-microvolt | `<0x1b7740>` |  |
| regulator-max-microvolt | `<0x1b7740>` |  |
| regulator-enable-ramp-delay | `<0x108>` |  |
| regulator-boot-on | `true (boolean)` |  |
| linux,phandle | `<0xf>` |  |
| phandle | `<0xf>` |  |

## `/soc/mt_pmic_regulator/ldo_regulators/ldo_vtcxo_0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| regulator-name | `["vtcxo_0"]` |  |
| regulator-min-microvolt | `<0x2ab980>` |  |
| regulator-max-microvolt | `<0x2ab980>` |  |
| regulator-enable-ramp-delay | `<0x6e>` |  |
| regulator-boot-on | `true (boolean)` |  |
| linux,phandle | `<0x10>` |  |
| phandle | `<0x10>` |  |

## `/soc/mt_pmic_regulator/ldo_regulators/ldo_vtcxo_1`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| regulator-name | `["vtcxo_1"]` |  |
| regulator-min-microvolt | `<0x2ab980>` |  |
| regulator-max-microvolt | `<0x2ab980>` |  |
| regulator-enable-ramp-delay | `<0x6e>` |  |
| linux,phandle | `<0x11>` |  |
| phandle | `<0x11>` |  |

## `/soc/mt_pmic_regulator/ldo_regulators/ldo_vaud28`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| regulator-name | `["vaud28"]` |  |
| regulator-min-microvolt | `<0x2ab980>` |  |
| regulator-max-microvolt | `<0x2ab980>` |  |
| regulator-enable-ramp-delay | `<0x108>` |  |
| regulator-boot-on | `true (boolean)` |  |
| linux,phandle | `<0x12>` |  |
| phandle | `<0x12>` |  |

## `/soc/mt_pmic_regulator/ldo_regulators/ldo_vcn28`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| regulator-name | `["vcn28"]` |  |
| regulator-min-microvolt | `<0x2ab980>` |  |
| regulator-max-microvolt | `<0x2ab980>` |  |
| regulator-enable-ramp-delay | `<0x108>` |  |
| linux,phandle | `<0x22>` |  |
| phandle | `<0x22>` |  |

## `/soc/mt_pmic_regulator/ldo_regulators/ldo_vcama`

| Property | Value | Resolved references |
|---|---|---|
| regulator-name | `["vcama"]` |  |
| regulator-min-microvolt | `<0x16e360>` |  |
| regulator-max-microvolt | `<0x2ab980>` |  |
| regulator-enable-ramp-delay | `<0x108>` |  |
| regulator-default-on | `<0x0>` |  |
| status | `["okay"]` |  |
| linux,phandle | `<0x4e>` |  |
| phandle | `<0x4e>` |  |

## `/soc/mt_pmic_regulator/ldo_regulators/ldo_vcn33_bt`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| regulator-name | `["vcn33_bt"]` |  |
| regulator-min-microvolt | `<0x325aa0>` |  |
| regulator-max-microvolt | `<0x36ee80>` |  |
| regulator-enable-ramp-delay | `<0x108>` |  |
| linux,phandle | `<0x23>` |  |
| phandle | `<0x23>` |  |

## `/soc/mt_pmic_regulator/ldo_regulators/ldo_vcn33_wifi`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| regulator-name | `["vcn33_wifi"]` |  |
| regulator-min-microvolt | `<0x325aa0>` |  |
| regulator-max-microvolt | `<0x36ee80>` |  |
| regulator-enable-ramp-delay | `<0x108>` |  |
| linux,phandle | `<0x24>` |  |
| phandle | `<0x24>` |  |

## `/soc/mt_pmic_regulator/ldo_regulators/ldo_vusb33`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| regulator-name | `["vusb33"]` |  |
| regulator-min-microvolt | `<0x325aa0>` |  |
| regulator-max-microvolt | `<0x325aa0>` |  |
| regulator-enable-ramp-delay | `<0x108>` |  |
| regulator-boot-on | `true (boolean)` |  |

## `/soc/mt_pmic_regulator/ldo_regulators/ldo_vefuse`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| regulator-name | `["vefuse"]` |  |
| regulator-min-microvolt | `<0x1b7740>` |  |
| regulator-max-microvolt | `<0x2191c0>` |  |
| regulator-enable-ramp-delay | `<0x108>` |  |
| linux,phandle | `<0x13>` |  |
| phandle | `<0x13>` |  |

## `/soc/mt_pmic_regulator/ldo_regulators/ldo_vsim1`

| Property | Value | Resolved references |
|---|---|---|
| regulator-name | `["vsim1"]` |  |
| regulator-min-microvolt | `<0x19f0a0>` |  |
| regulator-max-microvolt | `<0x200b20>` |  |
| regulator-enable-ramp-delay | `<0x108>` |  |
| regulator-default-on | `<0x0>` |  |
| status | `["okay"]` |  |
| linux,phandle | `<0x14>` |  |
| phandle | `<0x14>` |  |

## `/soc/mt_pmic_regulator/ldo_regulators/ldo_vsim2`

| Property | Value | Resolved references |
|---|---|---|
| regulator-name | `["vsim2"]` |  |
| regulator-min-microvolt | `<0x19f0a0>` |  |
| regulator-max-microvolt | `<0x200b20>` |  |
| regulator-enable-ramp-delay | `<0x108>` |  |
| regulator-default-on | `<0x0>` |  |
| status | `["okay"]` |  |
| linux,phandle | `<0x15>` |  |
| phandle | `<0x15>` |  |

## `/soc/mt_pmic_regulator/ldo_regulators/ldo_vemc_3v3`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| regulator-name | `["vemc_3v3"]` |  |
| regulator-min-microvolt | `<0x1b7740>` |  |
| regulator-max-microvolt | `<0x325aa0>` |  |
| regulator-enable-ramp-delay | `<0x108>` |  |
| regulator-boot-on | `true (boolean)` |  |
| linux,phandle | `<0x16>` |  |
| phandle | `<0x16>` |  |

## `/soc/mt_pmic_regulator/ldo_regulators/ldo_vmch`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| regulator-name | `["vmch"]` |  |
| regulator-min-microvolt | `<0x2c4020>` |  |
| regulator-max-microvolt | `<0x325aa0>` |  |
| regulator-enable-ramp-delay | `<0x2c>` |  |
| linux,phandle | `<0x17>` |  |
| phandle | `<0x17>` |  |

## `/soc/mt_pmic_regulator/ldo_regulators/ldo_vtref`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| regulator-name | `["vtref"]` |  |
| regulator-min-microvolt | `<0x1b7740>` |  |
| regulator-max-microvolt | `<0x1b7740>` |  |
| regulator-enable-ramp-delay | `<0xf0>` |  |
| linux,phandle | `<0x18>` |  |
| phandle | `<0x18>` |  |

## `/soc/mt_pmic_regulator/ldo_regulators/ldo_vmc`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| regulator-name | `["vmc"]` |  |
| regulator-min-microvolt | `<0x1b7740>` |  |
| regulator-max-microvolt | `<0x325aa0>` |  |
| regulator-enable-ramp-delay | `<0x2c>` |  |
| linux,phandle | `<0x19>` |  |
| phandle | `<0x19>` |  |

## `/soc/mt_pmic_regulator/ldo_regulators/ldo_vcamaf`

| Property | Value | Resolved references |
|---|---|---|
| regulator-name | `["vcamaf"]` |  |
| regulator-min-microvolt | `<0x124f80>` |  |
| regulator-max-microvolt | `<0x325aa0>` |  |
| regulator-enable-ramp-delay | `<0x108>` |  |
| regulator-default-on | `<0x0>` |  |
| status | `["okay"]` |  |
| linux,phandle | `<0x50>` |  |
| phandle | `<0x50>` |  |

## `/soc/mt_pmic_regulator/ldo_regulators/ldo_vio28`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| regulator-name | `["vio28"]` |  |
| regulator-min-microvolt | `<0x2ab980>` |  |
| regulator-max-microvolt | `<0x2ab980>` |  |
| regulator-enable-ramp-delay | `<0x108>` |  |
| regulator-boot-on | `true (boolean)` |  |
| linux,phandle | `<0x1a>` |  |
| phandle | `<0x1a>` |  |

## `/soc/mt_pmic_regulator/ldo_regulators/ldo_vgp1`

| Property | Value | Resolved references |
|---|---|---|
| regulator-name | `["vgp1"]` |  |
| regulator-min-microvolt | `<0x124f80>` |  |
| regulator-max-microvolt | `<0x325aa0>` |  |
| regulator-enable-ramp-delay | `<0x108>` |  |
| regulator-default-on | `<0x0>` |  |
| status | `["okay"]` |  |
| linux,phandle | `<0x34>` |  |
| phandle | `<0x34>` |  |

## `/soc/mt_pmic_regulator/ldo_regulators/ldo_vibr`

| Property | Value | Resolved references |
|---|---|---|
| regulator-name | `["vibr"]` |  |
| regulator-min-microvolt | `<0x124f80>` |  |
| regulator-max-microvolt | `<0x325aa0>` |  |
| regulator-enable-ramp-delay | `<0x2c>` |  |
| regulator-default-on | `<0x0>` |  |
| status | `["okay"]` |  |
| linux,phandle | `<0x1b>` |  |
| phandle | `<0x1b>` |  |

## `/soc/mt_pmic_regulator/ldo_regulators/ldo_vcamd`

| Property | Value | Resolved references |
|---|---|---|
| regulator-name | `["vcamd"]` |  |
| regulator-min-microvolt | `<0xdbba0>` |  |
| regulator-max-microvolt | `<0x16e360>` |  |
| regulator-enable-ramp-delay | `<0x108>` |  |
| regulator-default-on | `<0x0>` |  |
| status | `["okay"]` |  |
| linux,phandle | `<0x4f>` |  |
| phandle | `<0x4f>` |  |

## `/soc/mt_pmic_regulator/ldo_regulators/ldo_vrf18_0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| regulator-name | `["vrf18_0"]` |  |
| regulator-min-microvolt | `<0x1bd8e8>` |  |
| regulator-max-microvolt | `<0x1bd8e8>` |  |
| regulator-enable-ramp-delay | `<0xdc>` |  |
| linux,phandle | `<0x1c>` |  |
| phandle | `<0x1c>` |  |

## `/soc/mt_pmic_regulator/ldo_regulators/ldo_vrf18_1`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| regulator-name | `["vrf18_1"]` |  |
| regulator-min-microvolt | `<0x124f80>` |  |
| regulator-max-microvolt | `<0x1bd8e8>` |  |
| regulator-enable-ramp-delay | `<0xdc>` |  |
| linux,phandle | `<0x1d>` |  |
| phandle | `<0x1d>` |  |

## `/soc/mt_pmic_regulator/ldo_regulators/ldo_vio18`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| regulator-name | `["vio18"]` |  |
| regulator-min-microvolt | `<0x1b7740>` |  |
| regulator-max-microvolt | `<0x1b7740>` |  |
| regulator-enable-ramp-delay | `<0x108>` |  |
| regulator-boot-on | `true (boolean)` |  |
| linux,phandle | `<0x1e>` |  |
| phandle | `<0x1e>` |  |

## `/soc/mt_pmic_regulator/ldo_regulators/ldo_vcn18`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| regulator-name | `["vcn18"]` |  |
| regulator-min-microvolt | `<0x1b7740>` |  |
| regulator-max-microvolt | `<0x1b7740>` |  |
| regulator-enable-ramp-delay | `<0x2c>` |  |
| linux,phandle | `<0x21>` |  |
| phandle | `<0x21>` |  |

## `/soc/mt_pmic_regulator/ldo_regulators/ldo_vcamio`

| Property | Value | Resolved references |
|---|---|---|
| regulator-name | `["vcamio"]` |  |
| regulator-min-microvolt | `<0x124f80>` |  |
| regulator-max-microvolt | `<0x1b7740>` |  |
| regulator-enable-ramp-delay | `<0xdc>` |  |
| regulator-default-on | `<0x0>` |  |
| status | `["okay"]` |  |
| linux,phandle | `<0x51>` |  |
| phandle | `<0x51>` |  |

## `/soc/mt_pmic_regulator/ldo_regulators/ldo_vsram`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| regulator-name | `["vsram"]` |  |
| regulator-min-microvolt | `<0xaae60>` |  |
| regulator-max-microvolt | `<0x16caf6>` |  |
| regulator-enable-ramp-delay | `<0xdc>` |  |
| regulator-ramp-delay | `<0x186a>` |  |
| regulator-boot-on | `true (boolean)` |  |
| linux,phandle | `<0x1f>` |  |
| phandle | `<0x1f>` |  |

## `/soc/mt_pmic_regulator/ldo_regulators/ldo_vm`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| regulator-name | `["vm"]` |  |
| regulator-min-microvolt | `<0x12ebc0>` |  |
| regulator-max-microvolt | `<0x177fa0>` |  |
| regulator-enable-ramp-delay | `<0x108>` |  |
| regulator-boot-on | `true (boolean)` |  |
| linux,phandle | `<0x20>` |  |
| phandle | `<0x20>` |  |

## `/soc/mt_pmic_regulator/regulators_supply`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_pmic_regulator_supply"]` |  |
| vaux18-supply | `<0xf>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vaux18 |
| vtcxo_0-supply | `<0x10>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vtcxo_0 |
| vtcxo_1-supply | `<0x11>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vtcxo_1 |
| vaud28-supply | `<0x12>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vaud28 |
| vefuse-supply | `<0x13>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vefuse |
| vsim1-supply | `<0x14>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vsim1 |
| vsim2-supply | `<0x15>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vsim2 |
| vemc_3v3-supply | `<0x16>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vemc_3v3 |
| vmch-supply | `<0x17>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vmch |
| vtref-supply | `<0x18>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vtref |
| vmc-supply | `<0x19>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vmc |
| vio28-supply | `<0x1a>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vio28 |
| vibr-supply | `<0x1b>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vibr |
| vrf18_0-supply | `<0x1c>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vrf18_0 |
| vrf18_1-supply | `<0x1d>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vrf18_1 |
| vio18-supply | `<0x1e>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vio18 |
| vsram-supply | `<0x1f>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vsram |
| vm-supply | `<0x20>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vm |

## `/soc/toprgu@10212000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt6735-rgu"]` |  |
| reg | `<0x10212000 0x1000>` |  |
| interrupts | `<0x0 0x80 0x2>` |  |

## `/soc/mcu_biu@10300000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt6735-mcu_biu"]` |  |
| reg | `<0x10300000 0x8000>` |  |

## `/soc/gpio`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,gpio_usage_mapping"]` |  |
| GPIO_SIM1_HOT_PLUG | `<0x8>` |  |
| GPIO_SIM2_HOT_PLUG | `<0x9>` |  |
| GPIO_SIM2_SCLK | `<0xa0>` |  |
| GPIO_SIM2_SRST | `<0xa1>` |  |
| GPIO_SIM2_SIO | `<0xa2>` |  |
| GPIO_SIM1_SCLK | `<0xa3>` |  |
| GPIO_SIM1_SRST | `<0xa4>` |  |
| GPIO_SIM1_SIO | `<0xa5>` |  |

## `/soc/gpio@10211000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,gpio"]` |  |
| reg | `<0x10211000 0x1000>` |  |

## `/soc/dramc_nao@1020e000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt6735-dramc_nao"]` |  |
| reg | `<0x1020e000 0x1000>` |  |

## `/soc/ddrphy@10213000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt6735-ddrphy"]` |  |
| reg | `<0x10213000 0x1000>` |  |

## `/soc/dramc@10214000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt6735-dramc"]` |  |
| reg | `<0x10214000 0x1000>` |  |
| clock-names | `["infra-cqdma"]` |  |

## `/soc/keypad@10003000`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek,mt6735-keypad", "mediatek,mt6735m-keypad"]` |  |
| reg | `<0x10003000 0x1000>` |  |
| interrupts | `<0x0 0xa4 0x2>` |  |
| mediatek,kpd-key-debounce | `<0x400>` |  |
| mediatek,kpd-sw-pwrkey | `<0x74>` |  |
| mediatek,kpd-hw-pwrkey | `<0x8>` |  |
| mediatek,kpd-use-extend-type | `<0x0>` |  |
| mediatek,kpd-hw-map-num | `<0x48>` |  |
| mediatek,kpd-hw-init-map | `288 bytes; sha256=9d9b05d150767545bd55b9a88f5e5d3a20b6003ea9b3210cf20959058f1b8ed3 (large property omitted)` |  |
| mediatek,kpd-pwrkey-eint-gpio | `<0x0>` |  |
| mediatek,kpd-pwkey-gpio-din | `<0x0>` |  |
| mediatek,kpd-hw-dl-key0 | `<0x1>` |  |
| mediatek,kpd-hw-dl-key1 | `<0x0>` |  |
| mediatek,kpd-hw-dl-key2 | `<0x8>` |  |
| mediatek,kpd-hw-recovery-key | `<0x1>` |  |
| mediatek,kpd-hw-factory-key | `<0x0>` |  |
| status | `["okay"]` |  |

## `/soc/irtx@11011000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,irtx"]` |  |
| reg | `<0x11011000 0x1000>` |  |
| interrupts | `<0x0 0x7c 0x4>` |  |
| pwm_ch | `<0x0>` |  |

## `/soc/apuart0@11002000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| cell-index | `<0x0>` |  |
| compatible | `["mediatek,mt6735-uart"]` |  |
| reg | `<0x11002000 0x1000 0x11000380 0x1000 0x11000400 0x80>` |  |
| interrupts | `<0x0 0x5b 0x8 0x0 0x67 0x8 0x0 0x68 0x8>` |  |

## `/soc/apuart1@11003000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| cell-index | `<0x1>` |  |
| compatible | `["mediatek,mt6735-uart"]` |  |
| reg | `<0x11003000 0x1000 0x11000480 0x80 0x11000500 0x80>` |  |
| interrupts | `<0x0 0x5c 0x8 0x0 0x69 0x8 0x0 0x6a 0x8>` |  |

## `/soc/apuart2@11004000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| cell-index | `<0x2>` |  |
| compatible | `["mediatek,mt6735-uart"]` |  |
| reg | `<0x11004000 0x1000 0x11000580 0x80 0x11000600 0x80>` |  |
| interrupts | `<0x0 0x5d 0x8 0x0 0x6b 0x8 0x0 0x6c 0x8>` |  |

## `/soc/apuart3@11005000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| cell-index | `<0x3>` |  |
| compatible | `["mediatek,mt6735-uart"]` |  |
| reg | `<0x11005000 0x1000 0x11000680 0x80 0x11000700 0x80>` |  |
| interrupts | `<0x0 0x5e 0x8 0x0 0x6d 0x8 0x0 0x6e 0x8>` |  |

## `/soc/spi@1100a000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt6735m-spi"]` |  |
| cell-index | `<0x0>` |  |
| spi-padmacro | `<0x0>` |  |
| reg | `<0x1100a000 0x1000>` |  |
| interrupts | `<0x0 0x76 0x8>` |  |

## `/soc/btif_tx@11000780`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,btif_tx"]` |  |
| reg | `<0x11000780 0x80>` |  |
| interrupts | `<0x0 0x6f 0x8>` |  |

## `/soc/btif_rx@11000800`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,btif_rx"]` |  |
| reg | `<0x11000800 0x80>` |  |
| interrupts | `<0x0 0x70 0x8>` |  |

## `/soc/btif@1100c000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,btif"]` |  |
| reg | `<0x1100c000 0x1000>` |  |
| interrupts | `<0x0 0x5a 0x8>` |  |

## `/soc/consys@18070000`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek,mt6735m-consys", "mediatek,mt6735-consys"]` |  |
| reg | `<0x18070000 0x200 0x10212000 0x100 0x10000000 0x2000 0x10006000 0x1000>` |  |
| interrupts | `<0x0 0xe3 0x8 0x0 0xe1 0x8>` |  |
| vcn18-supply | `<0x21>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vcn18 |
| vcn28-supply | `<0x22>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vcn28 |
| vcn33_bt-supply | `<0x23>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vcn33_bt |
| vcn33_wifi-supply | `<0x24>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vcn33_wifi |
| pinctrl-names | `["default", "gps_lna_state_init", "gps_lna_state_oh", "gps_lna_state_ol"]` |  |
| pinctrl-0 | `<0x25>` | /bus/pinctrl@0x10211000/default |
| pinctrl-1 | `<0x26>` | /bus/pinctrl@0x10211000/gpslna@0 |
| pinctrl-2 | `<0x27>` | /bus/pinctrl@0x10211000/gpslna@1 |
| pinctrl-3 | `<0x28>` | /bus/pinctrl@0x10211000/gpslna@2 |
| status | `["okay"]` |  |

## `/soc/hacc@10008000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,hacc"]` |  |
| reg | `<0x10008000 0x1000>` |  |
| interrupts | `<0x0 0xae 0x8>` |  |

## `/soc/hall_1@1`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt6580-hall"]` |  |

## `/soc/als`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek, als-eint"]` |  |
| interrupt-parent | `<0xa>` | /bus/eintc@10005000 |
| interrupts | `<0x3c 0x8>` |  |
| debounce | `<0x3c 0x0>` |  |
| status | `["okay"]` |  |

## `/soc/gse_1`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek, gse_1-eint"]` |  |
| status | `["disabled"]` |  |

## `/soc/ext_buck_oc`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek, ext_buck_oc-eint"]` |  |
| status | `["disabled"]` |  |

## `/soc/swtp`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek, swtp-eint"]` |  |

## `/bus`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["simple-bus"]` |  |
| #address-cells | `<0x1>` |  |
| #size-cells | `<0x1>` |  |
| ranges | `<0x0 0x0 0x0 0xffffffff>` |  |

## `/bus/INFRACFG_AO@0x10000000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,INFRACFG_AO"]` |  |
| reg | `<0x10000000 0x1000>` |  |

## `/bus/usb2jtag@10000000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,usb2jtag_v1"]` |  |
| reg | `<0x10000000 0x1000 0x11210000 0x1000>` |  |

## `/bus/PWRAP@0x10001000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,PWRAP"]` |  |
| reg | `<0x10001000 0x1000>` |  |
| interrupts | `<0x0 0xa3 0x4>` |  |

## `/bus/PERICFG@0x10002000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,PERICFG"]` |  |
| reg | `<0x10002000 0x1000>` |  |

## `/bus/FHCTL@0x10209F00`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,FHCTL"]` |  |
| reg | `<0x10209f00 0x100>` |  |

## `/bus/KP@0x10003000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,KP"]` |  |
| reg | `<0x10003000 0x1000>` |  |
| interrupts | `<0x0 0xa4 0x2>` |  |

## `/bus/eintc@10005000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt-eic"]` |  |
| reg | `<0x10005000 0x1000>` |  |
| interrupts | `<0x0 0x99 0x4>` |  |
| #interrupt-cells | `<0x2>` |  |
| interrupt-controller | `true (boolean)` |  |
| mediatek,max_eint_num | `<0xd5>` |  |
| mediatek,mapping_table_entry | `<0x0>` |  |
| mediatek,debtime_setting_entry | `<0xa>` |  |
| mediatek,debtime_setting_array | `<0x0 0x7d 0x1 0xfa 0x2 0x1f4 0x3 0x3e8 0x4 0x3e80 0x5 0x7d00 0x6 0xfa00 0x7 0x1f400 0x8 0x3e800 0x9 0x7d000>` |  |
| linux,phandle | `<0xa>` |  |
| phandle | `<0xa>` |  |

## `/bus/eintc@10005000/MD1_SIM1_HOT_PLUG_EINT@0`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek,MD1_SIM1_HOT_PLUG_EINT-eint"]` |  |
| interrupts | `<0x0 0x4>` |  |
| debounce | `<0x0 0x186a0>` |  |
| dedicated | `<0x0 0x0>` |  |
| src_pin | `<0x0 0x1>` |  |
| sockettype | `<0x0 0x0>` |  |
| status | `["okay"]` |  |

## `/bus/eintc@10005000/MD1_SIM2_HOT_PLUG_EINT@1`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek,MD1_SIM2_HOT_PLUG_EINT-eint"]` |  |
| interrupts | `<0x1 0x4>` |  |
| debounce | `<0x1 0x186a0>` |  |
| dedicated | `<0x1 0x0>` |  |
| src_pin | `<0x1 0x2>` |  |
| sockettype | `<0x1 0x0>` |  |
| status | `["okay"]` |  |

## `/bus/eintc@10005000/pmic@206`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek, pmic-eint"]` |  |
| interrupt-parent | `<0xa>` | /bus/eintc@10005000 |
| interrupts | `<0xce 0x4>` |  |
| debounce | `<0xce 0x3e8>` |  |

## `/bus/SLEEP@0x10006000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,SLEEP"]` |  |
| reg | `<0x10006000 0x1000>` |  |
| interrupts | `<0x0 0xa5 0x8 0x0 0xa6 0x8 0x0 0xa7 0x8 0x0 0xa8 0x8>` |  |

## `/bus/DEVAPC_AO@10007000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,DEVAPC_AO"]` |  |
| reg | `<0x10007000 0x1000>` |  |

## `/bus/RSVD@0x10009000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,RSVD"]` |  |
| reg | `<0x10009000 0x1000>` |  |

## `/bus/bat_meter`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,bat_meter"]` |  |
| r_bat_sense | `<0x4>` |  |
| r_i_sense | `<0x4>` |  |
| r_charger_1 | `<0x14a>` |  |
| r_charger_2 | `<0x27>` |  |
| temperature_t0 | `<0x6e>` |  |
| temperature_t1 | `<0x0>` |  |
| temperature_t2 | `<0x19>` |  |
| temperature_t3 | `<0x32>` |  |
| temperature_t | `<0xff>` |  |
| fg_meter_resistance | `<0x0>` |  |
| q_max_pos_50 | `<0x9f2>` |  |
| q_max_pos_25 | `<0x9cc>` |  |
| q_max_pos_0 | `<0x883>` |  |
| q_max_neg_10 | `<0x709>` |  |
| q_max_pos_50_h_current | `<0x9db>` |  |
| q_max_pos_25_h_current | `<0x9b7>` |  |
| q_max_pos_0_h_current | `<0x70a>` |  |
| q_max_neg_10_h_current | `<0x225>` |  |
| oam_d5 | `<0x1>` |  |
| change_tracking_point | `<0x1>` |  |
| cust_tracking_point | `<0x1>` |  |
| cust_r_sense | `<0x64>` |  |
| cust_hw_cc | `<0x0>` |  |
| aging_tuning_value | `<0x67>` |  |
| cust_r_fg_offset | `<0x0>` |  |
| ocv_board_compesate | `<0x0>` |  |
| r_fg_board_base | `<0x3e8>` |  |
| r_fg_board_slope | `<0x3e8>` |  |
| car_tune_value | `<0x62>` |  |
| current_detect_r_fg | `<0xa>` |  |
| minerroroffset | `<0x3e8>` |  |
| fg_vbat_average_size | `<0x12>` |  |
| r_fg_value | `<0xa>` |  |
| cust_poweron_delta_capacity_tolrance | `<0x1e>` |  |
| cust_poweron_low_capacity_tolrance | `<0x5>` |  |
| cust_poweron_max_vbat_tolrance | `<0x5a>` |  |
| cust_poweron_delta_vbat_tolrance | `<0x1e>` |  |
| cust_poweron_delta_hw_sw_ocv_capacity_tolrance | `<0xa>` |  |
| fixed_tbat_25 | `<0x0>` |  |
| vbat_normal_wakeup | `<0xe10>` |  |
| vbat_low_power_wakeup | `<0xdac>` |  |
| normal_wakeup_period | `<0x1518>` |  |
| low_power_wakeup_period | `<0x12c>` |  |
| close_poweroff_wakeup_period | `<0x1e>` |  |
| rbat_pull_up_r | `<0x4204>` |  |
| rbat_pull_up_volt | `<0x708>` |  |
| batt_temperature_table_num | `<0x11>` |  |
| batt_temperature_table | `<0xffffffec 0x10a8d 0xfffffff1 0xd192 0xfffffff6 0xa60a 0xfffffffb 0x8464 0x0 0x6a53 0x5 0x5605 0xa 0x4606 0xf 0x3952 0x14 0x2f31 0x19 0x2710 0x1e 0x207b 0x23 0x1b24 0x28 0x16ca 0x2d 0x1335 0x32 0x1041 0x37 0xdcf 0x3c 0xbc6>` |  |
| battery_profile_t0_num | `<0x34>` |  |
| battery_profile_t0 | `416 bytes; sha256=6a0b45318d400f6274626b574eff4ed44987c589b44d239895332000f6250d3d (large property omitted)` |  |
| battery_profile_t1_num | `<0x34>` |  |
| battery_profile_t1 | `416 bytes; sha256=735c4c07a488874ebee18e83c545e0d5f56ad9045670ccd8e2ed5e8626f41060 (large property omitted)` |  |
| battery_profile_t2_num | `<0x34>` |  |
| battery_profile_t2 | `416 bytes; sha256=6a26b45ecd1b0ffb8e6a28c94c9e27a7a6bf683785371a1ea9b00c0c5d1014d9 (large property omitted)` |  |
| battery_profile_t3_num | `<0x34>` |  |
| battery_profile_t3 | `416 bytes; sha256=c694bd2944b0fa515c2600c4d8338db8426de5f0fa7fac7a0a4bcc0700bce21f (large property omitted)` |  |
| r_profile_t0_num | `<0x34>` |  |
| r_profile_t0 | `416 bytes; sha256=fb3dd07fc7e5e86f90f0a1332b8faa0e114ac027e7606ad3f8c03758614a91ea (large property omitted)` |  |
| r_profile_t1_num | `<0x34>` |  |
| r_profile_t1 | `416 bytes; sha256=0a79ec0a9553fb4adbd24f3e2619f612770a389fa610b7c102bb14e88a86445d (large property omitted)` |  |
| r_profile_t2_num | `<0x34>` |  |
| r_profile_t2 | `416 bytes; sha256=921e0b07d3cc29a45c93e9fe9858c28cd59e8a06bcc50002d1ad78ce361b7582 (large property omitted)` |  |
| r_profile_t3_num | `<0x34>` |  |
| r_profile_t3 | `416 bytes; sha256=6612c0d746c094e038600d6dadf169e82248b3c5baa165f3dd4cf809102feeaf (large property omitted)` |  |

## `/bus/bat_notify`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,bat_notify"]` |  |

## `/bus/bat_comm`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,battery"]` |  |
| stop_charging_in_takling | `<0x1>` |  |
| talking_recharge_voltage | `<0xed8>` |  |
| talking_sync_time | `<0x3c>` |  |
| mtk_temperature_recharge_support | `<0x1>` |  |
| max_charge_temperature | `<0x39>` |  |
| max_charge_temperature_minus_x_degree | `<0x32>` |  |
| min_charge_temperature | `<0x0>` |  |
| min_charge_temperature_plus_x_degree | `<0x6>` |  |
| err_charge_temperature | `<0xff>` |  |
| v_pre2cc_thres | `<0xd48>` |  |
| v_cc2topoff_thres | `<0x1068>` |  |
| recharging_voltage | `<0x10b8>` |  |
| charging_full_current | `<0x96>` |  |
| config_usb_if | `<0x0>` |  |
| usb_charger_current_suspend | `<0x0>` |  |
| usb_charger_current_unconfigured | `<0x1b58>` |  |
| usb_charger_current_configured | `<0xc350>` |  |
| usb_charger_current | `<0xc350>` |  |
| ac_charger_current | `<0x13880>` |  |
| non_std_ac_charger_current | `<0xc350>` |  |
| charging_host_charger_current | `<0xfde8>` |  |
| apple_0_5a_charger_current | `<0xc350>` |  |
| apple_1_0a_charger_current | `<0xfde8>` |  |
| apple_2_1a_charger_current | `<0x13880>` |  |
| bat_low_temp_protect_enable | `<0x1>` |  |
| v_charger_enable | `<0x1>` |  |
| v_charger_max | `<0x1770>` |  |
| v_charger_min | `<0x1130>` |  |
| onehundred_percent_tracking_time | `<0xa>` |  |
| npercent_tracking_time | `<0x14>` |  |
| sync_to_real_tracking_time | `<0x3c>` |  |
| v_0percent_tracking | `<0xd7a>` |  |
| high_battery_voltage_support | `<0x1>` |  |

## `/bus/mdcldma@1000A000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mdcldma"]` |  |
| reg | `<0x1000a000 0x1000 0x1000b000 0x1000 0x1021a000 0x1000 0x1021b000 0x1000 0x1020a000 0x1000 0x1020b000 0x1000>` |  |
| interrupts | `<0x0 0x91 0x4 0x0 0x8c 0x8 0x0 0xdd 0x2>` |  |
| mediatek,md_id | `<0x0>` |  |
| mediatek,cldma_capability | `<0x2>` |  |
| mediatek,md_smem_size | `<0x10000>` |  |
| pinctrl-names | `["default", "vsram_output_low", "vsram_output_high", "RFIC0_01_mode", "RFIC0_04_mode"]` |  |
| pinctrl-0 | `<0x29>` | /bus/pinctrl@0x10211000/vsram0default |
| pinctrl-1 | `<0x2a>` | /bus/pinctrl@0x10211000/vsram@1 |
| pinctrl-2 | `<0x2b>` | /bus/pinctrl@0x10211000/vsram@2 |
| pinctrl-3 | `<0x2c>` | /bus/pinctrl@0x10211000/clockbuf@1 |
| pinctrl-4 | `<0x2d>` | /bus/pinctrl@0x10211000/clockbuf@2 |

## `/bus/dbgapb_base@1011A000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,dbgapb_base"]` |  |
| reg | `<0x1011a000 0x100>` |  |

## `/bus/simswitch@0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,sim_switch"]` |  |
| pinctrl-names | `["default", "hot_plug_mode1", "hot_plug_mode2", "two_sims_bound_to_md1", "sim1_md3_sim2_md1"]` |  |
| pinctrl-0 | `<0x2e>` | /bus/pinctrl@0x10211000/ssw0default |
| pinctrl-1 | `<0x2f>` | /bus/pinctrl@0x10211000/ssw@1 |
| pinctrl-2 | `<0x30>` | /bus/pinctrl@0x10211000/ssw@2 |
| pinctrl-3 | `<0x31>` | /bus/pinctrl@0x10211000/ssw@3 |
| pinctrl-4 | `<0x32>` | /bus/pinctrl@0x10211000/ssw@4 |

## `/bus/DNL3_XGPT64@0x1000C000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,DNL3_XGPT64"]` |  |
| reg | `<0x1000c000 0x1000>` |  |
| interrupts | `<0x0 0x9f 0x8>` |  |

## `/bus/MCUCFG@0x10200000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,MCUCFG"]` |  |
| reg | `<0x10200000 0x200>` |  |
| interrupts | `<0x0 0x0 0x8>` |  |

## `/bus/mcucfg@10200000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt6735-mcucfg"]` |  |
| reg | `<0x10200000 0x200>` |  |
| interrupts | `<0x0 0x47 0x4>` |  |

## `/bus/RSVD@0x10200200`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,RSVD"]` |  |
| reg | `<0x10200200 0x200>` |  |

## `/bus/MCUSYS_MISCCFG@0x10200400`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,MCUSYS_MISCCFG"]` |  |
| reg | `<0x10200400 0x200>` |  |

## `/bus/MCUSYS_MCUCFG@0x10200600`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,MCUSYS_MCUCFG"]` |  |
| reg | `<0x10200600 0xa00>` |  |

## `/bus/INFRACFG@0x10201000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,INFRACFG"]` |  |
| reg | `<0x10201000 0x1000>` |  |

## `/bus/SRAMROM@0x10202000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,SRAMROM"]` |  |
| reg | `<0x10202000 0x1000>` |  |

## `/bus/EMI@0x10203000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,EMI"]` |  |
| reg | `<0x10203000 0x1000>` |  |
| interrupts | `<0x0 0x88 0x4>` |  |

## `/bus/sys_cirq@10204000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt6735-sys_cirq"]` |  |
| reg | `<0x10204000 0x1000>` |  |
| interrupts | `<0x0 0xe7 0x8>` |  |
| mediatek,cirq_num | `<0x9f>` |  |
| mediatek,spi_start_offset | `<0x48>` |  |

## `/bus/m4u@10205000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| cell-index | `<0x0>` |  |
| compatible | `["mediatek,m4u"]` |  |
| reg | `<0x10205000 0x1000>` |  |
| interrupts | `<0x0 0x92 0x8>` |  |

## `/bus/EFUSEC@10206000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,EFUSEC"]` |  |
| reg | `<0x10206000 0x1000>` |  |

## `/bus/DEVAPC@10207000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,DEVAPC"]` |  |
| reg | `<0x10207000 0x1000>` |  |
| interrupts | `<0x0 0x86 0x8>` |  |

## `/bus/bus_dbg@10208000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,bus_dbg-v1"]` |  |
| reg | `<0x10208000 0x1000>` |  |
| interrupts | `<0x0 0x89 0x8>` |  |

## `/bus/APMIXED@0x10209000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,APMIXED"]` |  |
| reg | `<0x10209000 0x1000>` |  |

## `/bus/RSVD@0x1020C000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,RSVD"]` |  |
| reg | `<0x1020c000 0x1000>` |  |

## `/bus/INFRA_MBIST@0x1020D000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,INFRA_MBIST"]` |  |
| reg | `<0x1020d000 0x1000>` |  |

## `/bus/TRNG@0x1020F000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,TRNG"]` |  |
| reg | `<0x1020f000 0x1000>` |  |
| interrupts | `<0x0 0x8d 0x8>` |  |

## `/bus/CKSYS@0x10210000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,CKSYS"]` |  |
| reg | `<0x10210000 0x1000>` |  |

## `/bus/MIPI_RX_ANA_CSI0@0x10215800`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,MIPI_RX_ANA_CSI0"]` |  |
| reg | `<0x10215800 0x400>` |  |

## `/bus/MIPI_RX_ANA_CSI1@0x10215C00`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,MIPI_RX_ANA_CSI1"]` |  |
| reg | `<0x10215c00 0x400>` |  |

## `/bus/gcpu@10216000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,gcpu"]` |  |
| reg | `<0x10216000 0x1000>` |  |
| interrupts | `<0x0 0x96 0x8>` |  |

## `/bus/gce@10217000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,gce"]` |  |
| reg | `<0x10217000 0xc00>` |  |
| interrupts | `<0x0 0x97 0x8 0x0 0x94 0x8>` |  |
| disp_mutex_reg | `<0x14015000 0x1000>` |  |
| g3d_config_base | `<0x13000000 0x0 0xffff0000>` |  |
| mmsys_config_base | `<0x14000000 0x1 0xffff0000>` |  |
| disp_dither_base | `<0x14010000 0x2 0xffff0000>` |  |
| mm_na_base | `<0x14020000 0x3 0xffff0000>` |  |
| imgsys_base | `<0x15000000 0x4 0xffff0000>` |  |
| vdec_gcon_base | `<0x16000000 0x5 0xffff0000>` |  |
| venc_gcon_base | `<0x17000000 0x6 0xffff0000>` |  |
| conn_peri_base | `<0x18000000 0x7 0xffff0000>` |  |
| topckgen_base | `<0x10000000 0x8 0xffff0000>` |  |
| kp_base | `<0x10010000 0x9 0xffff0000>` |  |
| scp_sram_base | `<0x10020000 0xa 0xffff0000>` |  |
| infra_na3_base | `<0x10030000 0xb 0xffff0000>` |  |
| infra_na4_base | `<0x10040000 0xc 0xffff0000>` |  |
| scp_base | `<0x10050000 0xd 0xffff0000>` |  |
| mcucfg_base | `<0x10200000 0xe 0xffff0000>` |  |
| gcpu_base | `<0x10210000 0xf 0xffff0000>` |  |
| usb0_base | `<0x11200000 0x10 0xffff0000>` |  |
| usb_sif_base | `<0x11210000 0x11 0xffff0000>` |  |
| audio_base | `<0x11220000 0x12 0xffff0000>` |  |
| msdc0_base | `<0x11230000 0x13 0xffff0000>` |  |
| msdc1_base | `<0x11240000 0x14 0xffff0000>` |  |
| msdc2_base | `<0x11250000 0x15 0xffff0000>` |  |
| msdc3_base | `<0x11260000 0x16 0xffff0000>` |  |
| pwm_sw_base | `<0x1100e000 0x63 0xfffff000>` |  |
| mdp_rdma0_sof | `<0x0>` |  |
| mdp_rsz0_sof | `<0x1>` |  |
| mdp_rsz1_sof | `<0x2>` |  |
| mdp_tdshp_sof | `<0x3>` |  |
| mdp_wdma_sof | `<0x4>` |  |
| mdp_wrot_sof | `<0x5>` |  |
| disp_ovl0_sof | `<0x6>` |  |
| disp_rdma0_sof | `<0x8>` |  |
| disp_rdma1_sof | `<0x9>` |  |
| disp_wdma0_sof | `<0xa>` |  |
| disp_ccorr_sof | `<0xb>` |  |
| disp_color_sof | `<0xc>` |  |
| disp_aal_sof | `<0xd>` |  |
| disp_gamma_sof | `<0xe>` |  |
| disp_dither_sof | `<0xf>` |  |
| disp_pwm0_sof | `<0x11>` |  |
| mdp_rdma0_frame_done | `<0x12>` |  |
| mdp_rsz0_frame_done | `<0x13>` |  |
| mdp_rsz1_frame_done | `<0x14>` |  |
| mdp_tdshp_frame_done | `<0x15>` |  |
| mdp_wdma_frame_done | `<0x16>` |  |
| mdp_wrot_write_frame_done | `<0x17>` |  |
| mdp_wrot_read_frame_done | `<0x18>` |  |
| disp_ovl0_frame_done | `<0x19>` |  |
| disp_rdma0_frame_done | `<0x1b>` |  |
| disp_rdma1_frame_done | `<0x1c>` |  |
| disp_wdma0_frame_done | `<0x1d>` |  |
| disp_ccorr_frame_done | `<0x1e>` |  |
| disp_color_frame_done | `<0x1f>` |  |
| disp_aal_frame_done | `<0x20>` |  |
| disp_gamma_frame_done | `<0x21>` |  |
| disp_dither_frame_done | `<0x22>` |  |
| disp_dpi0_frame_done | `<0x24>` |  |
| disp_dsi0_frame_done | `<0x25>` |  |
| stream_done_0 | `<0x26>` |  |
| stream_done_1 | `<0x27>` |  |
| stream_done_2 | `<0x28>` |  |
| stream_done_3 | `<0x29>` |  |
| stream_done_4 | `<0x2a>` |  |
| stream_done_5 | `<0x2b>` |  |
| stream_done_6 | `<0x2c>` |  |
| stream_done_7 | `<0x2d>` |  |
| stream_done_8 | `<0x2e>` |  |
| stream_done_9 | `<0x2f>` |  |
| buf_underrun_event_0 | `<0x30>` |  |
| buf_underrun_event_1 | `<0x31>` |  |
| dsi0_te_event | `<0x32>` |  |
| isp_frame_done_p2_1 | `<0x42>` |  |
| isp_frame_done_p2_0 | `<0x43>` |  |
| seninf_cam0_fifo_full | `<0x49>` |  |
| apxgpt2_count | `<0x10004028>` |  |

## `/bus/smi_larb0@14016000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,smi_larb0"]` |  |
| reg | `<0x14016000 0x1000>` |  |
| interrupts | `<0x0 0xb0 0x8>` |  |

## `/bus/smi_larb2@15001000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,smi_larb2"]` |  |
| reg | `<0x15001000 0x1000>` |  |
| interrupts | `<0x0 0xb2 0x8>` |  |

## `/bus/smi_common@14017000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,smi_common"]` |  |
| reg | `<0x14017000 0x1000 0x14016000 0x1000 0x16010000 0x1000 0x15001000 0x1000>` |  |

## `/bus/smi_larb1@16010000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,smi_larb1"]` |  |
| reg | `<0x16010000 0x10000>` |  |
| interrupts | `<0x0 0xb1 0x8>` |  |

## `/bus/cqdma@10217c00`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,cqdma"]` |  |
| reg | `<0x10217c00 0x400>` |  |
| interrupts | `<0x0 0x8a 0x8>` |  |
| nr_channel | `<0x1>` |  |

## `/bus/AP_CCIF1@0x10218000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,AP_CCIF1"]` |  |
| reg | `<0x10218000 0x1000>` |  |
| interrupts | `<0x0 0x8b 0x4>` |  |

## `/bus/MD_CCIF1@0x10219000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,MD_CCIF1"]` |  |
| reg | `<0x10219000 0x1000>` |  |

## `/bus/INFRA_MD@0x1021C000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,INFRA_MD"]` |  |
| reg | `<0x1021c000 0x1000>` |  |

## `/bus/DBGAPB@0x10400000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,DBGAPB"]` |  |
| reg | `<0x10400000 0xc00000>` |  |
| interrupts | `<0x0 0x84 0x8>` |  |

## `/bus/DEBUGTOP_CA7L@0x10800000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,DEBUGTOP_CA7L"]` |  |
| reg | `<0x10800000 0x400000>` |  |

## `/bus/DEBUGTOP_MD1@0x10450000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,DEBUGTOP_MD1"]` |  |
| reg | `<0x10450000 0x20000>` |  |

## `/bus/DEBUGTOP_MD2@0x10470000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,DEBUGTOP_MD2"]` |  |
| reg | `<0x10470000 0x10000>` |  |

## `/bus/CA9@0x10220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,CA9"]` |  |
| reg | `<0x10220000 0x8000>` |  |

## `/bus/cpu_dbgapb`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,hw_dbg"]` |  |
| num | `<0x4>` |  |
| reg | `<0x10810000 0x1000 0x10910000 0x1000 0x10a10000 0x1000 0x10b10000 0x1000>` |  |

## `/bus/dma@11000000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,ap_dma"]` |  |
| reg | `<0x11000000 0x1000>` |  |
| interrupts | `<0x0 0x61 0x8>` |  |

## `/bus/AP_DMA_IRDA@0x11000100`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,AP_DMA_IRDA"]` |  |
| reg | `<0x11000100 0x80>` |  |
| interrupts | `<0x0 0x62 0x8>` |  |

## `/bus/adc_hw@11001000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt6735-auxadc"]` |  |
| reg | `<0x11001000 0x1000>` |  |
| interrupts | `<0x0 0x4c 0x2>` |  |

## `/bus/adc_hw@11001000/adc_channel@`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek,adc_channel"]` |  |
| mediatek,temperature0 | `<0x0>` |  |
| mediatek,temperature1 | `<0x1>` |  |
| mediatek,adc_fdd_rf_params_dynamic_custom_ch | `<0xc>` |  |
| status | `["okay"]` |  |

## `/bus/PWM@0x11006000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,PWM"]` |  |
| reg | `<0x11006000 0x1000>` |  |
| interrupts | `<0x0 0x4d 0x8>` |  |

## `/bus/syscfg_pctl_a@0x10211000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt6735-pctl-a-syscfg", "syscon"]` |  |
| reg | `<0x10211000 0x1000>` |  |
| linux,phandle | `<0x33>` |  |
| phandle | `<0x33>` |  |

## `/bus/pinctrl@0x10211000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt6735-pinctrl"]` |  |
| reg | `<0x10211000 0x1000>` |  |
| mediatek,pctl-regmap | `<0x33>` |  |
| pins-are-numbered | `true (boolean)` |  |
| gpio-controller | `true (boolean)` |  |
| #gpio-cells | `<0x2>` |  |
| linux,phandle | `<0x9>` |  |
| phandle | `<0x9>` |  |

## `/bus/pinctrl@0x10211000/ssw0default`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x2e>` |  |
| phandle | `<0x2e>` |  |

## `/bus/pinctrl@0x10211000/ssw@1`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x2f>` |  |
| phandle | `<0x2f>` |  |

## `/bus/pinctrl@0x10211000/ssw@1/pins_cmd0_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x805>` |  |

## `/bus/pinctrl@0x10211000/ssw@1/pins_cmd1_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x904>` |  |

## `/bus/pinctrl@0x10211000/ssw@2`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x30>` |  |
| phandle | `<0x30>` |  |

## `/bus/pinctrl@0x10211000/ssw@2/pins_cmd0_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x802>` |  |

## `/bus/pinctrl@0x10211000/ssw@2/pins_cmd1_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x904>` |  |

## `/bus/pinctrl@0x10211000/ssw@3`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x31>` |  |
| phandle | `<0x31>` |  |

## `/bus/pinctrl@0x10211000/ssw@3/pins_cmd0_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0xa301>` |  |
| slew-rate | `<0x1>` |  |

## `/bus/pinctrl@0x10211000/ssw@3/pins_cmd1_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0xa401>` |  |
| slew-rate | `<0x1>` |  |

## `/bus/pinctrl@0x10211000/ssw@3/pins_cmd2_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0xa501>` |  |
| slew-rate | `<0x0>` |  |
| bias-pull-up | `<0x0>` |  |

## `/bus/pinctrl@0x10211000/ssw@3/pins_cmd3_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0xa001>` |  |
| slew-rate | `<0x1>` |  |

## `/bus/pinctrl@0x10211000/ssw@3/pins_cmd4_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0xa101>` |  |
| slew-rate | `<0x1>` |  |

## `/bus/pinctrl@0x10211000/ssw@3/pins_cmd5_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0xa201>` |  |
| slew-rate | `<0x0>` |  |
| bias-pull-up | `<0x0>` |  |

## `/bus/pinctrl@0x10211000/ssw@4`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x32>` |  |
| phandle | `<0x32>` |  |

## `/bus/pinctrl@0x10211000/ssw@4/pins_cmd0_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0xa304>` |  |

## `/bus/pinctrl@0x10211000/ssw@4/pins_cmd1_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0xa404>` |  |

## `/bus/pinctrl@0x10211000/ssw@4/pins_cmd2_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0xa504>` |  |

## `/bus/pinctrl@0x10211000/ssw@4/pins_cmd3_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0xa001>` |  |

## `/bus/pinctrl@0x10211000/ssw@4/pins_cmd4_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0xa101>` |  |

## `/bus/pinctrl@0x10211000/ssw@4/pins_cmd5_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0xa201>` |  |

## `/bus/pinctrl@0x10211000/vsram0default`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x29>` |  |
| phandle | `<0x29>` |  |

## `/bus/pinctrl@0x10211000/vsram@1`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x2a>` |  |
| phandle | `<0x2a>` |  |

## `/bus/pinctrl@0x10211000/vsram@1/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x8c00>` |  |
| slew-rate | `<0x1>` |  |
| output-low | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/vsram@2`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x2b>` |  |
| phandle | `<0x2b>` |  |

## `/bus/pinctrl@0x10211000/vsram@2/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x8c00>` |  |
| slew-rate | `<0x1>` |  |
| output-high | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/clockbuf@1`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x2c>` |  |
| phandle | `<0x2c>` |  |

## `/bus/pinctrl@0x10211000/clockbuf@1/pins_cmd0_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x6e01>` |  |

## `/bus/pinctrl@0x10211000/clockbuf@1/pins_cmd1_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x6f01>` |  |

## `/bus/pinctrl@0x10211000/clockbuf@1/pins_cmd2_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x7001>` |  |

## `/bus/pinctrl@0x10211000/clockbuf@1/pins_cmd3_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x7101>` |  |

## `/bus/pinctrl@0x10211000/clockbuf@1/pins_cmd4_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x7201>` |  |

## `/bus/pinctrl@0x10211000/clockbuf@2`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x2d>` |  |
| phandle | `<0x2d>` |  |

## `/bus/pinctrl@0x10211000/clockbuf@2/pins_cmd0_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x6e04>` |  |

## `/bus/pinctrl@0x10211000/clockbuf@2/pins_cmd1_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x6f04>` |  |

## `/bus/pinctrl@0x10211000/clockbuf@2/pins_cmd2_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x7004>` |  |

## `/bus/pinctrl@0x10211000/clockbuf@2/pins_cmd3_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x7104>` |  |

## `/bus/pinctrl@0x10211000/clockbuf@2/pins_cmd4_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x7204>` |  |

## `/bus/pinctrl@0x10211000/alspspincfg`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x5c>` |  |
| phandle | `<0x5c>` |  |

## `/bus/pinctrl@0x10211000/alspspincfg/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x3c00>` |  |
| slew-rate | `<0x0>` |  |
| bias-pull-up | `<0x0>` |  |

## `/bus/pinctrl@0x10211000/alspsdefaultcfg`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x5b>` |  |
| phandle | `<0x5b>` |  |

## `/bus/pinctrl@0x10211000/gyropincfg`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x5e>` |  |
| phandle | `<0x5e>` |  |

## `/bus/pinctrl@0x10211000/gyropincfg/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| slew-rate | `<0x0>` |  |
| bias-pull-down | `<0x0>` |  |

## `/bus/pinctrl@0x10211000/gyrodefaultcfg`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x5d>` |  |
| phandle | `<0x5d>` |  |

## `/bus/pinctrl@0x10211000/eint0default`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x35>` |  |
| phandle | `<0x35>` |  |

## `/bus/pinctrl@0x10211000/state_eint_as_int@state_eint_as_int`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x3b>` |  |
| phandle | `<0x3b>` |  |

## `/bus/pinctrl@0x10211000/state_eint_as_int@state_eint_as_int/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x600>` |  |
| slew-rate | `<0x0>` |  |
| bias-disable | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/audiodefault`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x49>` |  |
| phandle | `<0x49>` |  |

## `/bus/pinctrl@0x10211000/pmicclkmode0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x4a>` |  |
| phandle | `<0x4a>` |  |

## `/bus/pinctrl@0x10211000/pmicclkmode0/pins_cmd0_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x8f00>` |  |

## `/bus/pinctrl@0x10211000/pmicclkmode0/pins_cmd1_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x9000>` |  |

## `/bus/pinctrl@0x10211000/pmicclkmode0/pins_cmd2_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x9100>` |  |

## `/bus/pinctrl@0x10211000/pmicclkmode1`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x4b>` |  |
| phandle | `<0x4b>` |  |

## `/bus/pinctrl@0x10211000/pmicclkmode1/pins_cmd0_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x8f01>` |  |

## `/bus/pinctrl@0x10211000/pmicclkmode1/pins_cmd1_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x9001>` |  |

## `/bus/pinctrl@0x10211000/pmicclkmode1/pins_cmd2_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x9101>` |  |

## `/bus/pinctrl@0x10211000/amppullhigh`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x4c>` |  |
| phandle | `<0x4c>` |  |

## `/bus/pinctrl@0x10211000/amppullhigh/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x300>` |  |
| slew-rate | `<0x1>` |  |
| bias-disable | `true (boolean)` |  |
| output-high | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/amppulllow`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x4d>` |  |
| phandle | `<0x4d>` |  |

## `/bus/pinctrl@0x10211000/amppulllow/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x300>` |  |
| slew-rate | `<0x1>` |  |
| bias-disable | `true (boolean)` |  |
| output-low | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/eint@0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x36>` |  |
| phandle | `<0x36>` |  |

## `/bus/pinctrl@0x10211000/eint@0/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0xa00>` |  |
| slew-rate | `<0x0>` |  |
| bias-disable | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/eintoutput0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x37>` |  |
| phandle | `<0x37>` |  |

## `/bus/pinctrl@0x10211000/eintoutput0/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0xa00>` |  |
| slew-rate | `<0x1>` |  |
| output-low | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/eintoutput1`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x38>` |  |
| phandle | `<0x38>` |  |

## `/bus/pinctrl@0x10211000/eintoutput1/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0xa00>` |  |
| slew-rate | `<0x1>` |  |
| output-high | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/rstoutput0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x39>` |  |
| phandle | `<0x39>` |  |

## `/bus/pinctrl@0x10211000/rstoutput0/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x3e00>` |  |
| slew-rate | `<0x1>` |  |
| output-low | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/rstoutput1`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x3a>` |  |
| phandle | `<0x3a>` |  |

## `/bus/pinctrl@0x10211000/rstoutput1/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x3e00>` |  |
| slew-rate | `<0x1>` |  |
| output-high | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/default`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x25>` |  |
| phandle | `<0x25>` |  |

## `/bus/pinctrl@0x10211000/high@0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x5f>` |  |
| phandle | `<0x5f>` |  |

## `/bus/pinctrl@0x10211000/high@0/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x200>` |  |
| slew-rate | `<0x1>` |  |
| output-high | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/low@1`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x60>` |  |
| phandle | `<0x60>` |  |

## `/bus/pinctrl@0x10211000/low@1/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x200>` |  |
| slew-rate | `<0x1>` |  |
| output-low | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/en_high@0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x61>` |  |
| phandle | `<0x61>` |  |

## `/bus/pinctrl@0x10211000/en_high@0/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x200>` |  |
| slew-rate | `<0x1>` |  |
| output-high | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/en_low@1`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x62>` |  |
| phandle | `<0x62>` |  |

## `/bus/pinctrl@0x10211000/en_low@1/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x200>` |  |
| slew-rate | `<0x1>` |  |
| output-low | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/iset_high@0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x63>` |  |
| phandle | `<0x63>` |  |

## `/bus/pinctrl@0x10211000/iset_high@0/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| slew-rate | `<0x1>` |  |
| output-high | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/iset_low@1`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x64>` |  |
| phandle | `<0x64>` |  |

## `/bus/pinctrl@0x10211000/iset_low@1/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| slew-rate | `<0x1>` |  |
| output-low | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/cam0@0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x53>` |  |
| phandle | `<0x53>` |  |

## `/bus/pinctrl@0x10211000/cam0@0/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x2c00>` |  |
| slew-rate | `<0x1>` |  |
| output-low | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/cam0@1`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x54>` |  |
| phandle | `<0x54>` |  |

## `/bus/pinctrl@0x10211000/cam0@1/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x2c00>` |  |
| slew-rate | `<0x1>` |  |
| output-high | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/cam0@2`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x55>` |  |
| phandle | `<0x55>` |  |

## `/bus/pinctrl@0x10211000/cam0@2/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x700>` |  |
| slew-rate | `<0x1>` |  |
| output-low | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/cam0@3`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x56>` |  |
| phandle | `<0x56>` |  |

## `/bus/pinctrl@0x10211000/cam0@3/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x700>` |  |
| slew-rate | `<0x1>` |  |
| output-high | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/cam1@0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x57>` |  |
| phandle | `<0x57>` |  |

## `/bus/pinctrl@0x10211000/cam1@0/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x2c00>` |  |
| slew-rate | `<0x1>` |  |
| output-low | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/cam1@1`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x58>` |  |
| phandle | `<0x58>` |  |

## `/bus/pinctrl@0x10211000/cam1@1/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x2c00>` |  |
| slew-rate | `<0x1>` |  |
| output-high | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/cam1@2`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x59>` |  |
| phandle | `<0x59>` |  |

## `/bus/pinctrl@0x10211000/cam1@2/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0xc00>` |  |
| slew-rate | `<0x1>` |  |
| output-low | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/cam1@3`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x5a>` |  |
| phandle | `<0x5a>` |  |

## `/bus/pinctrl@0x10211000/cam1@3/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0xc00>` |  |
| slew-rate | `<0x1>` |  |
| output-high | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/camdefault`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x52>` |  |
| phandle | `<0x52>` |  |

## `/bus/pinctrl@0x10211000/en_high`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x65>` |  |
| phandle | `<0x65>` |  |

## `/bus/pinctrl@0x10211000/en_high/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x2a00>` |  |
| slew-rate | `<0x1>` |  |
| bias-disable | `true (boolean)` |  |
| output-high | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/en_low`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x66>` |  |
| phandle | `<0x66>` |  |

## `/bus/pinctrl@0x10211000/en_low/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x2a00>` |  |
| slew-rate | `<0x1>` |  |
| bias-disable | `true (boolean)` |  |
| output-low | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/mode_high`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x67>` |  |
| phandle | `<0x67>` |  |

## `/bus/pinctrl@0x10211000/mode_high/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x2b00>` |  |
| slew-rate | `<0x1>` |  |
| bias-disable | `true (boolean)` |  |
| output-high | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/mode_low`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x68>` |  |
| phandle | `<0x68>` |  |

## `/bus/pinctrl@0x10211000/mode_low/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x2b00>` |  |
| slew-rate | `<0x1>` |  |
| bias-disable | `true (boolean)` |  |
| output-low | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/gpslna@0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x26>` |  |
| phandle | `<0x26>` |  |

## `/bus/pinctrl@0x10211000/gpslna@0/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x4f00>` |  |
| slew-rate | `<0x0>` |  |
| bias-disable | `true (boolean)` |  |
| output-low | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/gpslna@1`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x27>` |  |
| phandle | `<0x27>` |  |

## `/bus/pinctrl@0x10211000/gpslna@1/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x4f00>` |  |
| slew-rate | `<0x1>` |  |
| output-high | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/gpslna@2`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x28>` |  |
| phandle | `<0x28>` |  |

## `/bus/pinctrl@0x10211000/gpslna@2/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x4f00>` |  |
| slew-rate | `<0x1>` |  |
| output-low | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/mmc0@default`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x2>` |  |
| phandle | `<0x2>` |  |

## `/bus/pinctrl@0x10211000/mmc0@default/pins_cmd`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| drive-strength | `[02]` |  |

## `/bus/pinctrl@0x10211000/mmc0@default/pins_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| drive-strength | `[02]` |  |

## `/bus/pinctrl@0x10211000/mmc0@default/pins_clk`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| drive-strength | `[02]` |  |

## `/bus/pinctrl@0x10211000/mmc0@default/pins_rst`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| drive-strength | `[02]` |  |

## `/bus/pinctrl@0x10211000/mmc0@default/pins_ds`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| drive-strength | `[02]` |  |

## `/bus/pinctrl@0x10211000/mmc0@register_default`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| dat0rddly | `[00]` |  |
| dat1rddly | `[00]` |  |
| dat2rddly | `[00]` |  |
| dat3rddly | `[00]` |  |
| dat4rddly | `[00]` |  |
| dat5rddly | `[00]` |  |
| dat6rddly | `[00]` |  |
| dat7rddly | `[00]` |  |
| datwrddly | `[00]` |  |
| cmdrrddly | `[00]` |  |
| cmdrddly | `[00]` |  |
| cmd_edge | `[01]` |  |
| rdata_edge | `[01]` |  |
| wdata_edge | `[01]` |  |
| ett-hs200-cells | `<0xc>` |  |
| ett-hs200-default | `<0xb0 0x380 0x0 0xb0 0x7c00 0x0 0xb4 0x38 0x1 0x4 0x2 0x0 0xf0 0x1f0000 0x7 0xf0 0x7c00000 0xb 0xb4 0x7 0x1 0xf0 0x1f 0xb 0x4 0x400 0x0 0xf8 0x1f000000 0x7 0xf0 0x1f00 0x9 0x4 0x4 0x0>` |  |
| ett-hs400-cells | `<0x8>` |  |
| ett-hs400-default | `<0xb0 0x380 0x0 0xb0 0x7c00 0x0 0x188 0x7c 0x2 0x188 0x1f000 0x10 0xb4 0x38 0x1 0x4 0x2 0x0 0xf0 0x1f0000 0x6 0xf0 0x7c00000 0x6>` |  |
| linux,phandle | `<0x3>` |  |
| phandle | `<0x3>` |  |

## `/bus/pinctrl@0x10211000/mmc1@default`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x4>` |  |
| phandle | `<0x4>` |  |

## `/bus/pinctrl@0x10211000/mmc1@default/pins_cmd`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| drive-strength | `[03]` |  |

## `/bus/pinctrl@0x10211000/mmc1@default/pins_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| drive-strength | `[03]` |  |

## `/bus/pinctrl@0x10211000/mmc1@default/pins_clk`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| drive-strength | `[03]` |  |

## `/bus/pinctrl@0x10211000/mmc1@sdr104`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x5>` |  |
| phandle | `<0x5>` |  |

## `/bus/pinctrl@0x10211000/mmc1@sdr104/pins_cmd`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| drive-strength | `[02]` |  |

## `/bus/pinctrl@0x10211000/mmc1@sdr104/pins_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| drive-strength | `[02]` |  |

## `/bus/pinctrl@0x10211000/mmc1@sdr104/pins_clk`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| drive-strength | `[03]` |  |

## `/bus/pinctrl@0x10211000/mmc1@sdr50`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x6>` |  |
| phandle | `<0x6>` |  |

## `/bus/pinctrl@0x10211000/mmc1@sdr50/pins_cmd`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| drive-strength | `[02]` |  |

## `/bus/pinctrl@0x10211000/mmc1@sdr50/pins_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| drive-strength | `[02]` |  |

## `/bus/pinctrl@0x10211000/mmc1@sdr50/pins_clk`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| drive-strength | `[03]` |  |

## `/bus/pinctrl@0x10211000/mmc1@ddr50`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x7>` |  |
| phandle | `<0x7>` |  |

## `/bus/pinctrl@0x10211000/mmc1@ddr50/pins_cmd`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| drive-strength | `[02]` |  |

## `/bus/pinctrl@0x10211000/mmc1@ddr50/pins_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| drive-strength | `[02]` |  |

## `/bus/pinctrl@0x10211000/mmc1@ddr50/pins_clk`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| drive-strength | `[03]` |  |

## `/bus/pinctrl@0x10211000/mmc1@register_default`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| dat0rddly | `[00]` |  |
| dat1rddly | `[00]` |  |
| dat2rddly | `[00]` |  |
| dat3rddly | `[00]` |  |
| datwrddly | `[00]` |  |
| cmdrrddly | `[00]` |  |
| cmdrddly | `[00]` |  |
| cmd_edge | `[01]` |  |
| rdata_edge | `[01]` |  |
| wdata_edge | `[01]` |  |
| linux,phandle | `<0x8>` |  |
| phandle | `<0x8>` |  |

## `/bus/pinctrl@0x10211000/mtknfc`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x3c>` |  |
| phandle | `<0x3c>` |  |

## `/bus/pinctrl@0x10211000/state_ven_high`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x3d>` |  |
| phandle | `<0x3d>` |  |

## `/bus/pinctrl@0x10211000/state_ven_high/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x400>` |  |
| slew-rate | `<0x1>` |  |
| output-high | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/state_ven_low`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x3e>` |  |
| phandle | `<0x3e>` |  |

## `/bus/pinctrl@0x10211000/state_ven_low/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x400>` |  |
| slew-rate | `<0x1>` |  |
| output-low | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/state_rst_high`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x3f>` |  |
| phandle | `<0x3f>` |  |

## `/bus/pinctrl@0x10211000/state_rst_high/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x300>` |  |
| slew-rate | `<0x1>` |  |
| output-high | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/state_rst_low`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x40>` |  |
| phandle | `<0x40>` |  |

## `/bus/pinctrl@0x10211000/state_rst_low/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x300>` |  |
| slew-rate | `<0x1>` |  |
| output-low | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/state_eint_high`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x41>` |  |
| phandle | `<0x41>` |  |

## `/bus/pinctrl@0x10211000/state_eint_high/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x100>` |  |
| slew-rate | `<0x1>` |  |
| output-high | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/state_eint_low`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x42>` |  |
| phandle | `<0x42>` |  |

## `/bus/pinctrl@0x10211000/state_eint_low/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x100>` |  |
| slew-rate | `<0x1>` |  |
| output-low | `true (boolean)` |  |

## `/bus/pinctrl@0x10211000/state_irq_init`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x43>` |  |
| phandle | `<0x43>` |  |

## `/bus/pinctrl@0x10211000/state_irq_init/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x200>` |  |
| slew-rate | `<0x0>` |  |
| bias-pull-down | `<0x0>` |  |

## `/bus/pinctrl@0x10211000/usb_default`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x44>` |  |
| phandle | `<0x44>` |  |

## `/bus/pinctrl@0x10211000/iddig_irq_init`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x45>` |  |
| phandle | `<0x45>` |  |

## `/bus/pinctrl@0x10211000/iddig_irq_init/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x1>` |  |
| slew-rate | `<0x0>` |  |
| bias-pull-up | `<0x0>` |  |

## `/bus/pinctrl@0x10211000/drvvbus_init`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x46>` |  |
| phandle | `<0x46>` |  |

## `/bus/pinctrl@0x10211000/drvvbus_init/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x200>` |  |
| slew-rate | `<0x1>` |  |
| bias-pull-down | `<0x0>` |  |

## `/bus/pinctrl@0x10211000/drvvbus_low`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x47>` |  |
| phandle | `<0x47>` |  |

## `/bus/pinctrl@0x10211000/drvvbus_low/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x200>` |  |
| slew-rate | `<0x1>` |  |
| output-low | `true (boolean)` |  |
| bias-pull-down | `<0x0>` |  |

## `/bus/pinctrl@0x10211000/drvvbus_high`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| linux,phandle | `<0x48>` |  |
| phandle | `<0x48>` |  |

## `/bus/pinctrl@0x10211000/drvvbus_high/pins_cmd_dat`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| pins | `<0x200>` |  |
| slew-rate | `<0x1>` |  |
| output-high | `true (boolean)` |  |
| bias-pull-down | `<0x0>` |  |

## `/bus/i2c@11007000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt6735m-i2c"]` |  |
| cell-index | `<0x0>` |  |
| reg | `<0x11007000 0x1000>` |  |
| interrupts | `<0x0 0x54 0x8 0x0 0x63 0x8>` |  |
| def_speed | `<0x64>` |  |
| #address-cells | `<0x1>` |  |
| #size-cells | `<0x0>` |  |

## `/bus/i2c@11007000/camera_main@10`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek,camera_main"]` |  |
| reg | `<0x10>` |  |
| status | `["okay"]` |  |

## `/bus/i2c@11007000/camera_main_af@0c`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek,camera_main_af"]` |  |
| reg | `<0xc>` |  |
| status | `["okay"]` |  |

## `/bus/i2c@11007000/camera_sub@3c`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek,camera_sub"]` |  |
| reg | `<0x3c>` |  |
| status | `["okay"]` |  |

## `/bus/i2c@11008000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt6735m-i2c"]` |  |
| cell-index | `<0x1>` |  |
| reg | `<0x11008000 0x1000>` |  |
| interrupts | `<0x0 0x55 0x8 0x0 0x64 0x8>` |  |
| def_speed | `<0x64>` |  |
| #address-cells | `<0x1>` |  |
| #size-cells | `<0x0>` |  |

## `/bus/i2c@11008000/cap_touch@5d`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek,cap_touch"]` |  |
| reg | `<0x5d>` |  |
| status | `["okay"]` |  |

## `/bus/i2c@11008000/i2c_lcd_bias@3e`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek,i2c_lcd_bias"]` |  |
| reg | `<0x3e>` |  |
| status | `["okay"]` |  |

## `/bus/i2c@11009000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt6735m-i2c"]` |  |
| cell-index | `<0x2>` |  |
| reg | `<0x11009000 0x1000>` |  |
| interrupts | `<0x0 0x56 0x8 0x0 0x65 0x8>` |  |
| def_speed | `<0x64>` |  |
| #address-cells | `<0x1>` |  |
| #size-cells | `<0x0>` |  |

## `/bus/i2c@11009000/msensor@0d`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek,msensor"]` |  |
| reg | `<0xd>` |  |
| status | `["okay"]` |  |

## `/bus/i2c@11009000/gyro@68`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek,gyro"]` |  |
| reg | `<0x68>` |  |
| status | `["okay"]` |  |

## `/bus/i2c@11009000/gsensor@4c`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek,gsensor"]` |  |
| reg | `<0x4c>` |  |
| status | `["okay"]` |  |

## `/bus/i2c@11009000/alsps@1e`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek,alsps"]` |  |
| reg | `<0x1e>` |  |
| status | `["okay"]` |  |

## `/bus/i2c@11009000/nfc@28`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek,nfc"]` |  |
| reg | `<0x28>` |  |
| status | `["okay"]` |  |

## `/bus/i2c@11009000/strobe_main@63`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek,strobe_main"]` |  |
| reg | `<0x63>` |  |
| status | `["okay"]` |  |

## `/bus/i2c@1100f000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt6735m-i2c"]` |  |
| cell-index | `<0x3>` |  |
| reg | `<0x1100f000 0x1000>` |  |
| interrupts | `<0x0 0x57 0x8 0x0 0x66 0x8>` |  |
| def_speed | `<0x64>` |  |
| #address-cells | `<0x1>` |  |
| #size-cells | `<0x0>` |  |

## `/bus/i2c@1100f000/ext_buck@6b`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek,ext_buck"]` |  |
| reg | `<0x6b>` |  |
| status | `["okay"]` |  |

## `/bus/G3D_CONFIG@0x13000000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,G3D_CONFIG"]` |  |
| reg | `<0x13000000 0x1000>` |  |

## `/bus/IMGSYS@0x15000000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,IMGSYS"]` |  |
| reg | `<0x15000000 0x1000>` |  |

## `/bus/SPI1@0x1100A000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| cell-index | `<0x0>` |  |
| spi-padmacro | `<0x0>` |  |
| compatible | `["mediatek,SPI1"]` |  |
| reg | `<0x1100a000 0x1000>` |  |
| interrupts | `<0x0 0x76 0x8>` |  |

## `/bus/touch@`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek,mt6735-touch", "mediatek,mt6735m-touch"]` |  |
| vtouch-supply | `<0x34>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vgp1 |
| interrupt-parent | `<0xa>` | /bus/eintc@10005000 |
| interrupts | `<0xa 0x2>` |  |
| debounce | `<0xa 0x0>` |  |
| status | `["okay"]` |  |
| tpd-resolution | `<0x1e0 0x356>` |  |
| use-tpd-button | `<0x1>` |  |
| tpd-key-num | `<0x3>` |  |
| tpd-key-local | `<0x8b 0x9e 0xac 0x0>` |  |
| tpd-key-dim-local | `<0x78 0x546 0x64 0x28 0x10e 0x546 0x64 0x28 0x258 0x546 0x64 0x28 0x0 0x0 0x0 0x0>` |  |
| tpd-max-touch-num | `<0x5>` |  |
| tpd-filter-enable | `<0x0>` |  |
| tpd-filter-pixel-density | `<0x7c>` |  |
| tpd-filter-custom-prameters | `<0x0 0x0 0x0 0x0 0x0 0x0 0x0 0x0 0x0 0x0 0x0 0x0>` |  |
| tpd-filter-custom-speed | `<0x0 0x0 0x0>` |  |
| pinctrl-names | `["default", "state_eint_as_int0", "state_eint_output0", "state_eint_output1", "state_rst_output0", "state_rst_output1"]` |  |
| pinctrl-0 | `<0x35>` | /bus/pinctrl@0x10211000/eint0default |
| pinctrl-1 | `<0x36>` | /bus/pinctrl@0x10211000/eint@0 |
| pinctrl-2 | `<0x37>` | /bus/pinctrl@0x10211000/eintoutput0 |
| pinctrl-3 | `<0x38>` | /bus/pinctrl@0x10211000/eintoutput1 |
| pinctrl-4 | `<0x39>` | /bus/pinctrl@0x10211000/rstoutput0 |
| pinctrl-5 | `<0x3a>` | /bus/pinctrl@0x10211000/rstoutput1 |

## `/bus/bl_fp@`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,BL229X"]` |  |

## `/bus/accdet@`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek,mt6735-accdet", "mediatek,mt6735m-accdet"]` |  |
| interrupt-parent | `<0xa>` | /bus/eintc@10005000 |
| interrupts | `<0x6 0x8>` |  |
| debounce | `<0x6 0x3e800>` |  |
| status | `["okay"]` |  |
| accdet-mic-vol | `<0x7>` |  |
| headset-mode-setting | `<0x500 0x200 0x1 0x1f0 0x800 0x800 0x20>` |  |
| accdet-plugout-debounce | `<0x14>` |  |
| accdet-mic-mode | `<0x1>` |  |
| headset-three-key-threshold | `<0x0 0x50 0xdc 0x1f4>` |  |
| headset-four-key-threshold | `<0x0 0x3a 0x79 0xc0 0x1c2>` |  |
| pinctrl-names | `["default", "state_eint_as_int"]` |  |
| pinctrl-0 | `<0x35>` | /bus/pinctrl@0x10211000/eint0default |
| pinctrl-1 | `<0x3b>` | /bus/pinctrl@0x10211000/state_eint_as_int@state_eint_as_int |

## `/bus/THERM_CTRL@0x1100B000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,THERM_CTRL"]` |  |
| reg | `<0x1100b000 0x1000>` |  |
| interrupts | `<0x0 0x4e 0x8>` |  |

## `/bus/ptp_fsm@1100b000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,ptp_fsm_v1"]` |  |
| reg | `<0x1100b000 0x1000>` |  |
| interrupts | `<0x0 0x7d 0x8>` |  |

## `/bus/AP_DMA_BTIF_TX@0x11000780`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,AP_DMA_BTIF_TX"]` |  |
| reg | `<0x11000780 0x80>` |  |
| interrupts | `<0x0 0x6f 0x8>` |  |

## `/bus/AP_DMA_BTIF_RX@0x11000800`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,AP_DMA_BTIF_RX"]` |  |
| reg | `<0x11000800 0x80>` |  |
| interrupts | `<0x0 0x70 0x8>` |  |

## `/bus/BTIF@0x1100C000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,BTIF"]` |  |
| reg | `<0x1100c000 0x1000>` |  |
| interrupts | `<0x0 0x5a 0x8>` |  |

## `/bus/nfc@0`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek,nfc-gpio-v2"]` |  |
| gpio-ven | `<0x4>` |  |
| gpio-rst | `<0x3>` |  |
| gpio-eint | `<0x1>` |  |
| gpio-irq | `<0x2>` |  |
| pinctrl-names | `["mtknfc", "ven_high", "ven_low", "rst_high", "rst_low", "eint_high", "eint_low", "irq_init"]` |  |
| pinctrl-0 | `<0x3c>` | /bus/pinctrl@0x10211000/mtknfc |
| pinctrl-1 | `<0x3d>` | /bus/pinctrl@0x10211000/state_ven_high |
| pinctrl-2 | `<0x3e>` | /bus/pinctrl@0x10211000/state_ven_low |
| pinctrl-3 | `<0x3f>` | /bus/pinctrl@0x10211000/state_rst_high |
| pinctrl-4 | `<0x40>` | /bus/pinctrl@0x10211000/state_rst_low |
| pinctrl-5 | `<0x41>` | /bus/pinctrl@0x10211000/state_eint_high |
| pinctrl-6 | `<0x42>` | /bus/pinctrl@0x10211000/state_eint_low |
| pinctrl-7 | `<0x43>` | /bus/pinctrl@0x10211000/state_irq_init |
| status | `["okay"]` |  |

## `/bus/gps`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,gps"]` |  |

## `/bus/btcvsd@10000000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,audio_bt_cvsd"]` |  |
| offset | `<0x700 0x800 0xfd0 0xfd4 0xfd8>` |  |
| reg | `<0x10000000 0x1000 0x18000000 0x10000 0x18080000 0x8000>` |  |
| interrupts | `<0x0 0xe4 0x8>` |  |

## `/bus/wifi@180F0000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,wifi"]` |  |
| reg | `<0x180f0000 0x5c>` |  |
| interrupts | `<0x0 0xe2 0x8>` |  |

## `/bus/NFI@0x1100D000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,NFI"]` |  |
| reg | `<0x1100d000 0x1000>` |  |
| interrupts | `<0x0 0x60 0x8>` |  |

## `/bus/DISP_PWM0@0x1100E000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,DISP_PWM0"]` |  |
| reg | `<0x1100e000 0x1000>` |  |

## `/bus/IRDA@0x11010000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,IRDA"]` |  |
| reg | `<0x11010000 0x1000>` |  |

## `/bus/usb20@11200000`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek,mt6735-usb20"]` |  |
| cell-index | `<0x0>` |  |
| reg | `<0x11200000 0x10000 0x11210000 0x10000>` |  |
| interrupts | `<0x0 0x48 0x8>` |  |
| mode | `<0x2>` |  |
| multipoint | `<0x1>` |  |
| dyn_fifo | `<0x1>` |  |
| soft_con | `<0x1>` |  |
| dma | `<0x1>` |  |
| num_eps | `<0x10>` |  |
| dma_channels | `<0x8>` |  |
| drvvbus_gpio | `<0x53 0x2>` |  |
| iddig_gpio | `<0x0 0x1>` |  |
| pinctrl-names | `["usb_default", "iddig_irq_init", "drvvbus_init", "drvvbus_low", "drvvbus_high"]` |  |
| pinctrl-0 | `<0x44>` | /bus/pinctrl@0x10211000/usb_default |
| pinctrl-1 | `<0x45>` | /bus/pinctrl@0x10211000/iddig_irq_init |
| pinctrl-2 | `<0x46>` | /bus/pinctrl@0x10211000/drvvbus_init |
| pinctrl-3 | `<0x47>` | /bus/pinctrl@0x10211000/drvvbus_low |
| pinctrl-4 | `<0x48>` | /bus/pinctrl@0x10211000/drvvbus_high |
| status | `["okay"]` |  |

## `/bus/audio@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,audio"]` |  |
| reg | `<0x11220000 0x10000>` |  |
| interrupts | `<0x0 0x90 0x8>` |  |

## `/bus/mt_soc_dl1_pcm@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt-soc-dl1-pcm"]` |  |
| reg | `<0x11220000 0x1000>` |  |
| interrupts | `<0x0 0x90 0x8>` |  |
| audclk-gpio | `<0x8f 0x0>` |  |
| audmiso-gpio | `<0x90 0x0>` |  |
| audmosi-gpio | `<0x91 0x0>` |  |
| vowclk-gpio | `<0x94 0x0>` |  |
| extspkamp-gpio | `<0x2 0x0>` |  |
| i2s1clk-gpio | `<0x50 0x0>` |  |
| i2s1dat-gpio | `<0x4e 0x0>` |  |
| i2s1mclk-gpio | `<0x9 0x0>` |  |
| i2s1ws-gpio | `<0x4f 0x0>` |  |
| pinctrl-names | `["default", "audpmicclk-mode0", "audpmicclk-mode1", "extamp-pullhigh", "extamp-pulllow"]` |  |
| pinctrl-0 | `<0x49>` | /bus/pinctrl@0x10211000/audiodefault |
| pinctrl-1 | `<0x4a>` | /bus/pinctrl@0x10211000/pmicclkmode0 |
| pinctrl-2 | `<0x4b>` | /bus/pinctrl@0x10211000/pmicclkmode1 |
| pinctrl-3 | `<0x4c>` | /bus/pinctrl@0x10211000/amppullhigh |
| pinctrl-4 | `<0x4d>` | /bus/pinctrl@0x10211000/amppulllow |

## `/bus/mt_soc_ul1_pcm@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_soc_pcm_capture"]` |  |

## `/bus/mt_soc_voice_md1@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_soc_pcm_voice_md1"]` |  |

## `/bus/mt_soc_hdmi_pcm@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_soc_pcm_hdmi"]` |  |

## `/bus/mt_soc_uldlloopback_pcm@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_soc_pcm_uldlloopback"]` |  |

## `/bus/mt_soc_i2s0_pcm@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_soc_pcm_dl1_i2s0"]` |  |

## `/bus/mt_soc_mrgrx_pcm@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_soc_pcm_mrgrx"]` |  |

## `/bus/mt_soc_mrgrx_awb_pcm@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_soc_pcm_mrgrx_awb"]` |  |

## `/bus/mt_soc_fm_i2s_pcm@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_soc_pcm_fm_i2s"]` |  |

## `/bus/mt_soc_fm_i2s_awb_pcm@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_soc_pcm_fm_i2s_awb"]` |  |

## `/bus/mt_soc_i2s0dl1_pcm@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_soc_pcm_dl1_i2s0Dl1"]` |  |

## `/bus/mt_soc_dl1_awb_pcm@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_soc_pcm_dl1_awb"]` |  |

## `/bus/mt_soc_voice_md1_bt@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_soc_pcm_voice_md1_bt"]` |  |

## `/bus/mt_soc_voip_bt_out@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_soc_pcm_dl1_bt"]` |  |

## `/bus/mt_soc_voip_bt_in@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_soc_pcm_bt_dai"]` |  |

## `/bus/mt_soc_tdmrx_pcm@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_soc_tdm_capture"]` |  |

## `/bus/mt_soc_fm_mrgtx_pcm@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_soc_pcm_fmtx"]` |  |

## `/bus/mt_soc_ul2_pcm@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_soc_pcm_capture2"]` |  |

## `/bus/mt_soc_i2s0_awb_pcm@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_soc_pcm_i2s0_awb"]` |  |

## `/bus/mt_soc_voice_md2@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_soc_pcm_voice_md2"]` |  |

## `/bus/mt_soc_routing_pcm@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_soc_pcm_routing"]` |  |
| i2s1clk-gpio | `<0x7 0x6>` |  |
| i2s1dat-gpio | `<0x5 0x6>` |  |
| i2s1mclk-gpio | `<0x9 0x6>` |  |
| i2s1ws-gpio | `<0x6 0x6>` |  |

## `/bus/mt_soc_voice_md2_bt@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_soc_pcm_voice_md2_bt"]` |  |

## `/bus/mt_soc_hp_impedance_pcm@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,Mt_soc_pcm_hp_impedance"]` |  |

## `/bus/mt_soc_codec_name@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_soc_codec_63xx"]` |  |

## `/bus/mt_soc_dummy_pcm@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_soc_pcm_dummy"]` |  |

## `/bus/mt_soc_codec_dummy_name@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_soc_codec_dummy"]` |  |

## `/bus/mt_soc_routing_dai_name@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_soc_dai_routing"]` |  |

## `/bus/mt_soc_dai_name@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_soc_dai_stub"]` |  |

## `/bus/mt_soc_offload_gdma@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_soc_pcm_offload_gdma"]` |  |

## `/bus/mt_soc_dl2_pcm@11220000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mt_soc_pcm_dl2"]` |  |

## `/bus/USB1@0x11260000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,USB1"]` |  |
| reg | `<0x11260000 0x10000>` |  |
| interrupts | `<0x0 0x49 0x8>` |  |

## `/bus/MSDC3@0x11260000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,MSDC3"]` |  |
| reg | `<0x11260000 0x10000>` |  |

## `/bus/WCN_AHB@0x11270000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,WCN_AHB"]` |  |
| reg | `<0x11270000 0x10000>` |  |
| interrupts | `<0x0 0xe4 0x8>` |  |

## `/bus/MDPERIPHERALS@0x20000000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,MD PERIPHERALS"]` |  |
| reg | `<0x20000000 0x0>` |  |

## `/bus/MD2PERIPHERALS@0x30000000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,MD2 PERIPHERALS"]` |  |
| reg | `<0x30000000 0x0>` |  |

## `/bus/C2KPERIPHERALS@0x38000000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,C2K PERIPHERALS"]` |  |
| reg | `<0x38000000 0x0>` |  |

## `/bus/MFGCFG@0x13000000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,MFGCFG"]` |  |
| reg | `<0x13000000 0x1000>` |  |
| interrupts | `<0x0 0xd2 0x8>` |  |

## `/bus/MALI@0x13040000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["arm,malit720", "arm,mali-t72x", "arm,malit7xx", "arm,mali-midgard"]` |  |
| reg | `<0x13040000 0x4000>` |  |
| interrupts | `<0x0 0xd4 0x8 0x0 0xd3 0x8 0x0 0xd2 0x8>` |  |
| interrupt-names | `["JOB", "MMU", "GPU"]` |  |
| clock-frequency | `<0x20c85580>` |  |

## `/bus/mmsys_config@14000000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mmsys_config"]` |  |
| reg | `<0x14000000 0x1000>` |  |
| interrupts | `<0x0 0xce 0x8>` |  |

## `/bus/mdp_rdma@14001000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mdp_rdma"]` |  |
| reg | `<0x14001000 0x1000>` |  |
| interrupts | `<0x0 0xbb 0x8>` |  |

## `/bus/mdp_rsz0@14002000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mdp_rsz0"]` |  |
| reg | `<0x14002000 0x1000>` |  |
| interrupts | `<0x0 0xbc 0x8>` |  |

## `/bus/mdp_rsz1@14003000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mdp_rsz1"]` |  |
| reg | `<0x14003000 0x1000>` |  |
| interrupts | `<0x0 0xbd 0x8>` |  |

## `/bus/mdp_wdma@14004000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mdp_wdma"]` |  |
| reg | `<0x14004000 0x1000>` |  |
| interrupts | `<0x0 0xbf 0x8>` |  |

## `/bus/mdp_wrot@14005000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mdp_wrot"]` |  |
| reg | `<0x14005000 0x1000>` |  |
| interrupts | `<0x0 0xc0 0x8>` |  |

## `/bus/mdp_tdshp@14006000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,mdp_tdshp"]` |  |
| reg | `<0x14006000 0x1000>` |  |
| interrupts | `<0x0 0xbe 0x8>` |  |

## `/bus/DISPSYS@14007000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,DISPSYS"]` |  |
| reg | `264 bytes; sha256=3aa4eb2b2558b125df5e7a6c744f274480296447a291d6d345ee53de57aa5b05 (large property omitted)` |  |
| interrupts | `336 bytes; sha256=968c6ae13e977311197c60298f1bd92397f883c59095e426c957ecd6e79217bc (large property omitted)` |  |

## `/bus/DISP_OVL0@0x14007000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,DISP_OVL0"]` |  |
| reg | `<0x14007000 0x1000>` |  |
| interrupts | `<0x0 0xc1 0x8>` |  |

## `/bus/DISP_OVL1@0x14008000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,DISP_OVL1"]` |  |
| reg | `<0x14008000 0x1000>` |  |
| interrupts | `<0x0 0xc2 0x8>` |  |

## `/bus/DISP_RDMA0@0x14009000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,DISP_RDMA0"]` |  |
| reg | `<0x14009000 0x1000>` |  |
| interrupts | `<0x0 0xc3 0x8>` |  |

## `/bus/DISP_RDMA1@0x1400A000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,DISP_RDMA1"]` |  |
| reg | `<0x1400a000 0x1000>` |  |
| interrupts | `<0x0 0xc4 0x8>` |  |

## `/bus/gpio@0x10000e00`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,fpga_gpio"]` |  |
| reg | `<0x10000e00 0x100>` |  |

## `/bus/DISP_WDMA0@0x1400B000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,DISP_WDMA0"]` |  |
| reg | `<0x1400b000 0x1000>` |  |
| interrupts | `<0x0 0xc5 0x8>` |  |

## `/bus/DISP_COLOR@0x1400C000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,DISP_COLOR"]` |  |
| reg | `<0x1400c000 0x1000>` |  |
| interrupts | `<0x0 0xc6 0x8>` |  |

## `/bus/DISP_CCORR@0x1400D000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,DISP_CCORR"]` |  |
| reg | `<0x1400d000 0x1000>` |  |
| interrupts | `<0x0 0xc7 0x8>` |  |

## `/bus/DISP_AAL@0x1400E000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,DISP_AAL"]` |  |
| reg | `<0x1400e000 0x1000>` |  |
| interrupts | `<0x0 0xc8 0x8>` |  |

## `/bus/DISP_GAMMA@0x1400F000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,DISP_GAMMA"]` |  |
| reg | `<0x1400f000 0x1000>` |  |
| interrupts | `<0x0 0xc9 0x8>` |  |

## `/bus/DISP_DITHER@0x14010000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,DISP_DITHER"]` |  |
| reg | `<0x14010000 0x1000>` |  |
| interrupts | `<0x0 0xca 0x8>` |  |

## `/bus/DISP_UFOE@0x14011000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,DISP_UFOE"]` |  |
| reg | `<0x14011000 0x1000>` |  |
| interrupts | `<0x0 0xcb 0x8>` |  |

## `/bus/DSI0@0x14012000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,DSI0"]` |  |
| reg | `<0x14012000 0x1000>` |  |
| interrupts | `<0x0 0xcc 0x8>` |  |

## `/bus/DPI0@0x14013000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,DPI0"]` |  |
| reg | `<0x14013000 0x1000>` |  |
| interrupts | `<0x0 0xcd 0x8>` |  |

## `/bus/DISP_PWM@0x14014000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,DISP_PWM"]` |  |
| reg | `<0x14014000 0x1000>` |  |

## `/bus/MM_MUTEX@0x14015000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,MM_MUTEX"]` |  |
| reg | `<0x14015000 0x1000>` |  |
| interrupts | `<0x0 0xba 0x8>` |  |

## `/bus/met_smi@14017000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,met_smi"]` |  |
| reg | `<0x14017000 0x1000 0x14016000 0x1000 0x16010000 0x1000 0x15001000 0x1000 0x17001000 0x1000>` |  |

## `/bus/MIPI_TX_CONFIG@0x14018000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,MIPI_TX_CONFIG"]` |  |
| reg | `<0x14018000 0x1000>` |  |

## `/bus/ISPSYS@0x15000000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,ISPSYS"]` |  |
| reg | `<0x15004000 0x9000 0x15000000 0x10000 0x10215000 0x1000>` |  |
| interrupts | `<0x0 0xb6 0x8 0x0 0xb7 0x8>` |  |

## `/bus/kd_camera_hw1@15008000`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek,camera_hw"]` |  |
| reg | `<0x15008000 0x1000>` |  |
| vcama-supply | `<0x4e>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vcama |
| vcamd-supply | `<0x4f>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vcamd |
| vcamaf-supply | `<0x50>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vcamaf |
| vcamio-supply | `<0x51>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vcamio |
| vcama_main2-supply | `<0x4e>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vcama |
| vcama_sub-supply | `<0x4e>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vcama |
| vcamaf_main2-supply | `<0x50>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vcamaf |
| vcamaf_sub-supply | `<0x50>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vcamaf |
| vcamd_main2-supply | `<0x4f>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vcamd |
| vcamd_sub-supply | `<0x4f>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vcamd |
| vcamio_main2-supply | `<0x51>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vcamio |
| vcamio_sub-supply | `<0x51>` | /soc/mt_pmic_regulator/ldo_regulators/ldo_vcamio |
| status | `["okay"]` |  |
| pinctrl-names | `["default", "cam0_rst0", "cam0_rst1", "cam0_pnd0", "cam0_pnd1", "cam1_rst0", "cam1_rst1", "cam1_pnd0", "cam1_pnd1"]` |  |
| pinctrl-0 | `<0x52>` | /bus/pinctrl@0x10211000/camdefault |
| pinctrl-1 | `<0x53>` | /bus/pinctrl@0x10211000/cam0@0 |
| pinctrl-2 | `<0x54>` | /bus/pinctrl@0x10211000/cam0@1 |
| pinctrl-3 | `<0x55>` | /bus/pinctrl@0x10211000/cam0@2 |
| pinctrl-4 | `<0x56>` | /bus/pinctrl@0x10211000/cam0@3 |
| pinctrl-5 | `<0x57>` | /bus/pinctrl@0x10211000/cam1@0 |
| pinctrl-6 | `<0x58>` | /bus/pinctrl@0x10211000/cam1@1 |
| pinctrl-7 | `<0x59>` | /bus/pinctrl@0x10211000/cam1@2 |
| pinctrl-8 | `<0x5a>` | /bus/pinctrl@0x10211000/cam1@3 |

## `/bus/kd_camera_hw2@15008000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,camera_hw2"]` |  |
| reg | `<0x15008000 0x1000>` |  |

## `/bus/ISP_SYSR@0x15000000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,ISP_SYSR"]` |  |

## `/bus/ISP_PIPEM@0x15000000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,ISP_PIPEM"]` |  |

## `/bus/SENINF_TOP@0x15008000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,SENINF_TOP"]` |  |
| reg | `<0x15008000 0x1000>` |  |
| interrupts | `<0x0 0xb6 0x8>` |  |

## `/bus/CAM@0x15004000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,CAM"]` |  |
| reg | `<0x15004000 0x1000>` |  |
| interrupts | `<0x0 0xb7 0x8>` |  |

## `/bus/VENC@0x15009000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,VENC"]` |  |
| reg | `<0x15009000 0x1000>` |  |
| interrupts | `<0x0 0xb4 0x8>` |  |

## `/bus/VDEC@0x1500B000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,VDEC"]` |  |
| reg | `<0x1500b000 0x1000>` |  |

## `/bus/JPGENC@0x1500A000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,JPGENC"]` |  |
| reg | `<0x1500a000 0x1000>` |  |
| interrupts | `<0x0 0xb5 0x8>` |  |

## `/bus/VDEC_GCON@0x16000000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,VDEC_GCON"]` |  |
| reg | `<0x16000000 0x1000>` |  |

## `/bus/VDEC_FULL_TOP@0x16020000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,VDEC_FULL_TOP"]` |  |
| reg | `<0x16020000 0x10000>` |  |
| interrupts | `<0x0 0xb3 0x8>` |  |

## `/bus/CHIPID@08000000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,CHIPID"]` |  |
| reg | `<0x8000000 0x4 0x8000004 0x4 0x8000008 0x4 0x800000c 0x4>` |  |

## `/bus/pwm@11006000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,pwm"]` |  |
| reg | `<0x11006000 0x1000>` |  |
| interrupts | `<0x0 0x4d 0x8>` |  |

## `/bus/MTKFB@5e200000`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,MTKFB"]` |  |
| reg | `<0x7f000000 0x1000000>` |  |

## `/lcm`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,lcm"]` |  |

## `/hwmsensor@0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,hwmsensor"]` |  |

## `/gsensor@0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,gsensor"]` |  |

## `/als_ps@0`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek,als_ps"]` |  |
| pinctrl-names | `["pin_default", "pin_cfg"]` |  |
| pinctrl-0 | `<0x5b>` | /bus/pinctrl@0x10211000/alspsdefaultcfg |
| pinctrl-1 | `<0x5c>` | /bus/pinctrl@0x10211000/alspspincfg |
| status | `["okay"]` |  |

## `/m_acc_pl@0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,m_acc_pl"]` |  |

## `/m_alsps_pl@0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,m_alsps_pl"]` |  |

## `/m_batch_pl@0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,m_batch_pl"]` |  |

## `/batchsensor@0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,batchsensor"]` |  |

## `/gyroscope@0`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek,gyroscope"]` |  |
| pinctrl-names | `["pin_default", "pin_cfg"]` |  |
| pinctrl-0 | `<0x5d>` | /bus/pinctrl@0x10211000/gyrodefaultcfg |
| pinctrl-1 | `<0x5e>` | /bus/pinctrl@0x10211000/gyropincfg |
| status | `["okay"]` |  |

## `/m_gyro_pl@0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,m_gyro_pl"]` |  |

## `/barometer@0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,barometer"]` |  |

## `/m_baro_pl@0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,m_baro_pl"]` |  |

## `/msensor@0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,msensor"]` |  |

## `/m_mag_pl@0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,m_mag_pl"]` |  |

## `/orientation@0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,orientation"]` |  |

## `/MOBICORE`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["trustonic,mobicore"]` |  |
| interrupts | `<0x0 0xf8 0x1>` |  |

## `/rf_clock_buffer`

| Property | Value | Resolved references |
|---|---|---|
| compatible | `["mediatek,rf_clock_buffer"]` |  |
| mediatek,clkbuf-quantity | `<0x4>` |  |
| mediatek,clkbuf-config | `<0x2 0x2 0x2 0x2>` |  |
| status | `["okay"]` |  |

## `/trusty`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["android,trusty-smc-v1"]` |  |
| ranges | `true (boolean)` |  |
| #address-cells | `<0x2>` |  |
| #size-cells | `<0x2>` |  |

## `/trusty/irq`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["android,trusty-irq-v1"]` |  |
| ppi-interrupt-parent | `<0x1>` |  |

## `/trusty/log`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["android,trusty-log-v1"]` |  |

## `/trusty/virtio`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["android,trusty-virtio-v1"]` |  |

## `/trusty/mtee`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,trusty-mtee-v1"]` |  |

## `/led@0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,red"]` |  |
| led_mode | `<0x0>` |  |
| data | `<0x1>` |  |
| pwm_config | `<0x0 0x0 0x0 0x0 0x0>` |  |

## `/led@1`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,green"]` |  |
| led_mode | `<0x0>` |  |
| data | `<0x1>` |  |
| pwm_config | `<0x0 0x0 0x0 0x0 0x0>` |  |

## `/led@2`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,blue"]` |  |
| led_mode | `<0x0>` |  |
| data | `<0x1>` |  |
| pwm_config | `<0x0 0x0 0x0 0x0 0x0>` |  |

## `/led@3`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,jogball-backlight"]` |  |
| led_mode | `<0x0>` |  |
| data | `<0x1>` |  |
| pwm_config | `<0x0 0x0 0x0 0x0 0x0>` |  |

## `/led@4`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,keyboard-backlight"]` |  |
| led_mode | `<0x0>` |  |
| data | `<0x1>` |  |
| pwm_config | `<0x0 0x0 0x0 0x0 0x0>` |  |

## `/led@5`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,button-backlight"]` |  |
| led_mode | `<0x0>` |  |
| data | `<0x1>` |  |
| pwm_config | `<0x0 0x0 0x0 0x0 0x0>` |  |

## `/led@6`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,lcd-backlight"]` |  |
| led_mode | `<0x5>` |  |
| data | `<0x1>` |  |
| pwm_config | `<0x0 0x0 0x0 0x0 0x0>` |  |

## `/aw3216`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,chg-aw3216"]` |  |
| pinctrl-names | `["default", "chg-en-high-aw3216", "chg-en-low-aw3216"]` |  |
| pinctrl-0 | `<0x25>` | /bus/pinctrl@0x10211000/default |
| pinctrl-1 | `<0x5f>` | /bus/pinctrl@0x10211000/high@0 |
| pinctrl-2 | `<0x60>` | /bus/pinctrl@0x10211000/low@1 |

## `/eta6195`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,chg-eta6195"]` |  |
| pinctrl-names | `["default", "chg-en-high", "chg-en-low", "chg-iset-high", "chg-iset-low"]` |  |
| pinctrl-0 | `<0x25>` | /bus/pinctrl@0x10211000/default |
| pinctrl-1 | `<0x61>` | /bus/pinctrl@0x10211000/en_high@0 |
| pinctrl-2 | `<0x62>` | /bus/pinctrl@0x10211000/en_low@1 |
| pinctrl-3 | `<0x63>` | /bus/pinctrl@0x10211000/iset_high@0 |
| pinctrl-4 | `<0x64>` | /bus/pinctrl@0x10211000/iset_low@1 |

## `/leddrvsgm3780`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,leddrv-sgm3780"]` |  |
| pinctrl-names | `["default", "flash-en-high", "flash-en-low", "flash-mode-high", "flash-mode-low"]` |  |
| pinctrl-0 | `<0x25>` | /bus/pinctrl@0x10211000/default |
| pinctrl-1 | `<0x65>` | /bus/pinctrl@0x10211000/en_high |
| pinctrl-2 | `<0x66>` | /bus/pinctrl@0x10211000/en_low |
| pinctrl-3 | `<0x67>` | /bus/pinctrl@0x10211000/mode_high |
| pinctrl-4 | `<0x68>` | /bus/pinctrl@0x10211000/mode_low |

## `/vibrator@0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,vibrator"]` |  |
| vib_timer | `<0x19>` |  |
| vib_limit | `<0x9>` |  |
| vib_vol | `<0x5>` |  |

## `/cust_accel@0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,kxtj2_1009"]` |  |
| i2c_num | `<0x2>` |  |
| i2c_addr | `<0xe 0x0 0x0 0x0>` |  |
| direction | `<0x0>` |  |
| power_id | `<0xffff>` |  |
| power_vol | `<0x0>` |  |
| firlen | `<0x0>` |  |
| is_batch_supported | `<0x0>` |  |

## `/cust_alsps@0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,AP3426"]` |  |
| i2c_num | `<0x2>` |  |
| i2c_addr | `<0x1e 0x0 0x0 0x0>` |  |
| polling_mode_ps | `<0x0>` |  |
| polling_mode_als | `<0x1>` |  |
| power_id | `<0xffff>` |  |
| power_vol | `<0x0>` |  |
| als_level | `<0x0 0xf 0x1e 0x32 0x91 0x12c 0x2bc 0x546 0x7d0 0xdac 0x1f40 0x2328 0x2710 0x2ee0 0x2ee0>` |  |
| als_value | `<0x0 0x14 0x32 0x64 0x96 0x12c 0x320 0x4b0 0x5dc 0x708 0x7d0 0x898 0x898 0x898 0x898 0x898>` |  |
| ps_threshold_high | `<0x12c>` |  |
| ps_threshold_low | `<0x64>` |  |
| is_batch_supported_ps | `<0x0>` |  |
| is_batch_supported_als | `<0x0>` |  |

## `/cust_mag@0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,akm09911"]` |  |
| i2c_num | `<0x2>` |  |
| i2c_addr | `<0xd 0x0 0x0 0x0>` |  |
| direction | `<0x1>` |  |
| power_id | `<0xffff>` |  |
| power_vol | `<0x0>` |  |
| is_batch_supported | `<0x0>` |  |

## `/cust_gyro@0`

| Property | Value | Resolved references |
|---|---|---|
| status | ABSENT (default enabled; check ancestors) | |
| compatible | `["mediatek,itg1010"]` |  |
| i2c_num | `<0x2>` |  |
| i2c_addr | `<0x68 0x0 0x0 0x0>` |  |
| direction | `<0x3>` |  |
| power_id | `<0xffff>` |  |
| power_vol | `<0x0>` |  |
| firlen | `<0x0>` |  |
| is_batch_supported | `<0x0>` |  |
