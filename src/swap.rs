//! Compressed swap in RAM (zram), so that a burst of memory use is paged out
//! instead of waking the kernel's OOM killer.
//!
//! The header is written here rather than with Alpine's `mkswap`, so that swap
//! does not depend on Alpine being installed.

/// The page size of this kernel, which the swap header is laid out for.
pub const PAGE_SIZE: usize = 4096;
/// The signature the kernel looks for at the end of the first page.
const SIGNATURE: &[u8] = b"SWAPSPACE2";
/// Where `struct swap_header_v1_2` starts inside the first page.
const INFO_OFFSET: usize = 1024;
/// Half of RAM: compressed at about 2:1 by lzo, a full device would use about a
/// quarter of RAM, and leave the rest for programs.
const RAM_SHARE_DIVISOR: u64 = 2;
/// Above every other swap area, which this system does not have anyway.
pub const PRIORITY: i32 = 100;

/// The zram device size for a `MemTotal:` value from `/proc/meminfo`, in bytes,
/// rounded down to whole pages.
pub fn device_size(meminfo: &str) -> Option<u64> {
    let kilobytes: u64 = meminfo
        .lines()
        .find_map(|line| line.strip_prefix("MemTotal:"))?
        .trim()
        .strip_suffix("kB")?
        .trim()
        .parse()
        .ok()?;
    let bytes = kilobytes * 1024 / RAM_SHARE_DIVISOR;
    let size = bytes - bytes % PAGE_SIZE as u64;
    (size > 0).then_some(size)
}

/// The first page of a swap area of `size` bytes, the same as `mkswap` writes,
/// without a UUID or label.
pub fn header(size: u64) -> Option<[u8; PAGE_SIZE]> {
    let pages = size / PAGE_SIZE as u64;
    // The kernel needs the header page and at least one page to swap to.
    if pages < 2 {
        return None;
    }
    let last_page = u32::try_from(pages - 1).ok()?;
    let mut page = [0u8; PAGE_SIZE];
    // version, last_page, nr_badpages, then the UUID and label, which stay zero.
    page[INFO_OFFSET..INFO_OFFSET + 4].copy_from_slice(&1u32.to_le_bytes());
    page[INFO_OFFSET + 4..INFO_OFFSET + 8].copy_from_slice(&last_page.to_le_bytes());
    page[PAGE_SIZE - SIGNATURE.len()..].copy_from_slice(SIGNATURE);
    Some(page)
}

/// The `swapon` flags that give the area `priority`, as in `<sys/swap.h>`.
pub fn swapon_flags(priority: i32) -> i32 {
    const SWAP_FLAG_PREFER: i32 = 0x8000;
    const SWAP_FLAG_PRIO_MASK: i32 = 0x7fff;
    SWAP_FLAG_PREFER | (priority & SWAP_FLAG_PRIO_MASK)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_size_is_half_of_ram_in_whole_pages() {
        // The value this phone reports.
        let meminfo = "MemTotal:         891436 kB\nMemFree:          815000 kB\n";
        let size = device_size(meminfo).unwrap();
        assert_eq!(size % PAGE_SIZE as u64, 0);
        assert!(size <= 891436 * 1024 / 2);
        assert!(891436 * 1024 / 2 - size < PAGE_SIZE as u64);
    }

    #[test]
    fn device_size_needs_mem_total() {
        assert_eq!(device_size("MemFree: 100 kB\n"), None);
        assert_eq!(device_size("MemTotal: lots\n"), None);
        assert_eq!(device_size("MemTotal: 0 kB\n"), None);
    }

    #[test]
    fn header_matches_mkswap() {
        // Busybox `mkswap` on a 448 MiB zram device on the phone printed a size of
        // 469757952 bytes: every page but the header.
        let size = 448 * 1024 * 1024;
        let page = header(size).unwrap();
        let word = |offset: usize| u32::from_le_bytes(page[offset..offset + 4].try_into().unwrap());
        assert_eq!(word(INFO_OFFSET), 1);
        assert_eq!(u64::from(word(INFO_OFFSET + 4)) * PAGE_SIZE as u64, 469757952);
        assert_eq!(word(INFO_OFFSET + 8), 0);
        assert_eq!(&page[PAGE_SIZE - 10..], b"SWAPSPACE2");
        // The boot sector area stays empty.
        assert!(page[..INFO_OFFSET].iter().all(|&byte| byte == 0));
    }

    #[test]
    fn header_refuses_a_device_too_small_to_swap_to() {
        assert!(header(PAGE_SIZE as u64).is_none());
        assert!(header(2 * PAGE_SIZE as u64).is_some());
    }

    #[test]
    fn flags_carry_the_priority() {
        assert_eq!(swapon_flags(PRIORITY), 0x8000 | 100);
        assert_eq!(swapon_flags(0), 0x8000);
    }
}
