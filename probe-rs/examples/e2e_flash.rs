//! Stage 2: program flash through probe-rs and verify it.
//!
//! Writes a distinctive pattern into a spare, blank region at the top of the
//! ESP32-S3's 8 MB flash (0x7F0000), which is confirmed erased and unused, so the
//! board's existing application is never touched. `verify: true` makes probe-rs
//! read the programmed data back through the flash algorithm; `esptool` is then
//! used out-of-band as an independent check.
//!
//! Deliberately does NOT set `do_chip_erase` - that would wipe the running app.

use probe_rs::{
    Permissions,
    config::TargetSelector,
    flashing::{DownloadOptions, FlashLoader},
    probe::{WireProtocol, list::Lister},
};

const PATTERN_LEN: usize = 4096;

fn pattern(tag: &str) -> Vec<u8> {
    let mut marker = tag.as_bytes().to_vec();
    marker.resize(21, b'-');
    // Recognisable on a hexdump, and position-dependent so a misaligned or
    // truncated write cannot accidentally look correct.
    let mut v = Vec::with_capacity(PATTERN_LEN);
    while v.len() < PATTERN_LEN {
        let off = v.len();
        if off % 64 == 0 {
            v.extend_from_slice(&marker);
            v.extend_from_slice(format!("{off:06x}").as_bytes());
        } else {
            v.push((off % 251) as u8);
        }
    }
    v.truncate(PATTERN_LEN);
    v
}

#[pollster::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chip = std::env::var("PROBE_RS_CHIP").unwrap_or_else(|_| "auto".into());
    let protocol = std::env::var("PROBE_RS_PROTOCOL").ok();
    let tag = std::env::var("PROBE_RS_TAG").unwrap_or_else(|_| "NATIVE-FLASH-TEST".into());
    let target_addr = u64::from_str_radix(
        std::env::var("PROBE_RS_FADDR").unwrap_or_else(|_| "7F0000".into()).trim_start_matches("0x"),
        16,
    )?;
    let data = pattern(&tag);
    println!("pattern: {} bytes, first 32 = {:02x?}", data.len(), &data[..32]);

    let lister = Lister::new();
    let probes = lister.list_all().await;
    let info = probes.first().ok_or("no probe found")?;
    println!("probe: {}", info.identifier);

    let mut probe = info.open().await?;
    if let Some(p) = &protocol {
        let wp = match p.to_ascii_lowercase().as_str() {
            "swd" => WireProtocol::Swd,
            "jtag" => WireProtocol::Jtag,
            other => return Err(format!("unknown protocol {other}").into()),
        };
        probe.select_protocol(wp).await?;
    }
    let selector = if chip.eq_ignore_ascii_case("auto") {
        TargetSelector::Auto
    } else {
        TargetSelector::Unspecified(chip.clone())
    };
    let mut session = probe.attach(selector, Permissions::default()).await?;
    println!("target: {}", session.target().name);

    let mut loader: FlashLoader = session.target().flash_loader();
    loader.add_data(target_addr, &data)?;

    // DownloadOptions is #[non_exhaustive], so build it and set fields.
    let mut options = DownloadOptions::new();
    options.verify = true;
    // do_chip_erase stays false on purpose: only the touched sectors are erased.

    println!("programming {PATTERN_LEN} bytes at {target_addr:#08x} tag={tag:?} (verify=on)...");
    let t0 = std::time::Instant::now();
    loader.commit(&mut session, options).await?;
    println!("commit ok in {:?}", t0.elapsed());

    println!("STAGE2_FLASH_RESULT=PASS");
    Ok(())
}
