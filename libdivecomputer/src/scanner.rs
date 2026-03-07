use std::ffi::{CStr, c_void};
use std::ptr;
use std::time::Duration;

use libdivecomputer_sys as ffi;

use crate::context::Context;
use crate::device::{ConnectionInfo, DeviceInfo};
use crate::error::{LibError, Result};
use crate::transport::Transport;

/// Builder for scanning for dive computer devices.
pub struct ScanBuilder<'a> {
    ctx: &'a Context,
    transport: Transport,
    timeout: Duration,
}

impl<'a> ScanBuilder<'a> {
    /// Set the scan timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Execute the scan and return discovered devices.
    pub fn execute(self) -> Result<Vec<DeviceInfo>> {
        match self.transport {
            Transport::Serial => scan_serial(self.ctx),
            Transport::Usb => scan_usb(self.ctx),
            Transport::UsbHid => scan_usbhid(self.ctx),
            Transport::Bluetooth => scan_bluetooth(self.ctx),
            Transport::Irda => scan_irda(self.ctx),
            #[cfg(feature = "ble")]
            Transport::Ble => crate::ble::scan_ble(self.timeout),
            #[cfg(not(feature = "ble"))]
            Transport::Ble => Err(LibError::TransportNotSupported(
                "BLE (feature not enabled)".into(),
            )),
            Transport::UsbStorage => Ok(Vec::new()), // No iterator-based scanning for USB storage
        }
    }

    /// Execute the scan and return discovered devices.
    #[deprecated(since = "0.2.0", note = "Use `execute()` instead")]
    pub fn scan(self) -> Result<Vec<DeviceInfo>> {
        self.execute()
    }
}

/// Create a scanner for the given transport.
pub fn scan(ctx: &Context, transport: Transport) -> ScanBuilder<'_> {
    ScanBuilder {
        ctx,
        transport,
        timeout: Duration::from_secs(5),
    }
}

/// Generic helper for C iterator-based scanning.
fn scan_with_iterator<T, FCreate, FNext, FExtract, FFree>(
    create: FCreate,
    next: FNext,
    extract: FExtract,
    free: FFree,
    transport_name: &str,
) -> Result<Vec<DeviceInfo>>
where
    FCreate: FnOnce(&mut *mut ffi::dc_iterator_t) -> ffi::dc_status_t,
    FNext: Fn(*mut ffi::dc_iterator_t, &mut *mut T) -> ffi::dc_status_t,
    FExtract: Fn(*mut T) -> DeviceInfo,
    FFree: Fn(*mut T),
{
    let mut iterator = ptr::null_mut();
    let status = create(&mut iterator);
    if status != ffi::DC_STATUS_SUCCESS {
        return Err(LibError::status_with_context(
            status,
            format!("failed to create {transport_name} iterator"),
        ));
    }

    let mut devices = Vec::new();

    loop {
        let mut device: *mut T = ptr::null_mut();
        let status = next(iterator, &mut device);

        if status == ffi::DC_STATUS_DONE {
            break;
        }
        if status != ffi::DC_STATUS_SUCCESS {
            break;
        }
        if device.is_null() {
            continue;
        }

        devices.push(extract(device));
        free(device);
    }

    unsafe { ffi::dc_iterator_free(iterator) };
    Ok(devices)
}

fn scan_serial(ctx: &Context) -> Result<Vec<DeviceInfo>> {
    scan_with_iterator(
        |iter| unsafe { ffi::dc_serial_iterator_new(iter, ctx.ptr(), ptr::null_mut()) },
        |iter, device| unsafe { ffi::dc_iterator_next(iter, device as *mut _ as *mut c_void) },
        |device| {
            let name_ptr = unsafe { ffi::dc_serial_device_get_name(device) };
            let path = if name_ptr.is_null() {
                "Unknown".to_string()
            } else {
                unsafe { CStr::from_ptr(name_ptr).to_string_lossy().to_string() }
            };
            let name = extract_device_name(&path);
            DeviceInfo {
                name: name.clone(),
                transport: Transport::Serial,
                connection: ConnectionInfo::Serial { name, path },
            }
        },
        |device| unsafe { ffi::dc_serial_device_free(device) },
        "serial",
    )
}

fn scan_usb(ctx: &Context) -> Result<Vec<DeviceInfo>> {
    scan_with_iterator(
        |iter| unsafe { ffi::dc_usb_iterator_new(iter, ctx.ptr(), ptr::null_mut()) },
        |iter, device| unsafe { ffi::dc_iterator_next(iter, device as *mut _ as *mut c_void) },
        |device| {
            let vid = unsafe { ffi::dc_usb_device_get_vid(device) } as u16;
            let pid = unsafe { ffi::dc_usb_device_get_pid(device) } as u16;
            let name = format!("USB Device {vid:04X}:{pid:04X}");
            DeviceInfo {
                name,
                transport: Transport::Usb,
                connection: ConnectionInfo::Usb {
                    vendor_id: vid,
                    product_id: pid,
                },
            }
        },
        |device| unsafe { ffi::dc_usb_device_free(device) },
        "USB",
    )
}

fn scan_usbhid(ctx: &Context) -> Result<Vec<DeviceInfo>> {
    scan_with_iterator(
        |iter| unsafe { ffi::dc_usbhid_iterator_new(iter, ctx.ptr(), ptr::null_mut()) },
        |iter, device| unsafe { ffi::dc_iterator_next(iter, device as *mut _ as *mut c_void) },
        |device| {
            let vid = unsafe { ffi::dc_usbhid_device_get_vid(device) } as u16;
            let pid = unsafe { ffi::dc_usbhid_device_get_pid(device) } as u16;
            let name = format!("USB HID Device {vid:04X}:{pid:04X}");
            DeviceInfo {
                name,
                transport: Transport::UsbHid,
                connection: ConnectionInfo::UsbHid {
                    vendor_id: vid,
                    product_id: pid,
                },
            }
        },
        |device| unsafe { ffi::dc_usbhid_device_free(device) },
        "USB HID",
    )
}

fn scan_bluetooth(ctx: &Context) -> Result<Vec<DeviceInfo>> {
    scan_with_iterator(
        |iter| unsafe { ffi::dc_bluetooth_iterator_new(iter, ctx.ptr(), ptr::null_mut()) },
        |iter, device| unsafe { ffi::dc_iterator_next(iter, device as *mut _ as *mut c_void) },
        |device| {
            let address = unsafe { ffi::dc_bluetooth_device_get_address(device) };
            let name_ptr = unsafe { ffi::dc_bluetooth_device_get_name(device) };
            let name = if name_ptr.is_null() {
                "Unknown Bluetooth Device".to_string()
            } else {
                unsafe { CStr::from_ptr(name_ptr).to_string_lossy().to_string() }
            };
            let address_string = format_bluetooth_address(address);
            DeviceInfo {
                name: name.clone(),
                transport: Transport::Bluetooth,
                connection: ConnectionInfo::Bluetooth {
                    address,
                    address_string,
                    name,
                },
            }
        },
        |device| unsafe { ffi::dc_bluetooth_device_free(device) },
        "Bluetooth",
    )
}

fn scan_irda(ctx: &Context) -> Result<Vec<DeviceInfo>> {
    scan_with_iterator(
        |iter| unsafe { ffi::dc_irda_iterator_new(iter, ctx.ptr(), ptr::null_mut()) },
        |iter, device| unsafe { ffi::dc_iterator_next(iter, device as *mut _ as *mut c_void) },
        |device| {
            let address = unsafe { ffi::dc_irda_device_get_address(device) };
            let name_ptr = unsafe { ffi::dc_irda_device_get_name(device) };
            let name = if name_ptr.is_null() {
                "Unknown IrDA Device".to_string()
            } else {
                unsafe { CStr::from_ptr(name_ptr).to_string_lossy().to_string() }
            };
            DeviceInfo {
                name: name.clone(),
                transport: Transport::Irda,
                connection: ConnectionInfo::Irda { address, name },
            }
        },
        |device| unsafe { ffi::dc_irda_device_free(device) },
        "IrDA",
    )
}

/// Format a Bluetooth address as a colon-separated hex string.
pub fn format_bluetooth_address(address: u64) -> String {
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        (address >> 40) & 0xFF,
        (address >> 32) & 0xFF,
        (address >> 24) & 0xFF,
        (address >> 16) & 0xFF,
        (address >> 8) & 0xFF,
        address & 0xFF
    )
}

/// Extract a friendly device name from a path.
fn extract_device_name(path: &str) -> String {
    path.split('/').next_back().unwrap_or(path).to_string()
}

/// Convert a MAC address string to a u64.
pub fn mac_string_to_u64(mac: &str) -> Option<u64> {
    let parts: Vec<&str> = mac.split(':').collect();
    if parts.len() != 6 {
        return None;
    }
    let mut address: u64 = 0;
    for (i, part) in parts.iter().enumerate() {
        let byte = u8::from_str_radix(part, 16).ok()?;
        address |= (byte as u64) << (40 - i * 8);
    }
    Some(address)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_bluetooth_address_known() {
        let addr: u64 = 0xAABBCCDDEEFF;
        assert_eq!(format_bluetooth_address(addr), "AA:BB:CC:DD:EE:FF");
    }

    #[test]
    fn format_bluetooth_address_zero() {
        assert_eq!(format_bluetooth_address(0), "00:00:00:00:00:00");
    }

    #[test]
    fn mac_string_to_u64_valid() {
        let addr = mac_string_to_u64("AA:BB:CC:DD:EE:FF").unwrap();
        assert_eq!(addr, 0xAABBCCDDEEFF);
    }

    #[test]
    fn mac_string_to_u64_lowercase() {
        let addr = mac_string_to_u64("aa:bb:cc:dd:ee:ff").unwrap();
        assert_eq!(addr, 0xAABBCCDDEEFF);
    }

    #[test]
    fn mac_string_to_u64_wrong_octets() {
        assert!(mac_string_to_u64("AA:BB:CC").is_none());
        assert!(mac_string_to_u64("AA:BB:CC:DD:EE:FF:00").is_none());
    }

    #[test]
    fn mac_string_to_u64_invalid_hex() {
        assert!(mac_string_to_u64("GG:HH:II:JJ:KK:LL").is_none());
    }

    #[test]
    fn mac_round_trip() {
        let mac = "AA:BB:CC:DD:EE:FF";
        let addr = mac_string_to_u64(mac).unwrap();
        let recovered = format_bluetooth_address(addr);
        assert_eq!(recovered, mac);
    }
}
