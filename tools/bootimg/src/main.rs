//! Host tool for inspecting and building boot images. It only reads and writes
//! local files and never talks to a device.

mod bootimg;
mod cpio;
mod sha1;

use bootimg::BootImage;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "usage:
  bootimg info IMAGE
  bootimg verify IMAGE              check that a rebuild is byte-identical
  bootimg unpack IMAGE DIR          write kernel, ramdisk.gz and trailer
  bootimg cpio-list ARCHIVE
  bootimg cpio-extract ARCHIVE DIR  extract regular files with safe names
  bootimg mkinitramfs INIT OUT      create a newc archive holding INIT as /init
  bootimg repack TEMPLATE RAMDISK_GZ OUT";

/// Size of the boot partition, from the stock OTA scatter (0x1d80000-0x2d80000).
const BOOT_PARTITION_BYTES: usize = 16 * 1024 * 1024;

/// Empty mount points required by the init process.
const INITRAMFS_DIRS: &[&str] = &["dev", "proc", "sys", "run"];

type Result<T = ()> = std::result::Result<T, String>;

fn read(path: &str) -> Result<Vec<u8>> {
    fs::read(path).map_err(|e| format!("{path}: {e}"))
}

fn write(path: impl AsRef<Path>, data: &[u8]) -> Result {
    let path = path.as_ref();
    fs::write(path, data).map_err(|e| format!("{}: {e}", path.display()))
}

fn load(path: &str) -> Result<BootImage> {
    BootImage::parse(&read(path)?)
}

fn info(path: &str) -> Result {
    let image = load(path)?;
    println!("name:        {}", image.text(48, 16));
    println!("cmdline:     {}", image.text(64, 512));
    println!("page size:   {}", image.page_size());
    println!("kernel:      {} bytes at {:#010x}", image.kernel.len(), image.field(12));
    println!("ramdisk:     {} bytes at {:#010x}", image.ramdisk.len(), image.field(20));
    println!("second:      {} bytes at {:#010x}", image.second.len(), image.field(28));
    println!("tags:        {:#010x}", image.field(32));
    println!("os version:  {:#010x}", image.field(44));
    println!("trailer:     {} bytes", image.trailer.len());
    println!("id matches:  {}", image.id_matches());
    Ok(())
}

fn verify(path: &str) -> Result {
    let original = read(path)?;
    if BootImage::parse(&original)?.build() != original {
        return Err("rebuild differs from the original".into());
    }
    println!("rebuild is byte-identical ({} bytes)", original.len());
    Ok(())
}

fn unpack(path: &str, dir: &str) -> Result {
    let image = load(path)?;
    fs::create_dir_all(dir).map_err(|e| format!("{dir}: {e}"))?;
    let dir = Path::new(dir);
    write(dir.join("kernel"), &image.kernel)?;
    write(dir.join("ramdisk.gz"), &image.ramdisk)?;
    write(dir.join("trailer"), &image.trailer)
}

fn cpio_list(path: &str) -> Result {
    for entry in cpio::parse(&read(path)?)? {
        let target = if entry.mode & cpio::S_IFMT == 0o120000 {
            format!(" -> {}", String::from_utf8_lossy(&entry.data))
        } else {
            String::new()
        };
        println!("{:06o} {:>8} {}{}", entry.mode, entry.data.len(), entry.name, target);
    }
    Ok(())
}

fn cpio_extract(path: &str, dir: &str) -> Result {
    let (mut written, mut skipped) = (0, 0);
    for entry in cpio::parse(&read(path)?)? {
        if !entry.is_regular() || !cpio::is_safe_name(&entry.name) {
            skipped += 1;
            continue;
        }
        let target = Path::new(dir).join(&entry.name);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        write(&target, &entry.data)?;
        written += 1;
    }
    println!("extracted {written} files, skipped {skipped} other entries");
    Ok(())
}

fn mkinitramfs(init: &str, out: &str) -> Result {
    let mut entries: Vec<_> = INITRAMFS_DIRS.iter().map(|d| cpio::Entry::dir(d)).collect();
    entries.push(cpio::Entry::file("init", 0o755, read(init)?));
    write(out, &cpio::build(&entries))
}

fn repack(template: &str, ramdisk: &str, out: &str) -> Result {
    let image = load(template)?.with_ramdisk(read(ramdisk)?).build();
    if image.len() > BOOT_PARTITION_BYTES {
        return Err(format!(
            "image is {} bytes, boot partition holds {BOOT_PARTITION_BYTES}",
            image.len()
        ));
    }
    write(out, &image)?;
    println!("wrote {out}: {} bytes", image.len());
    Ok(())
}

fn run(args: &[String]) -> Result {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["info", image] => info(image),
        ["verify", image] => verify(image),
        ["unpack", image, dir] => unpack(image, dir),
        ["cpio-list", archive] => cpio_list(archive),
        ["cpio-extract", archive, dir] => cpio_extract(archive, dir),
        ["mkinitramfs", init, out] => mkinitramfs(init, out),
        ["repack", template, ramdisk, out] => repack(template, ramdisk, out),
        _ => Err(USAGE.into()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
