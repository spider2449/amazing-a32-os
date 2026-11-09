//! Playing on the speaker and recording from the microphone, through two scripts
//! the init installs in Alpine: `a32-play` and `a32-record`.
//!
//! The stock kernel's audio driver panics the whole phone in two ways. Copying from
//! the capture buffer faults when the buffer is larger than the 32 KiB that is mapped
//! (ALSA allows 36864 bytes), and any mmap access to a capture buffer faults, which
//! includes ALSA's software conversion through `plug`. So the scripts open the
//! hardware devices directly, with read/write access and a small buffer, and the
//! PCM device nodes exist only while a script runs. Any other program that tries to
//! open them gets "no such file" instead of a kernel panic.

#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::io;
#[cfg(target_os = "linux")]
use std::os::unix::fs::PermissionsExt;

/// Where Alpine looks for local programs, from outside the chroot.
#[cfg(target_os = "linux")]
const BIN_DIR: &str = "/data/alpine/usr/local/bin";

/// Shared by both scripts. /dev is a tmpfs filled by hand, so nodes are made here.
const COMMON: &str = r##"#!/bin/sh
# Installed by the init at each start, so changes here are lost.
set -u
SND=/dev/snd
LOCK_DIR=/run
fail() { echo "$NAME: $1" >&2; exit 1; }
command -v aplay >/dev/null || fail "alsa-utils is missing: apk add alsa-utils"
make_node() {
    mkdir -p "$SND"
    [ -e "$SND/$1" ] && return 0
    number=$(cat "/sys/class/sound/$1/dev") || fail "no $1"
    mknod "$SND/$1" c "${number%:*}" "${number#*:}" || fail "mknod $1 failed"
}
mixer() { amixer -q -c 0 cset "name=$1" "$2" >/dev/null; }
"##;

const PLAY: &str = r##"NAME=a32-play
usage() {
    echo "usage: a32-play [-g DB] FILE.wav, or - for WAV on standard input" >&2
    echo "DB is the speaker gain: 0 (the default) or 4 to 12" >&2
    exit 2
}
GAIN=0
if [ "${1:-}" = -g ]; then
    [ $# -ge 2 ] || usage
    GAIN=$2
    shift 2
fi
[ $# -eq 1 ] || usage
# The mixer goes to 17 dB. Above 12 dB, only a weak signal was tried on this speaker.
case "$GAIN" in
    0|4|5|6|7|8|9|10|11|12) ;;
    *) usage ;;
esac
exec 9>"$LOCK_DIR/a32-play.lock"
flock -n 9 || fail "already playing"
STATUS=/proc/asound/card0/pcm0p/sub0/status
cleanup() {
    mixer Speaker_Amp_Switch Off
    rm -f "$SND/pcmC0D0p"
}
trap cleanup EXIT
trap 'exit 1' INT TERM
make_node controlC0
make_node pcmC0D0p
# A background job's standard input would otherwise be /dev/null.
exec 8<&0
aplay -q -D hw:0,0 --buffer-size=4096 --period-size=1024 "$1" <&8 &
player=$!
# The amplifier stays silent if it is switched on before playback runs.
tries=0
while [ $tries -lt 60 ] && kill -0 $player 2>/dev/null && ! grep -q RUNNING "$STATUS"; do
    sleep 0.05
    tries=$((tries + 1))
done
mixer Audio_Speaker_PGA_gain "${GAIN}Db"
mixer Speaker_Amp_Switch On
wait $player
"##;

const RECORD: &str = r##"NAME=a32-record
if [ $# -ne 2 ] || ! [ "$2" -gt 0 ] 2>/dev/null; then
    echo "usage: a32-record FILE.wav SECONDS, or - for WAV on standard output" >&2
    exit 2
fi
exec 9>"$LOCK_DIR/a32-record.lock"
flock -n 9 || fail "already recording"
cleanup() {
    mixer Audio_ADC_1_Switch Off
    mixer Audio_ADC_2_Switch Off
    rm -f "$SND/pcmC0D1c"
}
trap cleanup EXIT
trap 'exit 1' INT TERM
make_node controlC0
make_node pcmC0D1c
mixer Audio_Preamp1_Switch IN_ADC1
mixer Audio_ADC_1_Sel Preamp
mixer Audio_PGA1_Setting 18Db
mixer Audio_ADC_1_Switch On
mixer Audio_ADC_2_Switch On
# 48 kHz stereo only. The microphone is on the left channel; the right is silent.
arecord -q -D hw:0,1 -f S16_LE -r 48000 -c 2 --buffer-size=4096 --period-size=1024 -d "$2" "$1"
"##;

fn script(body: &str) -> String {
    format!("{COMMON}{body}")
}

/// Writes the scripts into Alpine, if it is installed.
#[cfg(target_os = "linux")]
pub fn install() -> io::Result<()> {
    if !crate::alpine::is_installed() {
        return Ok(());
    }
    fs::create_dir_all(BIN_DIR)?;
    for (name, body) in [("a32-play", PLAY), ("a32-record", RECORD)] {
        let path = format!("{BIN_DIR}/{name}");
        fs::write(&path, script(body))?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripts_start_as_shell_scripts_and_know_their_name() {
        assert!(script(PLAY).starts_with("#!/bin/sh\n"));
        assert!(script(PLAY).contains("NAME=a32-play\n"));
        assert!(script(RECORD).contains("NAME=a32-record\n"));
    }

    /// These limits keep the driver from panicking the kernel; see the module notes.
    #[test]
    fn devices_are_opened_directly_with_a_small_buffer() {
        for body in [PLAY, RECORD] {
            assert!(body.contains("--buffer-size=4096 --period-size=1024"));
            assert!(body.contains("-D hw:0,"));
            assert!(!body.contains("plughw"));
            assert!(!body.contains("--mmap"));
        }
    }

    #[test]
    fn device_nodes_are_removed_afterwards() {
        assert!(PLAY.contains("trap cleanup EXIT") && PLAY.contains("rm -f \"$SND/pcmC0D0p\""));
        assert!(RECORD.contains("trap cleanup EXIT") && RECORD.contains("rm -f \"$SND/pcmC0D1c\""));
    }

    #[test]
    fn the_gain_is_set_before_the_amplifier_and_only_to_values_the_mixer_has() {
        let gain = PLAY.find("mixer Audio_Speaker_PGA_gain").unwrap();
        let amplifier = PLAY.find("mixer Speaker_Amp_Switch On").unwrap();
        assert!(gain < amplifier);
        assert!(PLAY.contains("GAIN=0\n"));
        assert!(PLAY.contains("0|4|5|6|7|8|9|10|11|12)"));
    }

    #[test]
    fn the_amplifier_starts_after_playback_does() {
        let running = PLAY.find("grep -q RUNNING").unwrap();
        let amplifier = PLAY.find("mixer Speaker_Amp_Switch On").unwrap();
        assert!(running < amplifier);
    }
}
