#!/bin/sh
# Checks a32-play and a32-record on the phone: it plays a quiet 1 kHz tone on the
# speaker while recording, and compares the microphone before and during the tone.
# Audible. Pipe it to the phone: `ssh a32 sh < scripts/test-audio.sh`.
# Python is only used here, to make the tone and measure the recording.
: # A harmless first line for shells that are sent a byte order mark.
D=$(mktemp -d)
trap 'rm -rf "$D"' EXIT
FAILED=0
report() {
    if [ "$2" = ok ]; then echo "PASS  $1  $3"; else echo "FAIL  $1  $3"; FAILED=$((FAILED + 1)); fi
}

python3 - "$D/tone.wav" <<'EOF'
import math, struct, sys, wave
# 1 kHz stereo tone at -26 dBFS with short fades, 3 s at 48 kHz.
rate, secs, amp = 48000, 3, 0.05
count = rate * secs
with wave.open(sys.argv[1], "wb") as w:
    w.setnchannels(2); w.setsampwidth(2); w.setframerate(rate)
    out = bytearray()
    for i in range(count):
        fade = min(1.0, i / 2400, (count - i) / 2400)
        s = int(32767 * amp * fade * math.sin(2 * math.pi * 1000 * i / rate))
        out += struct.pack("<hh", s, s)
    w.writeframes(bytes(out))
EOF

UP_BEFORE=$(cut -d. -f1 /proc/uptime)
a32-record "$D/rec.wav" 5 &
RECORDER=$!
sleep 0.8
a32-play "$D/tone.wav"
report "a32-play exit" "$([ $? -eq 0 ] && echo ok)" ""
wait $RECORDER
report "a32-record exit" "$([ $? -eq 0 ] && echo ok)" ""

RESULT=$(python3 - "$D/rec.wav" <<'EOF'
import math, struct, sys, wave
# Prints the rise of 1 kHz energy on the left channel, in dB, and the frame count.
w = wave.open(sys.argv[1])
rate, ch = w.getframerate(), w.getnchannels()
raw = w.readframes(w.getnframes())
s = struct.unpack("<%dh" % (len(raw) // 2), raw)
def level(t0, t1):
    x = s[int(t0 * rate) * ch:int(t1 * rate) * ch:ch]
    mean = sum(x) / len(x)
    c = 2 * math.cos(2 * math.pi * 1000 / rate); a = b = 0.0
    for v in x:
        a, b = v - mean + c * a - b, a
    return 20 * math.log10(max(math.sqrt(max(a * a + b * b - c * a * b, 0)) * 2 / len(x), 1e-3))
print("%.1f %d %d %d %.1f" % (level(1.5, 2.8) - level(0.05, 0.45), w.getnframes(), rate, ch, level(1.5, 2.8)))
EOF
)
set -- $RESULT

# The same tone with 6 dB more gain.
a32-record "$D/gain.wav" 5 &
RECORDER=$!
sleep 0.8
a32-play -g 6 "$D/tone.wav"
wait $RECORDER
GAINED=$(python3 - "$D/gain.wav" <<'EOF'
import math, struct, sys, wave
# Prints the 1 kHz level on the left channel while the tone plays, in dB.
w = wave.open(sys.argv[1])
rate, ch = w.getframerate(), w.getnchannels()
raw = w.readframes(w.getnframes())
s = struct.unpack("<%dh" % (len(raw) // 2), raw)
x = s[int(1.5 * rate) * ch:int(2.8 * rate) * ch:ch]
mean = sum(x) / len(x)
c = 2 * math.cos(2 * math.pi * 1000 / rate); a = b = 0.0
for v in x:
    a, b = v - mean + c * a - b, a
print("%.1f" % (20 * math.log10(max(math.sqrt(max(a * a + b * b - c * a * b, 0)) * 2 / len(x), 1e-3))))
EOF
)
RISE=$(awk "BEGIN { printf \"%.1f\", $GAINED - $5 }")
report "gain of 6 dB" "$(awk "BEGIN { if ($RISE >= 4 && $RISE <= 8) print \"ok\" }")" "rise $RISE dB"
a32-play -g 13 "$D/tone.wav" 2>/dev/null
report "gain above 12 refused" "$([ $? -eq 2 ] && echo ok)" ""
report "recording format" "$([ "$3" = 48000 ] && [ "$4" = 2 ] && [ "$2" -ge 235000 ] && echo ok)" "$3 Hz, $4 ch, $2 frames"
report "tone heard by the microphone" "$(awk "BEGIN { if ($1 >= 20) print \"ok\" }")" "rise $1 dB"

cat "$D/tone.wav" | a32-play -
report "a32-play from standard input" "$([ $? -eq 0 ] && echo ok)" ""
a32-record - 1 > "$D/out.wav"
report "a32-record to standard output" "$([ $? -eq 0 ] && [ "$(wc -c < "$D/out.wav")" -ge 190000 ] && echo ok)" "$(wc -c < "$D/out.wav") bytes"

report "no PCM node left" "$([ -z "$(ls /dev/snd 2>/dev/null | grep pcm)" ] && echo ok)" "$(ls /dev/snd | tr '\n' ' ')"
OUT=$(arecord -q -D hw:0,1 -f S16_LE -r 48000 -c 2 -d 1 "$D/x.wav" 2>&1)
report "direct capture refused" "$(echo "$OUT" | grep -q 'No such file' && echo ok)" "$OUT"
for c in Speaker_Amp_Switch Audio_ADC_1_Switch; do
    report "$c off" "$(amixer -c 0 cget "name=$c" | grep -q ': values=off\|: values=0' && echo ok)" ""
done
UP_AFTER=$(cut -d. -f1 /proc/uptime)
report "no restart" "$([ "$UP_AFTER" -ge "$UP_BEFORE" ] && echo ok)" "uptime $UP_AFTER"
echo "failed checks: $FAILED"
exit $FAILED
