//! Baseline for the WebUSB end-to-end test: attach to a target, halt, read
//! memory, resume. Run natively first so any later browser failure is
//! attributable to the WebUSB transport rather than to the async refactor.
//!
//! Configurable so the same binary covers both boards under test:
//!   PROBE_RS_CHIP      target name, or "auto" (default)
//!   PROBE_RS_PROTOCOL  "swd" or "jtag"; omit to leave the probe's default
//!   PROBE_RS_ADDR      hex address to read, default 0x0
//!
//! e.g. ESP32-S3 over built-in USB-JTAG:  (defaults)
//!      nRF9160 via J-Link:  PROBE_RS_CHIP=nRF9160_xxAA PROBE_RS_PROTOCOL=swd

use std::time::Duration;

use probe_rs::{
    MemoryInterface, Permissions,
    config::TargetSelector,
    probe::{DebugProbeInfo, WireProtocol, list::Lister},
};

#[pollster::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let chip = std::env::var("PROBE_RS_CHIP").unwrap_or_else(|_| "auto".into());
    let protocol = std::env::var("PROBE_RS_PROTOCOL").ok();
    let addr = std::env::var("PROBE_RS_ADDR")
        .ok()
        .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
        .unwrap_or(0x0);

    let lister = Lister::new();
    let probes: Vec<DebugProbeInfo> = lister.list_all().await;

    println!("probes found: {}", probes.len());
    for (i, p) in probes.iter().enumerate() {
        println!(
            "  [{i}] {} {:04x}:{:04x} serial={:?}",
            p.identifier, p.vendor_id, p.product_id, p.serial_number
        );
    }

    let info = probes.first().ok_or("no probe found")?;
    println!("opening {}", info.identifier);
    let mut probe = info.open().await?;

    // The ESP USB-JTAG probe only supports JTAG and reports it already, so only
    // set the protocol when explicitly asked (needed for SWD parts via J-Link).
    if let Some(p) = &protocol {
        let wp = match p.to_ascii_lowercase().as_str() {
            "swd" => WireProtocol::Swd,
            "jtag" => WireProtocol::Jtag,
            other => return Err(format!("unknown protocol {other}").into()),
        };
        println!("selecting protocol {wp:?}");
        probe.select_protocol(wp).await?;
    }

    let selector = if chip.eq_ignore_ascii_case("auto") {
        TargetSelector::Auto
    } else {
        TargetSelector::Unspecified(chip.clone())
    };
    println!("attaching (target = {chip})...");
    let mut session = probe.attach(selector, Permissions::default()).await?;

    println!("target: {}", session.target().name);
    println!("cores:  {:?}", session.list_cores());

    let mut core = session.core(0).await?;

    let info = core.halt(Duration::from_millis(500)).await?;
    println!("halted, pc = {:#010x}", info.pc);

    let word = core.read_word_32(addr).await?;
    println!("read @{addr:#010x} = {word:#010x}");

    let status = core.status().await?;
    println!("core status: {status:?}");

    core.run().await?;
    println!("resumed");

    println!("STAGE1_RESULT=PASS");
    Ok(())
}
