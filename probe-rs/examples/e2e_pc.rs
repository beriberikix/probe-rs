//! Halt the first core and print PC plus the Cortex-M fault registers, to
//! diagnose a target that is "running" but silent.
use probe_rs::{MemoryInterface, Permissions, config::TargetSelector, probe::{WireProtocol, list::Lister}};

#[pollster::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();
    let chip = std::env::var("PROBE_RS_CHIP").unwrap_or_else(|_| "MCXA153".into());
    let probes = Lister::new().list_all().await;
    let mut probe = probes.first().ok_or("no probe")?.open().await?;
    probe.select_protocol(WireProtocol::Swd).await?;
    let mut session = probe.attach(TargetSelector::Unspecified(chip), Permissions::default()).await?;
    let mut core = session.core(0).await?;
    let status = core.status().await?;
    println!("status before halt: {status:?}");
    let info = core.halt(std::time::Duration::from_millis(500)).await?;
    println!("pc = {:#010x}", info.pc);
    for (name, addr) in [("DHCSR", 0xE000EDF0u64), ("DFSR", 0xE000ED30), ("CFSR", 0xE000ED28), ("HFSR", 0xE000ED2C), ("VTOR", 0xE000ED08), ("ICSR", 0xE000ED04)] {
        println!("{name} = {:#010x}", core.read_word_32(addr).await?);
    }
    let regs = core.registers();
    for r in regs.core_registers().take(16) {
        let v: u32 = core.read_core_reg(r.id()).await?;
        println!("{:>4} = {v:#010x}", r.name());
    }
    if std::env::var("PROBE_RS_DO_RESET").is_ok() {
        println!("reset()...");
        core.reset().await?;
        println!("reset ok; status {:?}", core.status().await?);
    } else {
        core.run().await?;
    }
    Ok(())
}
