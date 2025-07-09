use libdivecomputer::{
    Context, Descriptor,
    device::{Device, DeviceScanner},
    error::LibError,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Dive Computer Downloader\n");

    let mut context = Context::default();
    context.set_loglevel(libdivecomputer::LogLevel::Debug)?;
    context.set_logfunc(|_level, _msg| {
        // println!("{level}: {msg}");
    })?;

    let _fingerprint = "68442273".to_string();
    let descriptor = Descriptor::from(&context)
        .find(|item| item.product() == "Petrel 3" && item.vendor() == "Shearwater")
        .ok_or(LibError::DeviceError("invalid device, Perdix 2".into()))?;

    println!(
        "Will try to download dives from {} {}",
        descriptor.vendor(),
        descriptor.product()
    );

    let scanner = DeviceScanner::new(&context);
    for transport in descriptor.transports() {
        println!("Scanning {transport:?} devices...");
        match scanner.scan_transport(transport) {
            Ok(devices) => {
                println!("  Found {} device(s):", devices.len());
                for transport in devices {
                    if transport.display_name().contains(&descriptor.product())
                        && transport.display_name().contains(&descriptor.vendor())
                    {
                        let mut device =
                            Device::new(&context, transport, &descriptor)?.connect()?;

                        // device.set_fingerprint(&fingerprint)?;
                        for dive in device.download()? {
                            println!(
                                "Start: {}, duration: {} min, depth {}m",
                                dive.start,
                                dive.duration.as_secs() * 60,
                                dive.max_depth
                            );
                        }
                    }
                    println!()
                }
            }
            Err(err) => {
                println!("  Error scanning {transport:?}: {err}");
            }
        }
    }

    println!();

    Ok(())
}
