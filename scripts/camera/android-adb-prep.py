# Prepares the phone's /data so stock Android (RAM-booted) starts adb and trusts
# this computer: persist.sys.usb.config=mtp,adb and /data/misc/adb/adb_keys, with
# the owners, modes and SELinux labels Android's init would give them.
# Run outside the chroot paths: it takes the outer /data as an argument.
# Usage: python3 android-adb-prep.py DATA_DIR ADBKEY_PUB
import os, sys

data, key_file = sys.argv[1], sys.argv[2]
SYSTEM, SHELL = 1000, 2000

def set_label(path, label):
    # Fails with EOPNOTSUPP when no SELinux policy is loaded (this system); Android
    # may then relabel the files itself at boot.
    try:
        os.setxattr(path, "security.selinux", label.encode() + b"\0")
    except OSError as error:
        print("no label on %s: %s" % (path, error.strerror))

def write(path, text, uid, gid, mode, label):
    with open(path, "w") as f:
        f.write(text)
    os.chown(path, uid, gid)
    os.chmod(path, mode)
    set_label(path, label)
    print("wrote", path)

def directory(path, uid, gid, mode, label):
    os.makedirs(path, exist_ok=True)
    os.chown(path, uid, gid)
    os.chmod(path, mode)
    set_label(path, label)

directory(os.path.join(data, "property"), 0, 0, 0o700, "u:object_r:property_data_file:s0")
write(os.path.join(data, "property", "persist.sys.usb.config"), "mtp,adb", 0, 0, 0o600,
      "u:object_r:property_data_file:s0")
directory(os.path.join(data, "misc"), SYSTEM, 9998, 0o1771, "u:object_r:system_data_file:s0")
directory(os.path.join(data, "misc", "adb"), SYSTEM, SHELL, 0o2750, "u:object_r:adb_keys_file:s0")
write(os.path.join(data, "misc", "adb", "adb_keys"), open(key_file).read(), SYSTEM, SHELL, 0o640,
      "u:object_r:adb_keys_file:s0")
