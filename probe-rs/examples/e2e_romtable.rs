//! Reproduce the `info` scan's first steps natively on the fork: attach to an
//! unspecified target, initialize the ARM interface, and parse AP0's ROM table.
use probe_rs::{
    architecture::arm::{
        FullyQualifiedApAddress, dp::DpAddress, memory::Component, sequences::DefaultArmSequence,
    },
    probe::{WireProtocol, list::Lister},
};

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
    probe.select_protocol(WireProtocol::Swd).await?;
    probe.attach_to_unspecified().await?;
    let iface = probe.try_into_arm_interface().map_err(|(_, e)| e)?;
    let mut iface = iface
        .initialize(DefaultArmSequence::create(), DpAddress::Default)
        .await
        .map_err(|(_, e)| e)?;
    println!("arm interface up");
    let aps = iface.access_ports(DpAddress::Default).await?;
    println!("{} APs", aps.len());
    let ap0 = FullyQualifiedApAddress::v1_with_default_dp(0);
    let mut mem = iface.memory_interface(&ap0).await?;
    let base = mem.base_address().await?;
    println!("ROM table base {base:#x}");
    let comp = Component::try_parse(&mut *mem, base).await?;
    println!("parsed: {comp:?}");
    Ok(())
}
