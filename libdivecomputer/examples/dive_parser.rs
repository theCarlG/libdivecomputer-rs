use clap::{Parser as ClapParser, ValueEnum};
use libdivecomputer::{Context, Descriptor, Dive, Family, LogLevel, Parser};
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
}

#[derive(Debug, Serialize, Deserialize)]
struct DiveOutput {
    device: String,
    dives: Vec<DiveData>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DiveData {
    #[serde(flatten)]
    dive: Dive,
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

    let ctx = Context::builder().log_level(LogLevel::Warning).build()?;

    let desc = if let Some(ref device_name) = args.device {
        Descriptor::find_by_name(device_name).map_err(|e| format!("{e}"))?
    } else if let Some(family) = args.family {
        Descriptor::iter()
            .map_err(|e| format!("{e}"))?
            .find(|d| d.family() == family)
            .ok_or_else(|| format!("Device family '{family}' not found"))?
    } else {
        return Err("Either --device or --family must be specified".into());
    };

    let device_name = format!("{} {}", desc.vendor(), desc.product());

    let mut dive_output = DiveOutput {
        dives: Vec::new(),
        device: device_name.clone(),
    };

    for (index, file_path) in args.files.iter().enumerate() {
        eprintln!("Parsing file: {}", file_path.display());

        let data = fs::read(file_path)?;
        let file_size = data.len();

        let fingerprint = if data.len() > 16 {
            &data[12..16]
        } else {
            &data
        };

        let parser = Parser::from_descriptor(&ctx, &desc, &data)?;
        match parser.parse(fingerprint) {
            Ok(dive) => {
                dive_output.dives.push(DiveData {
                    dive,
                    file_info: Some(FileInfo {
                        filename: file_path.display().to_string(),
                        size: file_size,
                        index,
                    }),
                });
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
