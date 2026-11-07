#!/bin/sh
# Experiment: sample the camera supply rails while sensor-probe.py runs, to see
# whether the kernel powers the sensor on. Prints each distinct state once.
# Usage: sh rail-watch.sh [probe arguments]
R=/sys/class/regulator
sample() {
    for n in 4 14 17 12; do printf '%s=%s ' "$(cat $R/regulator.$n/name)" "$(cat $R/regulator.$n/state)"; done
    echo
}
(
    last=""
    end=$(( $(date +%s) + 25 ))
    while [ "$(date +%s)" -lt "$end" ]; do
        now=$(sample)
        [ "$now" != "$last" ] && echo "rails: $now" && last="$now"
    done
) &
watcher=$!
python3 /root/sensor-probe.py "$@"
sleep 1
kill $watcher 2>/dev/null
