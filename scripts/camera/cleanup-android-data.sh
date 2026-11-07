#!/system/bin/sh
# One-off: returns /data to what it held before stock Android was RAM-booted on
# 2026-11-07 (checked against phone-tools/backup/data-2026-11-07/a32-backup.tar):
# a32, alpine, lost+found, misc/wifi/sockets and nvram/APCFG/APRDEB/{WIFI,WIFI_CUSTOM}.
# Everything else at those levels was created by Android and is removed.
set -e
cd /data
for entry in .[!.]* *; do
    case "$entry" in
        a32|alpine|lost+found|misc|nvram) ;;
        *) echo "remove /data/$entry"; rm -rf "/data/$entry" ;;
    esac
done
for entry in /data/misc/* /data/misc/.[!.]*; do
    [ -e "$entry" ] || continue
    [ "$entry" = /data/misc/wifi ] || { echo "remove $entry"; rm -rf "$entry"; }
done
for entry in /data/misc/wifi/* /data/misc/wifi/.[!.]*; do
    [ -e "$entry" ] || continue
    [ "$entry" = /data/misc/wifi/sockets ] || { echo "remove $entry"; rm -rf "$entry"; }
done
for entry in /data/nvram/* /data/nvram/.[!.]*; do
    [ -e "$entry" ] || continue
    [ "$entry" = /data/nvram/APCFG ] || { echo "remove $entry"; rm -rf "$entry"; }
done
for entry in /data/nvram/APCFG/*; do
    [ "$entry" = /data/nvram/APCFG/APRDEB ] || { echo "remove $entry"; rm -rf "$entry"; }
done
for entry in /data/nvram/APCFG/APRDEB/*; do
    case "$entry" in
        */WIFI|*/WIFI_CUSTOM) ;;
        *) echo "remove $entry"; rm -rf "$entry" ;;
    esac
done
# Owners and modes as in the backup (wifi is uid/gid 1010).
chown root:root /data /data/misc /data/nvram /data/nvram/APCFG /data/nvram/APCFG/APRDEB
chmod 755 /data /data/misc /data/nvram /data/nvram/APCFG /data/nvram/APCFG/APRDEB
chown 1010:1010 /data/misc/wifi /data/misc/wifi/sockets
chmod 770 /data/misc/wifi /data/misc/wifi/sockets
chown root:root /data/nvram/APCFG/APRDEB/WIFI /data/nvram/APCFG/APRDEB/WIFI_CUSTOM
chmod 660 /data/nvram/APCFG/APRDEB/WIFI /data/nvram/APCFG/APRDEB/WIFI_CUSTOM
echo "done"
