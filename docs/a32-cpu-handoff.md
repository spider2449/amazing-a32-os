# A32 ARM32 CPU handoff

[Source provenance](a32-boot-critical-evidence-index.md).
Stock DT declares four Cortex-A53 CPUs, reg 0..3, each `enable-method="spin-table"`
and `cpu-release-addr=<0 0x40000200>`, at 1.1 GHz. It **also** declares:

```text
/psci: compatible arm,psci; method smc
cpu_suspend 0x84000001; cpu_off 0x84000002
cpu_on 0x84000003; affinity_info 0x84000004
```

The simultaneous nodes make a spin-table-only interpretation unsafe.
The 4.9 mt6735m DT has the same broad CPU/PSCI structure, 1 GHz CPU metadata;
k37 changes metadata to 1.1 GHz. Neither metadata value is a measured clock.

## 4.9 selected ARM32 path

`S/arch/arm/kernel/setup.c:1117-1127` calls `arm_dt_init_cpu_maps()` and
`psci_dt_init()`. If `psci_smp_available()`, it installs `psci_smp_ops` in preference
to the machine descriptor's SMP pointer. `arch/arm/kernel/psci_smp.c:147-150`
tests whether `psci_ops.cpu_on` is non-null. `drivers/firmware/psci.c:663-665`
reads the legacy `cpu_on` property for `arm,psci`; `:679` selects this binding.
`arch/arm/kernel/psci_smp.c:60-63` calls CPU_ON with CPU logical map and the
physical ARM secondary entry. Current resolved CONFIG_ARM_PSCI and
CONFIG_ARM_PSCI_FW are enabled.

Thus **4.9 source predicts PSCI via SMC with stock legacy function IDs** when this
DT is passed intact. No runtime PSCI availability, secure monitor behavior or
post-LK DT has been captured. The function IDs being numerically like PSCI 0.2
does not justify changing the legacy compatible to `arm,psci-0.2`.

## Alternative vendor code is not proof of its selection

`S/drivers/misc/mediatek/base/power/mt6735/mt-smp.c:105+` has a holding pen,
SPM MTCMOS CPU1-3 power-on, IPI wakeup and timeout; `:269-296` switches cores
to AArch32, writes boot ROM boot address where applicable and sets boot address;
`:299-310` defines mt_smp_ops with hotplug hooks. Secure register writes use
mcusys SMC helpers. Its existence alone does not show the selected kernel links
or installs this table. `arch/arm/mach-mediatek/platsmp.c` registers other SoC
methods (mt6589 and mt81xx), not a proven A32 method. Do not invent
`mediatek,mt6735-smp` merely from a commented DT line.

Authentic 3.18 SMP source is unavailable. Stock DT / binary strings support the
platform family but cannot establish whether stock preferred PSCI or a patched
vendor path. LK/bootrom/secure-world assist remains UNKNOWN.

Recommendation for a future first DTS: retain all **four** stock CPUs and the
evidence-backed PSCI description pending handoff verification. There is no
evidence justifying disabling CPU1-3, nor enough to certify full SMP readiness.
Do not substitute ARM64 spin-table behavior for ARM32 setup. No CPU was disabled,
no DT was created and no SMC was issued. CPU readiness: **PARTIAL**.
