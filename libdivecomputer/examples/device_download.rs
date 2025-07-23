use std::path::PathBuf;

use clap::{Parser as ClapParser, ValueEnum};
use libdivecomputer::{Dive, DiveComputer, Family, LibError, Product, Result, Transport};
use serde::{Deserialize, Serialize};
use tokio::fs;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Json,
    Xml,
    #[value(name = "pretty-json")]
    PrettyJson,
}

#[derive(ClapParser, Debug)]
#[command(author, version, about = "Download dives from dive computer", long_about = None)]
struct Args {
    /// Output filename (if not specified, prints to stdout)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Output format
    #[arg(short = 'p', long, value_enum, default_value = "pretty-json")]
    format: OutputFormat,

    /// Device name (e.g., "Shearwater Petrel 3")
    #[arg(short, long)]
    device: Option<String>,

    /// Device family type
    #[arg(short = 'f', long)]
    family: Option<Family>,

    /// Device transport
    #[arg(short = 't', long)]
    transport: Transport,

    /// Device fingerprint
    #[arg(long)]
    fingerprint: Option<String>,
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

    let product = if let Some(device_name) = &args.device {
        dive_computer
            .vendors()?
            .iter()
            .flat_map(|vendor| vendor.products())
            .find(|item| {
                let full_name = format!("{} {}", item.vendor, item.name);

                device_name == &full_name || device_name == &item.name
            })
            .ok_or(LibError::Other("Device not found".to_string()))
    } else if let Some(family) = &args.family {
        dive_computer
            .vendors()?
            .iter()
            .flat_map(|vendor| vendor.products())
            .find(|product| product.family == *family)
            .ok_or(LibError::Other("Device family not found".to_string()))
    } else {
        Err(LibError::Other(
            "No device name or family specified".to_string(),
        ))
    }?;

    let mut dive_output = DiveOutput {
        dives: Vec::new(),
        product: product.clone(),
    };

    let transport = product
        .transports
        .iter()
        .find(|transport| **transport == args.transport)
        .ok_or(LibError::Other("invalid transport".to_string()))?;

    println!("Scanning {transport:?} devices...");
    let mut devices = dive_computer.scan(&product, *transport).await?;
    let Some(device) = devices.next() else {
        return Err(LibError::Other("No device found".to_string()));
    };

    let mut iter = dive_computer
        .download(&product, device, args.fingerprint)
        .await?;

    while let Some(dive) = iter.next() {
        println!(
            "Dive {}m {} min at {}",
            dive.max_depth,
            dive.duration.as_secs() / 60,
            dive.start.to_string()
        );
        dive_output.dives.push(dive);
    }

    let output_string =
        match args.format {
            OutputFormat::Json => serde_json::to_string(&dive_output)
                .map_err(|err| LibError::Other(err.to_string()))?,
            OutputFormat::PrettyJson => serde_json::to_string_pretty(&dive_output)
                .map_err(|err| LibError::Other(err.to_string()))?,
            OutputFormat::Xml => serde_xml_rs::to_string(&dive_output)
                .map_err(|err| LibError::Other(err.to_string()))?,
        };

    if let Some(output_path) = &args.output {
        fs::write(output_path, output_string).await?;
    } else {
        println!("{output_string}");
    }

    Ok(())
}
