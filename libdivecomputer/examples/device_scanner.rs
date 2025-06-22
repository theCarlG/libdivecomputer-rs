use libdivecomputer::{
    Context,
    device::{DeviceScanner, Transport},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Dive Computer Scanner\n");

    let mut context = Context::default();
    context.set_loglevel(libdivecomputer::LogLevel::Debug)?;
    context.set_logfunc(|level, msg| {
        println!("{level}: {msg}");
    })?;

    let scanner = DeviceScanner::new(&context);
    let transports = [
        Transport::Serial,
        Transport::Usb,
        Transport::UsbHid,
        Transport::Bluetooth,
        Transport::Ble,
    ];

    for transport in transports {
        println!("\nScanning {:?} devices...", transport);

        match scanner.scan_transport(transport) {
            Ok(devices) => {
                println!("  Found {} device(s):", devices.len());
                for transport in devices {
                    println!("      • {transport}");
                }
            }
            Err(err) => {
                println!("  Error scanning {:?}: {}", transport, err);
            }
        }
    }

    Ok(())
}
