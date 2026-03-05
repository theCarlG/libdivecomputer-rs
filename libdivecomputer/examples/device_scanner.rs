use clap::Parser as ClapParser;
use libdivecomputer::{Context, Descriptor, LogLevel, Result, Transport, scan};

#[derive(ClapParser, Debug)]
#[command(author, version, about = "Scan for dive computers", long_about = None)]
struct Args {
    /// Device name (e.g., "Shearwater Petrel 3")
    #[arg(short, long)]
    device: Option<String>,

    /// Device transport (Serial, USB, BLE, etc.)
    #[arg(short = 't', long)]
    transport: Option<Transport>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let ctx = Context::builder().log_level(LogLevel::Warning).build()?;

    // Determine which transports to scan.
    let transports = if let Some(transport) = args.transport {
        vec![transport]
    } else if let Some(ref device_name) = args.device {
        // Find the descriptor and use its supported transports.
        if let Some(desc) = Descriptor::find_by_name(&ctx, device_name)? {
            desc.transport_list()
        } else {
            eprintln!("Device '{}' not found in descriptor database", device_name);
            return Ok(());
        }
    } else {
        // Scan all available transports.
        ctx.get_transports().to_vec()
    };

    for transport in transports {
        println!("\nScanning {transport} devices...");
        match scan(&ctx, transport).execute() {
            Ok(devices) => {
                for device in &devices {
                    println!("  Found: {} ({})", device.name, device.connection);
                }
                if devices.is_empty() {
                    println!("  No devices found.");
                }
            }
            Err(e) => {
                eprintln!("  Error scanning: {e}");
            }
        }
    }

    Ok(())
}
