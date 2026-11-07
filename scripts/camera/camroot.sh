#!/system/bin/sh
# Root helper for a RAM-booted stock Android test image only, never flashed.
# Runs /data/local/tmp/cmd.sh as root whenever adb (the shell user) drops one
# there, and leaves its output in /data/local/tmp/out.txt, ending with a
# "__done STATUS" line.
/system/bin/setenforce 0
while true; do
    if [ -f /data/local/tmp/cmd.sh ]; then
        mv /data/local/tmp/cmd.sh /data/local/tmp/cmd.run
        /system/bin/sh /data/local/tmp/cmd.run > /data/local/tmp/out.tmp 2>&1
        echo "__done $?" >> /data/local/tmp/out.tmp
        chmod 666 /data/local/tmp/out.tmp
        mv /data/local/tmp/out.tmp /data/local/tmp/out.txt
    fi
    sleep 1
done
