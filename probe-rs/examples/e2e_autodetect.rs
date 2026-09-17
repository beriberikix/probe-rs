//! Attach with `TargetSelector::Auto` and print what was detected.
use probe_rs::{Permissions, config::TargetSelector, probe::{WireProtocol, list::Lister}};

#[pollster::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();
    let probes = Lister::new().list_all().await;
    // PROBE_RS_PROBE=<vid:pid> (hex) picks a probe when several are attached.
    let want = std::env::var("PROBE_RS_PROBE").ok().map(|s| s.to_ascii_lowercase());
    let info = probes
        .iter()
        .find(|p| want.as_deref().is_none_or(|w| format!("{:04x}:{:04x}", p.vendor_id, p.product_id) == w))
        .ok_or("no matching probe")?;
    println!("probe: {} {:04x}:{:04x}", info.identifier, info.vendor_id, info.product_id);
    let mut probe = info.open().await?;
    let protocol = match std::env::var("PROBE_RS_PROTOCOL").as_deref() {
        Ok("jtag") => WireProtocol::Jtag,
        _ => WireProtocol::Swd,
    };
    probe.select_protocol(protocol).await?;
    let t0 = web_time::Instant::now();
    match probe.attach(TargetSelector::Auto, Permissions::default()).await {
        Ok(session) => println!("detected: {} in {:?}", session.target().name, t0.elapsed()),
        Err(e) => println!("auto-detect failed after {:?}: {e}", t0.elapsed()),
    }
    Ok(())
}
