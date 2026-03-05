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
    parser::{Dive, Parser},
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
#[derive(Debug, Clone)]
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
struct ForeachData<'a> {
    dive_cb: &'a mut dyn FnMut(&[u8], &[u8]) -> bool,
    event_cb: Option<&'a mut dyn FnMut(DeviceEvent)>,
    cancel_cb: Option<&'a dyn Fn() -> bool>,
}

/// Connected dive computer device. Wraps `dc_device_t`.
pub struct Device {
    ptr: *mut ffi::dc_device_t,
    _iostream: IoStream,
}

unsafe impl Send for Device {}
unsafe impl Sync for Device {}

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
    pub fn set_fingerprint(&self, fingerprint: &[u8]) -> Result<()> {
        let status = unsafe {
            ffi::dc_device_set_fingerprint(
                self.ptr,
                fingerprint.as_ptr(),
                fingerprint.len() as c_uint,
            )
        };
        Status::check(status, "failed to set fingerprint")
    }

    /// Set the fingerprint from a hex string.
    pub fn set_fingerprint_hex(&self, hex: &str) -> Result<()> {
        let bytes = hex_string_to_bytes(hex)?;
        self.set_fingerprint(&bytes)
    }

    /// Download all dives, calling `dive_cb` for each.
    /// The callback receives `(data, fingerprint)` and returns `true` to continue.
    pub fn foreach<F>(&self, mut dive_cb: F) -> Result<()>
    where
        F: FnMut(&[u8], &[u8]) -> bool,
    {
        let mut data = ForeachData {
            dive_cb: &mut dive_cb,
            event_cb: None,
            cancel_cb: None,
        };

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

            let status =
                ffi::dc_device_foreach(self.ptr, Some(dive_callback), as_void_ptr(&mut data));
            Status::check(status, "failed to download dives")?;
        }

        Ok(())
    }

    /// Download all dives with both dive and event callbacks.
    pub fn foreach_with_events<D, E>(&self, mut dive_cb: D, mut event_cb: E) -> Result<()>
    where
        D: FnMut(&[u8], &[u8]) -> bool,
        E: FnMut(DeviceEvent),
    {
        let mut data = ForeachData {
            dive_cb: &mut dive_cb,
            event_cb: Some(&mut event_cb),
            cancel_cb: None,
        };

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

            let status =
                ffi::dc_device_foreach(self.ptr, Some(dive_callback), as_void_ptr(&mut data));
            Status::check(status, "failed to download dives")?;
        }

        Ok(())
    }

    /// Set a cancel callback that is polled during downloads.
    pub fn foreach_with_cancel<D, E, C>(
        &self,
        mut dive_cb: D,
        mut event_cb: E,
        cancel_cb: C,
    ) -> Result<()>
    where
        D: FnMut(&[u8], &[u8]) -> bool,
        E: FnMut(DeviceEvent),
        C: Fn() -> bool,
    {
        let mut data = ForeachData {
            dive_cb: &mut dive_cb,
            event_cb: Some(&mut event_cb),
            cancel_cb: Some(&cancel_cb),
        };

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

            let status =
                ffi::dc_device_set_cancel(self.ptr, Some(cancel_callback), as_void_ptr(&mut data));
            Status::check(status, "failed to set cancel callback")?;

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
    /// For streaming or custom control flow, use the lower-level `foreach` methods.
    pub fn download_dives(&self, options: &mut DownloadOptions<'_>) -> Result<Vec<Dive>> {
        if let Some(fp) = options.fingerprint {
            self.set_fingerprint(fp)?;
        }

        let mut dives = Vec::new();
        let mut first_error: Option<LibError> = None;

        {
            let mut dive_cb = |data: &[u8], fingerprint: &[u8]| -> bool {
                match Parser::from_device(self, data).and_then(|parser| parser.parse(fingerprint)) {
                    Ok(dive) => dives.push(dive),
                    Err(e) => {
                        if first_error.is_none() {
                            first_error = Some(e);
                        }
                    }
                }
                true
            };

            match options.on_event.as_mut() {
                Some(event_cb) => {
                    self.foreach_with_events(&mut dive_cb, event_cb)?;
                }
                None => {
                    self.foreach(&mut dive_cb)?;
                }
            }
        }

        if dives.is_empty()
            && let Some(e) = first_error
        {
            return Err(e);
        }

        Ok(dives)
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
    pub fingerprint: Option<&'a [u8]>,
    /// Optional callback for device events (progress, device info, etc.).
    pub on_event: Option<Box<dyn FnMut(DeviceEvent) + 'a>>,
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
}

extern "C" fn dive_callback(
    data: *const c_uchar,
    size: c_uint,
    fingerprint: *const c_uchar,
    fsize: c_uint,
    userdata: *mut c_void,
) -> c_int {
    let foreach_data = unsafe { from_void_ptr::<ForeachData>(userdata) };

    let data_slice = unsafe { std::slice::from_raw_parts(data, size as usize) };
    let fp_slice = unsafe { std::slice::from_raw_parts(fingerprint, fsize as usize) };

    if (foreach_data.dive_cb)(data_slice, fp_slice) {
        1
    } else {
        0
    }
}

extern "C" fn cancel_callback(userdata: *mut c_void) -> c_int {
    let foreach_data = unsafe { from_void_ptr::<ForeachData>(userdata) };
    if let Some(ref cb) = foreach_data.cancel_cb {
        if cb() { 1 } else { 0 }
    } else {
        0
    }
}

/// Convert a hex string to bytes.
pub fn hex_string_to_bytes(hex: &str) -> std::result::Result<Vec<u8>, std::num::ParseIntError> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
        .collect()
}

/// Convert bytes to a hex string.
pub fn bytes_to_hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02X}")).collect()
}
