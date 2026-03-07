use std::{
    borrow::Cow,
    ffi::{c_int, c_uchar, c_uint, c_void},
    fmt, ptr,
};

use libdivecomputer_sys as ffi;
use serde::{Deserialize, Serialize};

use crate::{
    buffer::Buffer,
    common::{as_void_ptr, from_void_ptr},
    context::Context,
    descriptor::Descriptor,
    error::{LibError, Result},
    iostream::IoStream,
    parser::{Dive, Fingerprint, Parser},
    status::Status,
    transport::Transport,
};

/// Information about a discovered device.
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub name: String,
    pub transport: Transport,
    pub connection: ConnectionInfo,
}

/// Connection details for a device.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ConnectionInfo {
    Serial {
        name: String,
        path: String,
    },
    Usb {
        vendor_id: u16,
        product_id: u16,
    },
    UsbHid {
        vendor_id: u16,
        product_id: u16,
    },
    Bluetooth {
        address: u64,
        name: String,
        address_string: String,
    },
    Ble {
        address: u64,
        local_name: Option<String>,
        service_name: String,
        address_string: String,
    },
    Irda {
        address: u32,
        name: String,
    },
    UsbStorage {
        name: String,
        path: String,
    },
}

impl ConnectionInfo {
    /// Get a connection string for this device.
    pub fn connection_string(&self) -> Option<Cow<'_, str>> {
        match self {
            Self::Serial { path, .. } => Some(Cow::Borrowed(path)),
            Self::Bluetooth { address_string, .. } | Self::Ble { address_string, .. } => {
                Some(Cow::Borrowed(address_string))
            }
            Self::Irda { address, .. } => Some(Cow::Owned(format!("0x{address:08X}"))),
            Self::UsbStorage { path, .. } => Some(Cow::Borrowed(path)),
            Self::Usb { .. } | Self::UsbHid { .. } => None,
        }
    }

    /// Get a human-readable display name.
    pub fn display_name(&self) -> Cow<'_, str> {
        match self {
            Self::Serial { name, .. } => Cow::Borrowed(name),
            Self::Usb {
                vendor_id,
                product_id,
            }
            | Self::UsbHid {
                vendor_id,
                product_id,
            } => Cow::Owned(format!("USB Device {vendor_id:04X}:{product_id:04X}")),
            Self::Bluetooth { name, .. } => Cow::Borrowed(name),
            Self::Ble {
                local_name,
                service_name,
                ..
            } => local_name
                .as_ref()
                .map(|name| Cow::Owned(format!("{name} - {service_name}")))
                .unwrap_or(Cow::Borrowed(service_name)),
            Self::Irda { name, .. } => Cow::Borrowed(name),
            Self::UsbStorage { name, .. } => Cow::Borrowed(name),
        }
    }
}

impl fmt::Display for ConnectionInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

impl From<&ConnectionInfo> for Transport {
    fn from(value: &ConnectionInfo) -> Self {
        match value {
            ConnectionInfo::Serial { .. } => Self::Serial,
            ConnectionInfo::Usb { .. } => Self::Usb,
            ConnectionInfo::UsbHid { .. } => Self::UsbHid,
            ConnectionInfo::Bluetooth { .. } => Self::Bluetooth,
            ConnectionInfo::Ble { .. } => Self::Ble,
            ConnectionInfo::Irda { .. } => Self::Irda,
            ConnectionInfo::UsbStorage { .. } => Self::UsbStorage,
        }
    }
}

/// A device event received during download.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DeviceEvent {
    /// Device is waiting for user action (e.g. press a button).
    Waiting,
    /// Download progress update.
    Progress { current: u32, maximum: u32 },
    /// Device info received.
    DevInfo {
        model: u32,
        firmware: u32,
        serial: u32,
    },
    /// Clock sync info.
    Clock { devtime: u32, systime: i64 },
    /// Vendor-specific data.
    Vendor { data: Vec<u8> },
}

/// Callback data passed to the FFI during foreach.
struct ForeachData<'d, 'e, 'c> {
    dive_cb: &'d mut dyn FnMut(&[u8], &Fingerprint) -> bool,
    event_cb: Option<&'e mut dyn FnMut(DeviceEvent)>,
    cancel_cb: Option<&'c dyn Fn() -> bool>,
}

/// Connected dive computer device. Wraps `dc_device_t`.
pub struct Device {
    ptr: *mut ffi::dc_device_t,
    _iostream: IoStream,
}

// SAFETY: dc_device_t operations are serialized through the C library.
// The device is accessed through &self methods that go through FFI, where
// the C library handles any necessary synchronization.
unsafe impl Send for Device {}

impl Device {
    /// Open a device connection.
    pub fn open(ctx: &Context, desc: &Descriptor, iostream: IoStream) -> Result<Self> {
        let mut ptr = ptr::null_mut();
        let status = unsafe { ffi::dc_device_open(&mut ptr, ctx.ptr(), desc.ptr, iostream.ptr) };
        Status::check(status, "failed to open device")?;
        Ok(Self {
            ptr,
            _iostream: iostream,
        })
    }

    /// Set the fingerprint for incremental downloads.
    pub fn set_fingerprint(&self, fingerprint: &Fingerprint) -> Result<()> {
        let bytes = fingerprint.as_bytes();
        let status = unsafe {
            ffi::dc_device_set_fingerprint(self.ptr, bytes.as_ptr(), bytes.len() as c_uint)
        };
        Status::check(status, "failed to set fingerprint")
    }

    /// Set the fingerprint from a hex string.
    pub fn set_fingerprint_hex(&self, hex: &str) -> Result<()> {
        let fp = Fingerprint::from_hex(hex)?;
        self.set_fingerprint(&fp)
    }

    /// Download all dives, calling `dive_cb` for each.
    ///
    /// The callback receives `(data, fingerprint)` and returns `true` to continue.
    /// Optionally provide an event callback and/or a cancel callback.
    pub fn foreach(
        &self,
        dive_cb: &mut dyn FnMut(&[u8], &Fingerprint) -> bool,
        event_cb: Option<&mut dyn FnMut(DeviceEvent)>,
        cancel_cb: Option<&dyn Fn() -> bool>,
    ) -> Result<()> {
        self.foreach_internal(ForeachData {
            dive_cb,
            event_cb,
            cancel_cb,
        })
    }

    fn foreach_internal(&self, mut data: ForeachData<'_, '_, '_>) -> Result<()> {
        let has_cancel = data.cancel_cb.is_some();

        unsafe {
            let events = ffi::DC_EVENT_WAITING
                | ffi::DC_EVENT_PROGRESS
                | ffi::DC_EVENT_DEVINFO
                | ffi::DC_EVENT_CLOCK
                | ffi::DC_EVENT_VENDOR;

            let status = ffi::dc_device_set_events(
                self.ptr,
                events,
                Some(event_callback),
                as_void_ptr(&mut data),
            );
            Status::check(status, "failed to set event handler")?;

            if has_cancel {
                let status = ffi::dc_device_set_cancel(
                    self.ptr,
                    Some(cancel_callback),
                    as_void_ptr(&mut data),
                );
                Status::check(status, "failed to set cancel callback")?;
            }

            let status =
                ffi::dc_device_foreach(self.ptr, Some(dive_callback), as_void_ptr(&mut data));
            Status::check(status, "failed to download dives")?;
        }

        Ok(())
    }

    /// Read memory from the device at the given address.
    pub fn read(&self, address: u32, buf: &mut [u8]) -> Result<()> {
        let status = unsafe {
            ffi::dc_device_read(self.ptr, address, buf.as_mut_ptr(), buf.len() as c_uint)
        };
        Status::check(status, "failed to read from device")
    }

    /// Write memory to the device at the given address.
    pub fn write(&self, address: u32, data: &[u8]) -> Result<()> {
        let status =
            unsafe { ffi::dc_device_write(self.ptr, address, data.as_ptr(), data.len() as c_uint) };
        Status::check(status, "failed to write to device")
    }

    /// Dump the full device memory.
    pub fn dump(&self) -> Result<Vec<u8>> {
        let buffer = Buffer::new(0);
        let status = unsafe { ffi::dc_device_dump(self.ptr, buffer.ptr) };
        Status::check(status, "failed to dump device memory")?;
        Ok(buffer.to_vec())
    }

    /// Synchronize the device clock.
    pub fn timesync(&self, timestamp: jiff::Timestamp) -> Result<()> {
        let ffi_dt = crate::datetime::timestamp_to_ffi(timestamp);
        let status = unsafe { ffi::dc_device_timesync(self.ptr, &ffi_dt) };
        Status::check(status, "failed to sync device time")
    }

    /// Create a parser for dive data from this device.
    pub fn parser(&self, data: &[u8]) -> Result<Parser> {
        Parser::from_device(self, data)
    }

    /// Download and parse all dives from the device.
    ///
    /// This is a convenience method that handles the foreach/parse cycle.
    /// For streaming or custom control flow, use the lower-level `foreach` method.
    ///
    /// Returns successfully parsed dives and any parse errors that occurred.
    pub fn download_dives(&self, options: DownloadOptions<'_>) -> DownloadResult {
        if let Some(fp) = options.fingerprint
            && let Err(e) = self.set_fingerprint(fp)
        {
            return DownloadResult {
                dives: Vec::new(),
                errors: vec![e],
            };
        }

        let mut dives = Vec::new();
        let mut errors: Vec<LibError> = Vec::new();

        {
            let mut dive_cb = |data: &[u8], fingerprint: &Fingerprint| -> bool {
                match Parser::from_device(self, data).and_then(|parser| parser.parse(fingerprint)) {
                    Ok(dive) => dives.push(dive),
                    Err(e) => errors.push(e),
                }
                true
            };

            if let Err(e) = self.foreach_internal(ForeachData {
                dive_cb: &mut dive_cb,
                event_cb: options.on_event,
                cancel_cb: options.cancel_cb,
            }) {
                errors.push(e);
            }
        }

        DownloadResult { dives, errors }
    }

    /// Get the device family (type).
    pub fn family(&self) -> crate::family::Family {
        let raw = unsafe { ffi::dc_device_get_type(self.ptr) };
        crate::family::Family::from(raw)
    }

    /// Get the raw device pointer (for vendor-specific APIs).
    pub(crate) fn raw_ptr(&self) -> *mut ffi::dc_device_t {
        self.ptr
    }
}

/// Options for downloading and parsing dives.
#[derive(Default)]
pub struct DownloadOptions<'a> {
    /// Fingerprint for incremental downloads. Only dives newer than this will be downloaded.
    pub fingerprint: Option<&'a Fingerprint>,
    /// Optional callback for device events (progress, device info, etc.).
    pub on_event: Option<&'a mut dyn FnMut(DeviceEvent)>,
    /// Optional callback to cancel the download. Return `true` to cancel.
    pub cancel_cb: Option<&'a dyn Fn() -> bool>,
}

/// Result of a dive download operation.
///
/// Contains both successfully parsed dives and any errors encountered during parsing.
/// This allows partial success: some dives may parse correctly while others fail.
pub struct DownloadResult {
    /// Successfully parsed dives.
    pub dives: Vec<Dive>,
    /// Errors encountered during download or parsing.
    pub errors: Vec<LibError>,
}

impl DownloadResult {
    /// Returns `true` if all dives were parsed successfully (no errors).
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    /// Returns `true` if any errors occurred during parsing.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Consume this result, returning the dives if successful, the first error if no dives
    /// were parsed, or a `PartialDownload` error if some dives succeeded but errors occurred.
    pub fn into_result(self) -> Result<Vec<Dive>> {
        if self.errors.is_empty() {
            Ok(self.dives)
        } else if self.dives.is_empty() {
            Err(self.errors.into_iter().next().unwrap())
        } else {
            Err(LibError::PartialDownload {
                dives: self.dives,
                errors: self.errors,
            })
        }
    }
}

impl std::fmt::Debug for Device {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Device")
            .field("open", &!self.ptr.is_null())
            .finish()
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            if !self.ptr.is_null() {
                ffi::dc_device_close(self.ptr);
            }
        }
    }
}

extern "C" fn event_callback(
    _device: *mut ffi::dc_device_t,
    event: ffi::dc_event_type_t,
    data: *const c_void,
    userdata: *mut c_void,
) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let foreach_data = unsafe { from_void_ptr::<ForeachData>(userdata) };

        let device_event = match event {
            ffi::DC_EVENT_WAITING => DeviceEvent::Waiting,
            ffi::DC_EVENT_PROGRESS => {
                let progress = unsafe { &*(data as *const ffi::dc_event_progress_t) };
                DeviceEvent::Progress {
                    current: progress.current,
                    maximum: progress.maximum,
                }
            }
            ffi::DC_EVENT_DEVINFO => {
                let devinfo = unsafe { &*(data as *const ffi::dc_event_devinfo_t) };
                DeviceEvent::DevInfo {
                    model: devinfo.model,
                    firmware: devinfo.firmware,
                    serial: devinfo.serial,
                }
            }
            ffi::DC_EVENT_CLOCK => {
                let clock = unsafe { &*(data as *const ffi::dc_event_clock_t) };
                DeviceEvent::Clock {
                    devtime: clock.devtime,
                    systime: clock.systime,
                }
            }
            ffi::DC_EVENT_VENDOR => {
                let vendor = unsafe { &*(data as *const ffi::dc_event_vendor_t) };
                let data_slice =
                    unsafe { std::slice::from_raw_parts(vendor.data, vendor.size as usize) };
                DeviceEvent::Vendor {
                    data: data_slice.to_vec(),
                }
            }
            _ => return,
        };

        if let Some(ref mut cb) = foreach_data.event_cb {
            cb(device_event);
        }
    }));
}

extern "C" fn dive_callback(
    data: *const c_uchar,
    size: c_uint,
    fingerprint: *const c_uchar,
    fsize: c_uint,
    userdata: *mut c_void,
) -> c_int {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let foreach_data = unsafe { from_void_ptr::<ForeachData>(userdata) };

        let data_slice = unsafe { std::slice::from_raw_parts(data, size as usize) };
        let fp_slice = unsafe { std::slice::from_raw_parts(fingerprint, fsize as usize) };
        let fp = Fingerprint::from(fp_slice);

        if (foreach_data.dive_cb)(data_slice, &fp) {
            1
        } else {
            0
        }
    }));
    result.unwrap_or_default()
}

extern "C" fn cancel_callback(userdata: *mut c_void) -> c_int {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let foreach_data = unsafe { from_void_ptr::<ForeachData>(userdata) };
        if let Some(ref cb) = foreach_data.cancel_cb {
            if cb() { 1 } else { 0 }
        } else {
            0
        }
    }));
    result.unwrap_or_default()
}

/// Convert a hex string to bytes.
///
/// Prefer [`Fingerprint::from_hex`] for fingerprint-specific use cases.
#[deprecated(since = "0.2.0", note = "Use Fingerprint::from_hex instead")]
pub fn hex_string_to_bytes(hex: &str) -> Result<Vec<u8>> {
    crate::parser::Fingerprint::from_hex(hex).map(|fp| fp.as_bytes().to_vec())
}

/// Convert bytes to a hex string.
///
/// Prefer [`Fingerprint::to_hex`] for fingerprint-specific use cases.
#[deprecated(since = "0.2.0", note = "Use Fingerprint::to_hex instead")]
pub fn bytes_to_hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02X}")).collect()
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;

    #[test]
    fn hex_string_to_bytes_valid() {
        let bytes = hex_string_to_bytes("DEADBEEF").unwrap();
        assert_eq!(bytes, vec![0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn hex_string_to_bytes_invalid() {
        assert!(hex_string_to_bytes("ZZZZ").is_err());
        assert!(hex_string_to_bytes("ABC").is_err()); // odd length
    }

    #[test]
    fn bytes_to_hex_known() {
        assert_eq!(bytes_to_hex(&[0xDE, 0xAD, 0xBE, 0xEF]), "DEADBEEF");
        assert_eq!(bytes_to_hex(&[0x00, 0xFF]), "00FF");
    }

    #[test]
    fn hex_round_trip() {
        let original = vec![0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF];
        let hex = bytes_to_hex(&original);
        let recovered = hex_string_to_bytes(&hex).unwrap();
        assert_eq!(recovered, original);
    }

    #[test]
    fn connection_info_connection_string_serial() {
        let ci = ConnectionInfo::Serial {
            name: "ttyUSB0".into(),
            path: "/dev/ttyUSB0".into(),
        };
        assert_eq!(ci.connection_string().unwrap().as_ref(), "/dev/ttyUSB0");
    }

    #[test]
    fn connection_info_connection_string_usb_returns_none() {
        let ci = ConnectionInfo::Usb {
            vendor_id: 0x1234,
            product_id: 0x5678,
        };
        assert!(ci.connection_string().is_none());
    }

    #[test]
    fn connection_info_connection_string_ble() {
        let ci = ConnectionInfo::Ble {
            address: 0,
            local_name: Some("MyDevice".into()),
            service_name: "svc".into(),
            address_string: "AA:BB:CC:DD:EE:FF".into(),
        };
        assert_eq!(
            ci.connection_string().unwrap().as_ref(),
            "AA:BB:CC:DD:EE:FF"
        );
    }

    #[test]
    fn connection_info_display_name_serial() {
        let ci = ConnectionInfo::Serial {
            name: "ttyUSB0".into(),
            path: "/dev/ttyUSB0".into(),
        };
        assert_eq!(ci.display_name().as_ref(), "ttyUSB0");
    }

    #[test]
    fn connection_info_display_name_usb() {
        let ci = ConnectionInfo::Usb {
            vendor_id: 0x1234,
            product_id: 0x5678,
        };
        assert_eq!(ci.display_name().as_ref(), "USB Device 1234:5678");
    }

    #[test]
    fn connection_info_display_name_ble_with_name() {
        let ci = ConnectionInfo::Ble {
            address: 0,
            local_name: Some("MyDevice".into()),
            service_name: "svc".into(),
            address_string: "".into(),
        };
        assert_eq!(ci.display_name().as_ref(), "MyDevice - svc");
    }

    #[test]
    fn connection_info_display_name_ble_without_name() {
        let ci = ConnectionInfo::Ble {
            address: 0,
            local_name: None,
            service_name: "svc".into(),
            address_string: "".into(),
        };
        assert_eq!(ci.display_name().as_ref(), "svc");
    }

    #[test]
    fn transport_from_connection_info() {
        let cases: Vec<(ConnectionInfo, Transport)> = vec![
            (
                ConnectionInfo::Serial {
                    name: "".into(),
                    path: "".into(),
                },
                Transport::Serial,
            ),
            (
                ConnectionInfo::Usb {
                    vendor_id: 0,
                    product_id: 0,
                },
                Transport::Usb,
            ),
            (
                ConnectionInfo::UsbHid {
                    vendor_id: 0,
                    product_id: 0,
                },
                Transport::UsbHid,
            ),
            (
                ConnectionInfo::Bluetooth {
                    address: 0,
                    name: "".into(),
                    address_string: "".into(),
                },
                Transport::Bluetooth,
            ),
            (
                ConnectionInfo::Ble {
                    address: 0,
                    local_name: None,
                    service_name: "".into(),
                    address_string: "".into(),
                },
                Transport::Ble,
            ),
            (
                ConnectionInfo::Irda {
                    address: 0,
                    name: "".into(),
                },
                Transport::Irda,
            ),
            (
                ConnectionInfo::UsbStorage {
                    name: "".into(),
                    path: "".into(),
                },
                Transport::UsbStorage,
            ),
        ];
        for (ci, expected) in &cases {
            assert_eq!(Transport::from(ci), *expected);
        }
    }

    #[test]
    fn download_result_is_ok_and_has_errors() {
        let ok_result = DownloadResult {
            dives: vec![],
            errors: vec![],
        };
        assert!(ok_result.is_ok());
        assert!(!ok_result.has_errors());

        let err_result = DownloadResult {
            dives: vec![],
            errors: vec![LibError::Unknown],
        };
        assert!(!err_result.is_ok());
        assert!(err_result.has_errors());
    }

    #[test]
    fn download_result_into_result_empty_with_errors() {
        let result = DownloadResult {
            dives: vec![],
            errors: vec![LibError::Unknown],
        };
        assert!(result.into_result().is_err());
    }

    #[test]
    fn download_result_into_result_has_dives_and_errors() {
        let result = DownloadResult {
            dives: vec![Dive::default()],
            errors: vec![LibError::Unknown],
        };
        match result.into_result() {
            Err(LibError::PartialDownload { dives, errors }) => {
                assert_eq!(dives.len(), 1);
                assert_eq!(errors.len(), 1);
            }
            other => panic!("Expected PartialDownload, got {other:?}"),
        }
    }

    #[test]
    fn download_result_into_result_empty_no_errors() {
        let result = DownloadResult {
            dives: vec![],
            errors: vec![],
        };
        let dives = result.into_result().unwrap();
        assert!(dives.is_empty());
    }

    #[test]
    fn download_options_default() {
        let opts = DownloadOptions::default();
        assert!(opts.fingerprint.is_none());
        assert!(opts.on_event.is_none());
        assert!(opts.cancel_cb.is_none());
    }
}
