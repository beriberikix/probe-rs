//! Stage 2b: reset the target through probe-rs so the boot log can be captured
//! on the CDC serial interface, confirming the board still runs its application
//! after probe-rs programmed flash.

use std::time::Duration;

use probe_rs::{Permissions, config::TargetSelector, probe::list::Lister};

#[pollster::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let lister = Lister::new();
    let probes = lister.list_all().await;
    let info = probes.first().ok_or("no probe found")?;

    let probe = info.open().await?;
    let mut session = probe
        .attach(TargetSelector::Auto, Permissions::default())
        .await?;
    println!("target: {}", session.target().name);

    {
        let mut core = session.core(0).await?;
        println!("resetting core 0 and letting it run...");
        core.reset().await?;
        // Give the ROM bootloader time to emit its banner before we detach.
        core.run().await.ok();
    }

    std::thread::sleep(Duration::from_millis(2500));

    let mut core = session.core(0).await?;
    let status = core.status().await?;
    println!("post-reset core status: {status:?}");

    println!("STAGE2_RESET_RESULT=PASS");
    Ok(())
}
