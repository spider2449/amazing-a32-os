# Compatibility test plan — 2026-11-11

Baseline: feature/kernel-compat-audit, fd58def7ca90f30d320d9266c3e90be17adcb2cf.

1. Run the documented Windows workspace unit tests (cargo test-host --locked).
2. Check the ARM Linux target with cargo check --workspace --locked.
3. Attempt read-only SSH device discovery using the configured a32 host. If reachable, run scripts/compat-audit.sh through standard input inside Alpine; do not install packages, flash, or reboot.
4. Record outcomes and distinguish host/build checks from actual device capability evidence.

## Initial host results (before device connection)

- Windows `cargo test-host --locked`: exit 0; bootimg 8 passed and init 273 passed (281 total), zero failures/ignored. Includes compatibility parser/report tests. 23 dead-code warnings.
- ARM `cargo check --workspace --locked`: exit 0. This verifies compilation, not linking or execution on Linux 3.18.
- Read-only SSH attempt: exit 1, `banner exchange: Connection to UNKNOWN port -1: Connection refused`. No device audit executed; this does not establish a kernel compatibility failure.
- At this stage, ARM syscall probes, Alpine package compatibility and hardware runtime were unverified because the configured SSH route was unavailable. Later device results follow below.

## Initial phone verification (before RAM boot)

- Serial COM3 responded. Wi-Fi address is 192.168.1.106; the SSH alias still used 192.168.1.104. Connected directly with the saved a32 host key, without modifying SSH configuration.
- The initially running image did not recognize `compat`; the new ARM syscall probe was subsequently exercised after the RAM boot documented below.
- Device: Linux 3.18.35+, armv7l, Alpine 3.24.4, apk-tools 3.0.9-r0, armv7 repositories (v3.24 main/community).
- Read-only audit completed over SSH, exit 0. Initial stdin transmission produced a trailing newline error; retained original log, then normalized script newlines for the completed run.
- Present: mount namespace; cpu/cpuacct/freezer/debug cgroups; ext2/ext3/ext4/FUSE; registered tun and loop-control.
- Absent: user/pid/network namespaces; memory/pids cgroups; overlay/squashfs/f2fs. Device registration does not prove usable device nodes or functional operation.
- strace not installed. No packages installed, no flashing/reboot, no application-specific compatibility or modern syscall runtime claim.
- Evidence: 2026-11-11-phone-compat-serial.log, 2026-11-11-phone-compat-audit.log, 2026-11-11-phone-compat-audit-normalized.log in this directory.

## Image build
Run scripts/build-image.sh using Git Bash and the installed Windows ARM Rust toolchain; inspect image metadata and SHA-256. Build only.

Build completed: exit 0, release compilation 43.70s. target/a32-boot.img: 7417856 bytes; kernel 6756869 bytes; ramdisk 656177 bytes; image ID matches true. SHA256: 62A9AE768381D36A4FBF7161826FE29B904D90EF6EA8FF593D648E608730C738. Initial Git Bash invocation lacked coreutils PATH; corrected process PATH and ran successfully. Phone unchanged.

## Native compat command after RAM boot

fastboot boot completed with exit 0 (Sending OKAY, Booting OKAY). COM3 `compat` returned the full report and prompt; console script exit 0. Evidence: 2026-11-11-phone-compat-native.log.

ARM syscall probes: getrandom and memfd_create implemented (non-ENOSYS); copy_file_range, statx, io_uring_setup, pidfd_open, clone3 and openat2 returned ENOSYS. Non-ENOSYS proves implementation detection, not complete functional behavior. Namespace/cgroup/filesystem/device registration results agree with the earlier shell audit. This verifies the new compat command on the RAM-booted phone; no flash write performed.

## Additional phone functional plan
Run existing Alpine Python via serial, exercise getrandom and memfd read/write, inspect missing syscall errno with safe zero arguments, and check Python subprocess/thread/socketpair/TLS imports. No package installation or persistent device writes.

Functional result: Python 3.14.10 on armv7l completed all assertions and printed ALL FUNCTIONAL CHECKS PASSED. Verified getrandom 32 bytes; memfd write/seek/read/close; all six absent syscall ENOSYS results; socketpair exchange; thread completion; subprocess output; TLS context creation with OpenSSL 3.5.9 and 118 CA certificates. TLS network handshake not tested. Serial wrapper exit 0; child completion established by terminal success marker, not an independently captured child exit code. Evidence: 2026-11-11-phone-compat-functional.log.
