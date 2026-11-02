//! Read-only inspection of the running kernel through procfs.

use std::fs;

/// Returns the trimmed value of the first `key : value` line matching `key`.
pub fn field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        (name.trim() == key).then(|| value.trim())
    })
}

/// Builds a human-readable summary of the kernel, SoC and memory.
pub fn report() -> String {
    let read = |path: &str| fs::read_to_string(path).unwrap_or_default();
    let cpuinfo = read("/proc/cpuinfo");
    let meminfo = read("/proc/meminfo");
    let version = read("/proc/version");

    format!(
        "a32-init {}\narch:   {}\nkernel: {}\nsoc:    {}\nmemory: {}\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::ARCH,
        version.trim(),
        field(&cpuinfo, "Hardware").unwrap_or("unknown"),
        field(&meminfo, "MemTotal").unwrap_or("unknown"),
    )
}

#[cfg(test)]
mod tests {
    use super::field;

    const CPUINFO: &str = "Processor\t: ARMv7 Processor rev 4 (v7l)\n\
                           processor\t: 0\n\
                           Hardware\t: MT6737M\n";

    #[test]
    fn finds_tab_separated_field() {
        assert_eq!(field(CPUINFO, "Hardware"), Some("MT6737M"));
    }

    #[test]
    fn key_match_is_exact_and_case_sensitive() {
        assert_eq!(field(CPUINFO, "Processor"), Some("ARMv7 Processor rev 4 (v7l)"));
        assert_eq!(field(CPUINFO, "processor"), Some("0"));
        assert_eq!(field(CPUINFO, "Hard"), None);
    }

    #[test]
    fn value_may_contain_colons() {
        assert_eq!(field("key: a:b\n", "key"), Some("a:b"));
    }
}
