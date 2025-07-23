use clap::{Parser as ClapParser, ValueEnum};
use libdivecomputer::{Dive, DiveComputer, LibError, Product, Result, Transport};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Json,
    Xml,
    #[value(name = "pretty-json")]
    PrettyJson,
}

#[derive(ClapParser, Debug)]
#[command(author, version, about = "Scan for dive computers", long_about = None)]
struct Args {
    /// Device name (e.g., "Shearwater Petrel 3")
    #[arg(short, long)]
    device: String,

    /// Device transport
    #[arg(short = 't', long)]
    transport: Option<Transport>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DiveOutput {
    product: Product,
    dives: Vec<Dive>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let dive_computer = DiveComputer::new();

    let product = dive_computer
        .vendors()?
        .iter()
        .flat_map(|vendor| vendor.products())
        .find(|item| {
            let full_name = format!("{} {}", item.vendor, item.name);

            args.device == full_name || args.device == item.name
        })
        .ok_or(LibError::Other("Device not found".to_string()))?;

    let transports = if let Some(transport) = args.transport {
        vec![transport]
    } else {
        product.transports.clone()
    };

    for transport in transports {
        println!("\nScanning {transport:?} devices...");
        for device in dive_computer.scan(&product, transport).await? {
            println!("{device:?}");
        }
    }

    Ok(())
}
