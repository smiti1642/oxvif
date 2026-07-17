//! Spin up a fleet of mock ONVIF cameras and batch-scan them.
//!
//! Each device binds its own ephemeral port with a distinct identity, so this
//! mirrors what a fleet health-scan / discovery UI / NVR onboarding flow sees —
//! no hardware required. Dropping the `Fleet` shuts every device down.
//!
//! ```text
//! cargo run --example mock_fleet --features mock-server
//! ```

use oxvif::OnvifClient;
use oxvif::mock::Fleet;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Five virtual cameras, each on its own port with a distinct serial/model.
    let fleet = Fleet::start(5).await?;
    println!("started {} devices", fleet.len());

    // Batch-scan: hit every device the way a fleet tool would.
    for url in fleet.device_urls() {
        let client = OnvifClient::new(url);
        let info = client.get_device_info().await?;
        println!(
            "  {url} → {} {} (sn {})",
            info.manufacturer, info.model, info.serial_number
        );
    }

    Ok(())
}
