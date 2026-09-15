//! Raw ARM debug-port identification, for targets probe-rs has no chip
//! definition for (e.g. NXP MCX W7x).
//!
//! Attaches to an unspecified target, enumerates access ports and reads the
//! Cortex-M CPUID, which is enough to prove the probe driver and the SWD wire
//! work end to end. Strictly read-only.

use probe_rs::{
    MemoryInterface,
    architecture::arm::{dp::DpAddress, sequences::DefaultArmSequence},
    probe::{WireProtocol, list::Lister},
};

#[pollster::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let lister = Lister::new();
    let probes = lister.list_all().await;

    println!("probes found: {}", probes.len());
    for (i, p) in probes.iter().enumerate() {
        println!(
            "  [{i}] {} {:04x}:{:04x} serial={:?}",
            p.identifier, p.vendor_id, p.product_id, p.serial_number
        );
    }
    let info = probes.first().ok_or("no probe found")?;

    let mut probe = info.open().await?;
    probe.select_protocol(WireProtocol::Swd).await?;
    println!("protocol: Swd");

    probe.attach_to_unspecified().await?;
    let iface = probe
        .try_into_arm_interface()
        .map_err(|_| "probe has no ARM interface")?;

    let mut iface = iface
        .initialize(DefaultArmSequence::create(), DpAddress::Default)
        .await
        .map_err(|(_probe, e)| e)?;
    println!("ARM DP initialized");

    let aps = iface.access_ports(DpAddress::Default).await?;
    println!("access ports: {}", aps.len());
    for ap in &aps {
        println!("  {ap:x?}");
    }

    // Cortex-M CPUID lives at 0xE000ED00 in the debug memory map.
    let ap = aps.first().ok_or("no access ports found")?;
    let mut mem = iface.memory_interface(ap).await?;
    let cpuid = mem.read_word_32(0xE000_ED00).await?;
    println!("CPUID @0xE000ED00 = {cpuid:#010x}");

    let partno = (cpuid >> 4) & 0xFFF;
    let implementer = (cpuid >> 24) & 0xFF;
    let core = match partno {
        0xC20 => "Cortex-M0",
        0xC60 => "Cortex-M0+",
        0xC23 => "Cortex-M3",
        0xC24 => "Cortex-M4",
        0xC27 => "Cortex-M7",
        0xD20 => "Cortex-M23",
        0xD21 => "Cortex-M33",
        _ => "unknown",
    };
    println!("  implementer = {implementer:#04x} ({})", if implementer == 0x41 { "ARM" } else { "?" });
    println!("  part number = {partno:#05x} => {core}");

    // CoreSight ROM table identity: PIDR0/1 give the part number, PIDR1/2 the
    // JEP106 designer. These are SoC-specific and distinguish parts that share
    // a core (e.g. MCX W7x vs RW612, both Cortex-M33).
    let base = mem.read_word_32(0xE00F_EFF8).await.ok();
    println!("ROM table (via SCS-adjacent base) probe: {base:?}");

    let mut pidr = [0u32; 8];
    for (i, off) in [0xFE0u64, 0xFE4, 0xFE8, 0xFEC, 0xFD0, 0xFD4, 0xFD8, 0xFDC]
        .iter()
        .enumerate()
    {
        pidr[i] = mem.read_word_32(0xE00F_E000 + off).await.unwrap_or(0);
    }
    let part = (pidr[0] & 0xFF) | ((pidr[1] & 0x0F) << 8);
    let jep_id = ((pidr[1] >> 4) & 0x0F) | ((pidr[2] & 0x07) << 4);
    let jep_used = (pidr[2] >> 3) & 1;
    let jep_cont = pidr[4] & 0x0F;
    println!("ROM PIDR = {pidr:08x?}");
    println!("  part number = {part:#05x}");
    println!("  JEP106 = cont {jep_cont:#x} id {jep_id:#04x} (used={jep_used})");

    println!("DAP_RESULT=PASS");
    Ok(())
}
