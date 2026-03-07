use std::path::PathBuf;

use clap::{Parser as ClapParser, ValueEnum};
use libdivecomputer::{
    Context, Descriptor, Device, DeviceEvent, Dive, DownloadOptions, IoStream, LogLevel, Result,
    Transport, scan,
};
use serde::{Deserialize, Serialize};

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
    device: String,

    /// Device transport
    #[arg(short = 't', long)]
    transport: Transport,

    /// Device fingerprint (hex string for incremental download)
    #[arg(long)]
    fingerprint: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DiveOutput {
    device: String,
    dives: Vec<Dive>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let ctx = Context::builder().log_level(LogLevel::Warning).build()?;

    let desc = Descriptor::find_by_name(&args.device)?;

    // Scan for devices.
    println!("Scanning {} devices...", args.transport);
    let devices = scan(&ctx, args.transport).execute()?;
    let device_info = devices
        .into_iter()
        .next()
        .ok_or_else(|| libdivecomputer::LibError::DeviceError("No device found".into()))?;

    println!("Connecting to {}...", device_info.name);

    let iostream = IoStream::open(&ctx, &device_info.connection)?;
    let dev = Device::open(&ctx, &desc, iostream)?;

    let fp_bytes = args
        .fingerprint
        .as_ref()
        .map(|fp| libdivecomputer::device::hex_string_to_bytes(fp))
        .transpose()
        .map_err(|e| libdivecomputer::LibError::ParseError(e.to_string()))?;

    let mut on_event = |event: DeviceEvent| match event {
        DeviceEvent::Progress { current, maximum } => {
            println!(
                "Progress: {:.1}%",
                100.0 * (current as f64) / (maximum as f64)
            );
        }
        DeviceEvent::DevInfo { model, serial, .. } => {
            println!("Device: model={model}, serial={serial}");
        }
        _ => {}
    };

    let result = dev.download_dives(DownloadOptions {
        fingerprint: fp_bytes.as_deref(),
        on_event: Some(&mut on_event),
    });

    if result.has_errors() {
        for e in &result.errors {
            eprintln!("Parse error: {e}");
        }
    }

    let dives = result.into_result()?;

    for dive in &dives {
        println!(
            "Dive {:.1}m {} min at {}",
            dive.max_depth,
            dive.duration.as_secs() / 60,
            dive.start,
        );
    }

    let output = DiveOutput {
        device: args.device,
        dives,
    };

    let output_string = match args.format {
        OutputFormat::Json => serde_json::to_string(&output).unwrap(),
        OutputFormat::PrettyJson => serde_json::to_string_pretty(&output).unwrap(),
        OutputFormat::Xml => serde_xml_rs::to_string(&output).unwrap(),
    };

    if let Some(output_path) = &args.output {
        std::fs::write(output_path, output_string)?;
    } else {
        println!("{output_string}");
    }

    Ok(())
}
