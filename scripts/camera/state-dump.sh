#!/system/bin/sh
# Prints the hardware state that can differ between boots: GPIO pins, clock
# manager, regulators and camera-related proc files. Runs under this system's
# shell and under stock Android's adb shell, so the two can be diffed.
echo "## pins"
cat /sys/devices/virtual/misc/mtgpio/pin
for f in clk_stat mux_stat pll_stat subsys_stat; do
    echo "## clkmgr $f"
    cat /proc/clkmgr/$f 2>&1
done
echo "## regulators"
for r in /sys/class/regulator/regulator.*; do
    echo "$(cat $r/name) $(cat $r/state 2>&1) $(cat $r/microvolts 2>&1)"
done
echo "## camera_info"
cat /proc/driver/camera_info 2>&1
