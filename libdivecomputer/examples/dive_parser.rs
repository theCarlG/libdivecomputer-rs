use clap::{Parser as ClapParser, ValueEnum};
use libdivecomputer::DiveComputer;
use libdivecomputer::{Context, Descriptor, LogLevel, parser::Parser};
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
    family: Option<String>,

    /// Model number
    #[arg(short, long)]
    model: Option<u32>,

    /// Device time (UNIX timestamp)
    #[arg(long)]
    devtime: Option<u32>,

    /// System time (UNIX timestamp)
    #[arg(short = 's', long)]
    systime: Option<i64>,

    /// Log level (0-5, where 0=none, 5=all)
    #[arg(short = 'v', long, default_value = "2")]
    loglevel: u8,
}

#[derive(Debug, Serialize, Deserialize)]
struct DiveOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    device: Option<DiveComputer>,
    dives: Vec<DiveData>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DiveData {
    #[serde(flatten)]
    dive: libdivecomputer::parser::Dive,
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

    let mut context = Context::default();
    let log_level = match args.loglevel {
        0 => LogLevel::None,
        1 => LogLevel::Error,
        2 => LogLevel::Warning,
        3 => LogLevel::Info,
        4 => LogLevel::Debug,
        _ => LogLevel::All,
    };
    context.set_loglevel(log_level)?;
    context.set_logfunc(|level, msg| {
        eprintln!("{level}: {msg}");
    })?;

    let descriptor_item = find_descriptor(&context, &args)?;

    let dive_computer = DiveComputer::try_from(&descriptor_item).unwrap();

    let mut dive_output = DiveOutput {
        device: Some(dive_computer),
        dives: Vec::new(),
    };

    for (index, file_path) in args.files.iter().enumerate() {
        eprintln!("Parsing file: {}", file_path.display());

        let data = fs::read(file_path)?;
        let file_size = data.len();

        match Parser::parse_standalone(&context, &descriptor_item, data) {
            Ok(dive) => {
                let dive_data = DiveData {
                    dive,
                    file_info: Some(FileInfo {
                        filename: file_path.display().to_string(),
                        size: file_size,
                        index: index + 1,
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

fn find_descriptor<'ctx>(
    context: &'ctx Context,
    args: &Args,
) -> Result<libdivecomputer::DescriptorItem<'ctx>, Box<dyn std::error::Error>> {
    if let Some(device_name) = &args.device {
        Descriptor::from(context)
            .find(|item| {
                let product = item.product();
                let vendor = item.vendor();
                let full_name = format!("{vendor} {product}");

                device_name == &full_name || device_name == &product
            })
            .ok_or("Device not found".into())
    } else if let Some(family_name) = &args.family {
        Descriptor::from(context)
            .find(|item| {
                let matches_family = format!("{:?}", item.family())
                    .to_lowercase()
                    .contains(&family_name.to_lowercase());
                if let Some(model) = args.model {
                    matches_family && item.model() == model
                } else {
                    matches_family
                }
            })
            .ok_or("Device family not found".into())
    } else {
        Err("No device name or family specified".into())
    }
}
