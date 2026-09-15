//! Read a region of target memory/flash and print it, so a write performed by
//! another code path (e.g. the browser/WebUSB harness) can be verified
//! out-of-band.
//!
//!   PROBE_RS_CHIP=nRF9160_xxAA PROBE_RS_PROTOCOL=swd \
//!   PROBE_RS_ADDR=0xF0000 PROBE_RS_LEN=64 cargo run --example e2e_dump

use probe_rs::{
    MemoryInterface, Permissions,
    config::TargetSelector,
    probe::{WireProtocol, list::Lister},
};

#[pollster::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chip = std::env::var("PROBE_RS_CHIP").unwrap_or_else(|_| "auto".into());
    let protocol = std::env::var("PROBE_RS_PROTOCOL").ok();
    let addr = u64::from_str_radix(
        std::env::var("PROBE_RS_ADDR")
            .unwrap_or_else(|_| "0".into())
            .trim_start_matches("0x"),
        16,
    )?;
    let len: usize = std::env::var("PROBE_RS_LEN")
        .unwrap_or_else(|_| "64".into())
        .parse()?;

    let lister = Lister::new();
    let probes = lister.list_all().await;
    let info = probes.first().ok_or("no probe found")?;
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

    let mut core = session.core(0).await?;
    core.halt(std::time::Duration::from_millis(500)).await?;

    let mut buf = vec![0u8; len];
    core.read_8(addr, &mut buf).await?;
    core.run().await.ok();

    println!("dump {len} bytes @ {addr:#010x}");
    for (i, chunk) in buf.chunks(16).enumerate() {
        let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
        let ascii: String = chunk
            .iter()
            .map(|&b| if (0x20..0x7f).contains(&b) { b as char } else { '.' })
            .collect();
        println!("{:08x}  {:<48}  {ascii}", addr as usize + i * 16, hex.join(" "));
    }
    Ok(())
}
