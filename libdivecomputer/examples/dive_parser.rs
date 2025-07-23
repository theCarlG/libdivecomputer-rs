use clap::{Parser as ClapParser, ValueEnum};
use libdivecomputer::{DiveComputerSync, Family, Product};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Json,
    Xml,
    #[value(name = "pretty-json")]
    PrettyJson,
}

#[derive(ClapParser, Debug)]
#[command(author, version, about = "Parse previously downloaded dives", long_about = None)]
struct Args {
    /// Input files to parse
    #[arg(required = true)]
    files: Vec<PathBuf>,

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

    /// Model number
    #[arg(short, long)]
    model: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DiveOutput {
    product: Product,
    dives: Vec<DiveData>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DiveData {
    #[serde(flatten)]
    dive: libdivecomputer::Dive,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_info: Option<FileInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
struct FileInfo {
    filename: String,
    size: usize,
    index: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let dive_computer = DiveComputerSync::new();

    let product = if let Some(device_name) = &args.device {
        dive_computer
            .vendors()?
            .iter()
            .flat_map(|vendor| vendor.products())
            .find(|item| {
                let full_name = format!("{} {}", item.vendor, item.name);

                device_name == &full_name || device_name == &item.name
            })
            .ok_or("Device not found".to_string())
    } else if let Some(family) = &args.family {
        dive_computer
            .vendors()?
            .iter()
            .flat_map(|vendor| vendor.products())
            .find(|product| product.family == *family)
            .ok_or("Device family not found".into())
    } else {
        Err("No device name or family specified".into())
    }?;

    let mut dive_output = DiveOutput {
        dives: Vec::new(),
        product: product.clone(),
    };

    for (index, file_path) in args.files.iter().enumerate() {
        eprintln!("Parsing file: {}", file_path.display());

        let data = fs::read(file_path)?;
        let file_size = data.len();

        match dive_computer.parse(&product, data) {
            Ok(dive) => {
                let dive_data = DiveData {
                    dive,
                    file_info: Some(FileInfo {
                        filename: file_path.display().to_string(),
                        size: file_size,
                        index,
                    }),
                };

                dive_output.dives.push(dive_data);
            }
            Err(e) => {
                eprintln!("Error parsing {}: {}", file_path.display(), e);
            }
        }
    }

    let output_string = match args.format {
        OutputFormat::Json => serde_json::to_string(&dive_output)?,
        OutputFormat::PrettyJson => serde_json::to_string_pretty(&dive_output)?,
        OutputFormat::Xml => serde_xml_rs::to_string(&dive_output)?,
    };

    if let Some(output_path) = &args.output {
        fs::write(output_path, output_string)?;
    } else {
        println!("{output_string}");
    }

    Ok(())
}
