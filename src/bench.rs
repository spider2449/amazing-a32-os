//! Repeatable performance measurements, run from the console.
//!
//! Each result is one line, `NAME key=value ...`, so that results from different
//! builds and devices can be compared. Only `bench write` writes to storage, to a
//! new file that it removes again.

use std::hint::black_box;
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const KERNEL_TIME: Duration = Duration::from_secs(3);
const CPU_ROOT: &str = "/sys/devices/system/cpu";
const THERMAL_ROOT: &str = "/sys/class/thermal";

const CRC_BUFFER: usize = 64 * 1024;
const MATRIX_SIZE: usize = 64;
const VECTOR_ITEMS: usize = 4 * 1024;
const MEMORY_BUFFER: usize = 64 * 1024 * 1024;
const LATENCY_SIZES: &[usize] =
    &[16 * 1024, 256 * 1024, 2 * 1024 * 1024, 16 * 1024 * 1024, 64 * 1024 * 1024];
const LATENCY_STEPS: u64 = 1 << 20;

// The kernel's thermal zones are disabled here, because the stock service that
// enables them does not run, so the load test has to stop by itself.
const CPU_LIMIT_MILLIDEGREES: i64 = 85_000;
const BATTERY_LIMIT_MILLIDEGREES: i64 = 45_000;
const LOAD_SAMPLE: Duration = Duration::from_secs(5);
const LOAD_CHUNK: usize = 16 * 1024;

const NET_BUFFER: usize = 64 * 1024;
const NET_DEFAULT_SECONDS: u64 = 10;

/// Runs one measurement named by the console arguments.
pub fn run(arguments: &[&str]) -> io::Result<String> {
    match arguments {
        ["cpu"] => Ok(cpu()),
        ["mem"] => Ok(memory()),
        #[cfg(target_os = "linux")]
        ["disk", path] => disk(path),
        #[cfg(target_os = "linux")]
        ["write", path] => disk_write(path),
        ["net", port] => net(number(port)? as u16, NET_DEFAULT_SECONDS),
        ["net", port, seconds] => net(number(port)? as u16, number(seconds)?),
        ["load", seconds] => Ok(load(Duration::from_secs(number(seconds)?))),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: bench cpu | mem | disk PATH | write NEW_FILE | net PORT [SECONDS] | load SECONDS",
        )),
    }
}

fn number(text: &str) -> io::Result<u64> {
    text.parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "bad number"))
}

/// Repeats `step`, which returns the work it did, and returns work per second.
fn rate(limit: Duration, mut step: impl FnMut() -> u64) -> f64 {
    let start = Instant::now();
    let mut work = 0;
    while start.elapsed() < limit {
        work += step();
    }
    work as f64 / start.elapsed().as_secs_f64()
}

/// A small generator for test data and random positions.
fn xorshift(state: &mut u32) -> u32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    *state
}

fn random_bytes(length: usize) -> Vec<u8> {
    let mut state = 0x6132_0001;
    (0..length).map(|_| xorshift(&mut state) as u8).collect()
}

/// Integer work: CRC-32 of a buffer that fits in the cache. Returns MB/s.
fn crc_rate(limit: Duration) -> f64 {
    let buffer = random_bytes(CRC_BUFFER);
    rate(limit, || {
        black_box(crate::dump::crc32(black_box(&buffer)));
        buffer.len() as u64
    }) / 1e6
}

/// Floating point work: multiplies two matrices. Returns million operations/s.
fn matrix_rate(limit: Duration) -> f64 {
    let size = MATRIX_SIZE;
    let a: Vec<f64> = (0..size * size).map(|index| (index % 7) as f64 + 0.5).collect();
    let b: Vec<f64> = (0..size * size).map(|index| (index % 5) as f64 - 1.5).collect();
    let mut c = vec![0.0f64; size * size];
    rate(limit, || {
        c.fill(0.0);
        for i in 0..size {
            for j in 0..size {
                let factor = a[i * size + j];
                for k in 0..size {
                    c[i * size + k] += factor * b[j * size + k];
                }
            }
        }
        black_box(&c);
        2 * (size * size * size) as u64
    }) / 1e6
}

/// Work a compiler can vectorize, to show what NEON adds. Returns MB/s.
fn vector_rate(limit: Duration) -> f64 {
    let a: Vec<u32> = (0..VECTOR_ITEMS as u32).map(|item| item.wrapping_mul(2_654_435_761)).collect();
    let b: Vec<u32> = (0..VECTOR_ITEMS as u32).map(|item| item ^ 0x5a5a_5a5a).collect();
    rate(limit, || {
        let sum = black_box(&a)
            .iter()
            .zip(black_box(&b))
            .fold(0u32, |sum, (x, y)| sum.wrapping_add(x.wrapping_mul(*y)));
        black_box(sum);
        (VECTOR_ITEMS * 2 * size_of::<u32>()) as u64
    }) / 1e6
}

/// Parses a kernel CPU list such as `0-3` or `0,2`.
fn count_cpus(list: &str) -> Option<usize> {
    let mut count = 0;
    for part in list.trim().split(',') {
        count += match part.split_once('-') {
            Some((first, last)) => {
                last.parse::<usize>().ok()?.checked_sub(first.parse::<usize>().ok()?)? + 1
            }
            None => {
                part.parse::<usize>().ok()?;
                1
            }
        };
    }
    Some(count)
}

fn read_cpu_file(name: &str) -> String {
    std::fs::read_to_string(format!("{CPU_ROOT}/{name}"))
        .map_or_else(|_| "?".into(), |text| text.trim().to_string())
}

/// All CPUs, online or not. The kernel starts the others when there is load.
fn present_cpus() -> usize {
    count_cpus(&read_cpu_file("present")).unwrap_or(1)
}

/// Runs a kernel on each thread, and reports the sum and the state during the run.
fn parallel(threads: usize, kernel: fn(Duration) -> f64) -> (f64, String, String) {
    thread::scope(|scope| {
        let workers: Vec<_> =
            (0..threads).map(|_| scope.spawn(move || kernel(KERNEL_TIME))).collect();
        // Sampled partway, because setting up the kernels takes a moment.
        thread::sleep(KERNEL_TIME / 2);
        let online = read_cpu_file("online");
        let frequency = read_cpu_file("cpu0/cpufreq/scaling_cur_freq");
        let total = workers.into_iter().filter_map(|worker| worker.join().ok()).sum();
        (total, online, frequency)
    })
}

fn cpu() -> String {
    let kernels: [(&str, &str, fn(Duration) -> f64); 3] = [
        ("cpu.crc", "MB/s", crc_rate),
        ("cpu.matrix", "Mflop/s", matrix_rate),
        ("cpu.vector", "MB/s", vector_rate),
    ];
    let mut report = String::new();
    for (name, unit, kernel) in kernels {
        for threads in [1, present_cpus()] {
            let (value, online, frequency) = parallel(threads, kernel);
            report += &format!(
                "{name} threads={threads} value={value:.1} unit={unit} online={online} cpu0_khz={frequency}\n"
            );
        }
    }
    report
}

/// A single cycle through all entries, so that the chase visits each one.
fn chase_table(entries: usize) -> Vec<u32> {
    let mut table: Vec<u32> = (0..entries as u32).collect();
    let mut state = 0x6132_0002;
    // Sattolo's algorithm, which unlike a plain shuffle gives one cycle.
    for index in (1..entries).rev() {
        table.swap(index, xorshift(&mut state) as usize % index);
    }
    table
}

fn latency_ns(table: &[u32], limit: Duration) -> f64 {
    let mut position = 0usize;
    let per_second = rate(limit, || {
        for _ in 0..LATENCY_STEPS {
            position = table[position] as usize;
        }
        black_box(position);
        LATENCY_STEPS
    });
    1e9 / per_second
}

fn memory() -> String {
    let source = random_bytes(MEMORY_BUFFER);
    let mut target = vec![0u8; MEMORY_BUFFER];
    let bytes = MEMORY_BUFFER as u64;
    let copy = rate(KERNEL_TIME, || {
        target.copy_from_slice(black_box(&source));
        black_box(&target);
        bytes
    }) / 1e6;
    let fill = rate(KERNEL_TIME, || {
        target.fill(black_box(0x5a));
        black_box(&target);
        bytes
    }) / 1e6;
    let read = rate(KERNEL_TIME, || {
        let sum = black_box(&source)
            .chunks_exact(4)
            .fold(0u32, |sum, word| sum.wrapping_add(u32::from(word[0])));
        black_box(sum);
        bytes
    }) / 1e6;
    drop((source, target));

    let mut report = format!(
        "mem.copy value={copy:.0} unit=MB/s\nmem.fill value={fill:.0} unit=MB/s\nmem.read value={read:.0} unit=MB/s\n"
    );
    for &size in LATENCY_SIZES {
        let table = chase_table(size / size_of::<u32>());
        let nanoseconds = latency_ns(&table, KERNEL_TIME);
        report += &format!("mem.latency size_kib={} value={nanoseconds:.1} unit=ns\n", size / 1024);
    }
    report
}

// The value on 32-bit ARM, the only supported target. It differs elsewhere.
#[cfg(target_os = "linux")]
const O_DIRECT: i32 = 0o200000;
#[cfg(target_os = "linux")]
const ALIGNMENT: usize = 4096;
#[cfg(target_os = "linux")]
const SEQUENTIAL_BLOCK: usize = 1024 * 1024;
#[cfg(target_os = "linux")]
const DISK_TIME: Duration = Duration::from_secs(5);
#[cfg(target_os = "linux")]
const WRITE_FILE_LIMIT: u64 = 256 * 1024 * 1024;

/// Writes a new file past the page cache, in sequence and at random, then removes
/// it. Each rate includes the final flush to the eMMC. It refuses an existing file.
#[cfg(target_os = "linux")]
fn disk_write(path: &str) -> io::Result<String> {
    use std::fs::OpenOptions;
    use std::os::unix::fs::{FileExt, OpenOptionsExt};

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .custom_flags(O_DIRECT)
        .open(path)?;
    let result = (|| {
        let data = random_bytes(SEQUENTIAL_BLOCK + ALIGNMENT);
        let skip = data.as_ptr().align_offset(ALIGNMENT);
        let block = &data[skip..skip + SEQUENTIAL_BLOCK];

        let mut failure = None;
        let mut size = 0u64;
        let start = Instant::now();
        while start.elapsed() < DISK_TIME && size < WRITE_FILE_LIMIT {
            if let Err(err) = file.write_all_at(block, size) {
                failure = Some(err);
                break;
            }
            size += SEQUENTIAL_BLOCK as u64;
        }
        file.sync_all()?;
        let sequential = size as f64 / start.elapsed().as_secs_f64() / 1e6;
        if let Some(err) = failure {
            return Err(err);
        }

        let pages = size / ALIGNMENT as u64;
        let mut state = 0x6132_0004;
        let mut writes = 0u64;
        let start = Instant::now();
        while start.elapsed() < DISK_TIME {
            let position = u64::from(xorshift(&mut state)) % pages * ALIGNMENT as u64;
            file.write_all_at(&block[..ALIGNMENT], position)?;
            writes += 1;
        }
        file.sync_all()?;
        let random = writes as f64 / start.elapsed().as_secs_f64();

        Ok(format!(
            "disk.write_sequential block_kib=1024 bytes={size} value={sequential:.1} unit=MB/s
disk.write_random block_kib=4 value={random:.0} unit=IOPS
"
        ))
    })();
    std::fs::remove_file(path)?;
    result
}

/// Reads a block device past the page cache, in sequence and at random.
#[cfg(target_os = "linux")]
fn disk(path: &str) -> io::Result<String> {
    use std::fs::OpenOptions;
    use std::io::{Seek, SeekFrom};
    use std::os::unix::fs::{FileExt, OpenOptionsExt};

    let mut file = OpenOptions::new().read(true).custom_flags(O_DIRECT).open(path)?;
    let size = file.seek(SeekFrom::End(0))?;
    if size < SEQUENTIAL_BLOCK as u64 {
        return Err(io::Error::other("device too small"));
    }

    let mut storage = vec![0u8; SEQUENTIAL_BLOCK + ALIGNMENT];
    let skip = storage.as_ptr().align_offset(ALIGNMENT);
    let block = &mut storage[skip..skip + SEQUENTIAL_BLOCK];

    let mut failure = None;
    let mut offset = 0;
    let sequential = rate(DISK_TIME, || {
        if let Err(err) = file.read_exact_at(block, offset) {
            failure.get_or_insert(err);
            return 0;
        }
        offset += SEQUENTIAL_BLOCK as u64;
        if offset + SEQUENTIAL_BLOCK as u64 > size {
            offset = 0;
        }
        SEQUENTIAL_BLOCK as u64
    }) / 1e6;

    let pages = size / ALIGNMENT as u64;
    let mut state = 0x6132_0003;
    let random = rate(DISK_TIME, || {
        let position = u64::from(xorshift(&mut state)) % pages * ALIGNMENT as u64;
        if let Err(err) = file.read_exact_at(&mut block[..ALIGNMENT], position) {
            failure.get_or_insert(err);
            return 0;
        }
        1
    });
    if let Some(err) = failure {
        return Err(err);
    }
    Ok(format!(
        "disk.read_sequential block_kib=1024 value={sequential:.1} unit=MB/s\ndisk.read_random block_kib=4 value={random:.0} unit=IOPS\n"
    ))
}

/// Serves one connection: the client sends until it closes its side, then this
/// sends for `seconds`. The port is only open during this call.
fn net(port: u16, seconds: u64) -> io::Result<String> {
    let mut stream = crate::transfer::accept_one(port)?;

    let mut buffer = vec![0u8; NET_BUFFER];
    let mut received = 0u64;
    let receive_start = Instant::now();
    loop {
        match stream.read(&mut buffer)? {
            0 => break,
            length => received += length as u64,
        }
    }
    let receive_seconds = receive_start.elapsed().as_secs_f64();

    let payload = random_bytes(NET_BUFFER);
    let limit = Duration::from_secs(seconds);
    let mut sent = 0u64;
    let send_start = Instant::now();
    while send_start.elapsed() < limit {
        stream.write_all(&payload)?;
        sent += payload.len() as u64;
    }
    let send_seconds = send_start.elapsed().as_secs_f64();
    stream.shutdown(std::net::Shutdown::Both)?;

    let megabits = |bytes: u64, time: f64| bytes as f64 * 8.0 / time / 1e6;
    Ok(format!(
        "net.receive bytes={received} value={:.1} unit=Mbit/s\nnet.send bytes={sent} value={:.1} unit=Mbit/s\n",
        megabits(received, receive_seconds),
        megabits(sent, send_seconds),
    ))
}

/// Finds a thermal zone by its type and returns millidegrees.
fn zone_temperature(wanted: &str) -> Option<i64> {
    for entry in std::fs::read_dir(THERMAL_ROOT).ok()?.flatten() {
        let path = entry.path();
        let name = std::fs::read_to_string(path.join("type")).unwrap_or_default();
        if name.trim() == wanted {
            return std::fs::read_to_string(path.join("temp")).ok()?.trim().parse().ok();
        }
    }
    None
}

/// Runs a load on all CPUs and records how the phone responds. It stops early at
/// a temperature limit, or if the temperatures cannot be read.
fn load(duration: Duration) -> String {
    let done = AtomicU64::new(0);
    let stop = AtomicBool::new(false);
    let mut report = String::new();
    thread::scope(|scope| {
        for _ in 0..present_cpus() {
            scope.spawn(|| {
                let buffer = random_bytes(LOAD_CHUNK);
                while !stop.load(Ordering::Relaxed) {
                    black_box(crate::dump::crc32(black_box(&buffer)));
                    done.fetch_add(LOAD_CHUNK as u64, Ordering::Relaxed);
                }
            });
        }

        let start = Instant::now();
        let mut last_done = 0;
        let mut last_time = start;
        let mut ended = "time";
        while start.elapsed() < duration {
            thread::sleep(LOAD_SAMPLE);
            let total = done.load(Ordering::Relaxed);
            let now = Instant::now();
            let speed = (total - last_done) as f64 / (now - last_time).as_secs_f64() / 1e6;
            (last_done, last_time) = (total, now);

            let cpu = zone_temperature("mtktscpu");
            let battery = zone_temperature("mtktsbattery");
            report += &format!(
                "load.sample seconds={} mb_s={speed:.1} online={} cpu0_khz={} cpu_mc={} battery_mc={}\n",
                start.elapsed().as_secs(),
                read_cpu_file("online"),
                read_cpu_file("cpu0/cpufreq/scaling_cur_freq"),
                cpu.map_or("?".into(), |value| value.to_string()),
                battery.map_or("?".into(), |value| value.to_string()),
            );
            match (cpu, battery) {
                (Some(cpu), Some(battery)) => {
                    if cpu >= CPU_LIMIT_MILLIDEGREES {
                        ended = "cpu_limit";
                        break;
                    }
                    if battery >= BATTERY_LIMIT_MILLIDEGREES {
                        ended = "battery_limit";
                        break;
                    }
                }
                _ => {
                    ended = "no_temperature";
                    break;
                }
            }
        }
        stop.store(true, Ordering::Relaxed);
        report += &format!("load.end reason={ended} seconds={}\n", start.elapsed().as_secs());
    });
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHORT: Duration = Duration::from_millis(20);

    #[test]
    fn kernels_report_positive_rates() {
        assert!(crc_rate(SHORT) > 0.0);
        assert!(matrix_rate(SHORT) > 0.0);
        assert!(vector_rate(SHORT) > 0.0);
    }

    #[test]
    fn counts_cpu_lists() {
        assert_eq!(count_cpus("0-3\n"), Some(4));
        assert_eq!(count_cpus("0"), Some(1));
        assert_eq!(count_cpus("0,2-3"), Some(3));
        assert_eq!(count_cpus("3-0"), None);
        assert_eq!(count_cpus("x"), None);
    }

    #[test]
    fn chase_table_is_one_cycle() {
        let entries = 1000;
        let table = chase_table(entries);
        let mut position = 0;
        for step in 1..=entries {
            position = table[position] as usize;
            assert_eq!(position == 0, step == entries);
        }
        assert!(latency_ns(&table, SHORT) > 0.0);
    }

    #[test]
    fn rejects_bad_arguments() {
        assert!(run(&[]).is_err());
        assert!(run(&["cpu", "extra"]).is_err());
        assert!(run(&["load", "soon"]).is_err());
        assert!(run(&["net", "x"]).is_err());
    }

    #[test]
    fn net_receives_then_sends() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let server = thread::spawn(move || net(port, 0));
        let mut client = loop {
            match std::net::TcpStream::connect(("127.0.0.1", port)) {
                Ok(client) => break client,
                Err(_) => thread::sleep(Duration::from_millis(10)),
            }
        };
        client.write_all(&[1u8; 1000]).unwrap();
        client.shutdown(std::net::Shutdown::Write).unwrap();
        let mut rest = Vec::new();
        client.read_to_end(&mut rest).unwrap();
        let report = server.join().unwrap().unwrap();
        assert!(report.contains("net.receive bytes=1000 "), "{report}");
    }
}
