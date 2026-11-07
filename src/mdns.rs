//! What `avahi-daemon` needs to answer for `a32.local` here.
//!
//! The init writes the configuration under /run and passes it with `-f`, so the
//! daemon works with Alpine's package as installed, and a reinstall of Alpine does
//! not lose it.

/// The configuration, at the same path inside and outside the chroot, since /run is
/// shared with it.
pub const CONFIG_PATH: &str = "/run/avahi-daemon.conf";

/// There is no D-Bus daemon here. IPv6 is off because the firewall drops all of it.
/// The SSH service files Alpine installs are still published.
pub const CONFIG: &str = "\
[server]
use-ipv4=yes
use-ipv6=no
enable-dbus=no
ratelimit-interval-usec=1000000
ratelimit-burst=1000

[publish]
publish-hinfo=no
publish-workstation=no
";

/// The Android kernel lets only this group open network sockets
/// (CONFIG_ANDROID_PARANOID_NETWORK). Root is exempt, but avahi drops to its
/// own user, and then got `socket() failed: Permission denied`.
pub const NETWORK_GROUP_ID: u32 = 3003;
const NETWORK_GROUP_NAME: &str = "inet";

/// `/etc/group` with `user` added to the group with the network group id, which is
/// created if missing. `None` when nothing needs to change.
pub fn with_network_member(group: &str, user: &str) -> Option<String> {
    let id = NETWORK_GROUP_ID.to_string();
    let mut found = false;
    let mut changed = false;
    let mut lines: Vec<String> = Vec::new();
    for line in group.lines() {
        let fields: Vec<&str> = line.splitn(4, ':').collect();
        if fields.len() == 4 && fields[2] == id {
            found = true;
            let members: Vec<&str> = fields[3].split(',').filter(|member| !member.is_empty()).collect();
            if !members.contains(&user) {
                let mut members = members;
                members.push(user);
                lines.push(format!("{}:{}:{}:{}", fields[0], fields[1], fields[2], members.join(",")));
                changed = true;
                continue;
            }
        }
        lines.push(line.to_string());
    }
    if !found {
        lines.push(format!("{NETWORK_GROUP_NAME}:x:{id}:{user}"));
        changed = true;
    }
    changed.then(|| {
        let mut text = lines.join("\n");
        text.push('\n');
        text
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const GROUP: &str = "root:x:0:root\navahi:x:86:\n";

    #[test]
    fn creates_the_group_when_missing() {
        assert_eq!(
            with_network_member(GROUP, "avahi").as_deref(),
            Some("root:x:0:root\navahi:x:86:\ninet:x:3003:avahi\n")
        );
    }

    #[test]
    fn adds_to_an_existing_group_of_any_name() {
        let group = "root:x:0:root\naid_inet:x:3003:sshd\n";
        assert_eq!(
            with_network_member(group, "avahi").as_deref(),
            Some("root:x:0:root\naid_inet:x:3003:sshd,avahi\n")
        );
        let empty = "inet:x:3003:\n";
        assert_eq!(with_network_member(empty, "avahi").as_deref(), Some("inet:x:3003:avahi\n"));
    }

    #[test]
    fn leaves_a_member_alone() {
        assert_eq!(with_network_member("inet:x:3003:avahi\n", "avahi"), None);
        assert_eq!(with_network_member("inet:x:3003:sshd,avahi\n", "avahi"), None);
    }

    #[test]
    fn does_not_mistake_a_member_name_for_the_id() {
        let group = "other:x:12:3003\n";
        let updated = with_network_member(group, "avahi").unwrap();
        assert_eq!(updated, "other:x:12:3003\ninet:x:3003:avahi\n");
    }

    #[test]
    fn config_turns_off_dbus() {
        assert!(CONFIG.contains("enable-dbus=no"));
    }
}
