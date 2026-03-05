use std::ffi::CString;
use std::ptr;

use libdivecomputer_sys as ffi;

use crate::context::Context;
use crate::device::ConnectionInfo;
use crate::error::{LibError, Result};
use crate::status::Status;

/// Direction for purge operations.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Direction {
    Input = ffi::DC_DIRECTION_INPUT,
    Output = ffi::DC_DIRECTION_OUTPUT,
    All = ffi::DC_DIRECTION_ALL,
}

/// Serial parity.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Parity {
    None = ffi::DC_PARITY_NONE,
    Odd = ffi::DC_PARITY_ODD,
    Even = ffi::DC_PARITY_EVEN,
    Mark = ffi::DC_PARITY_MARK,
    Space = ffi::DC_PARITY_SPACE,
}

/// Serial stop bits.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum StopBits {
    One = ffi::DC_STOPBITS_ONE,
    OneAndHalf = ffi::DC_STOPBITS_ONEPOINTFIVE,
    Two = ffi::DC_STOPBITS_TWO,
}

/// Serial flow control.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FlowControl {
    None = ffi::DC_FLOWCONTROL_NONE,
    Hardware = ffi::DC_FLOWCONTROL_HARDWARE,
    Software = ffi::DC_FLOWCONTROL_SOFTWARE,
}

/// Serial port configuration.
#[derive(Debug, Clone)]
pub struct SerialConfig {
    pub baudrate: u32,
    pub databits: u32,
    pub parity: Parity,
    pub stopbits: StopBits,
    pub flowcontrol: FlowControl,
}

/// Safe wrapper around `dc_iostream_t`. Manages the iostream lifecycle.
pub struct IoStream {
    pub(crate) ptr: *mut ffi::dc_iostream_t,
}

unsafe impl Send for IoStream {}
unsafe impl Sync for IoStream {}

impl IoStream {
    /// Open an iostream for the given connection info, auto-dispatching to the
    /// correct transport.
    ///
    /// This is the recommended way to open a connection. For transport-specific
    /// options (e.g. custom Bluetooth port or IrDA LSAP), use the individual
    /// constructors directly.
    pub fn open(ctx: &Context, connection: &ConnectionInfo) -> Result<Self> {
        match connection {
            ConnectionInfo::Serial { path, .. } => Self::serial(ctx, path),
            ConnectionInfo::Bluetooth { address, .. } => Self::bluetooth(ctx, *address, 0),
            ConnectionInfo::Irda { address, .. } => Self::irda(ctx, *address, 1),
            ConnectionInfo::UsbStorage { path, .. } => Self::usb_storage(ctx, path),
            #[cfg(feature = "ble")]
            ConnectionInfo::Ble { address_string, .. } => {
                crate::ble::ble_iostream_open(ctx, address_string)
            }
            #[cfg(not(feature = "ble"))]
            ConnectionInfo::Ble { .. } => Err(LibError::TransportNotSupported("BLE".into())),
            ConnectionInfo::Usb { .. } | ConnectionInfo::UsbHid { .. } => {
                Err(LibError::TransportNotSupported(
                    "USB/USB HID requires device handle from scanner".into(),
                ))
            }
        }
    }

    /// Open a serial port iostream.
    pub fn serial(ctx: &Context, name: &str) -> Result<Self> {
        let mut ptr = ptr::null_mut();
        let c_name = CString::new(name)?;
        let status = unsafe { ffi::dc_serial_open(&mut ptr, ctx.ptr(), c_name.as_ptr()) };
        Status::check(status, "failed to open serial iostream")?;
        Ok(Self { ptr })
    }

    /// Open a USB iostream by device reference.
    #[allow(dead_code)]
    pub(crate) fn usb_from_device(
        ctx: &Context,
        device: *mut ffi::dc_usb_device_t,
    ) -> Result<Self> {
        let mut ptr = ptr::null_mut();
        let status = unsafe { ffi::dc_usb_open(&mut ptr, ctx.ptr(), device) };
        Status::check(status, "failed to open USB iostream")?;
        Ok(Self { ptr })
    }

    /// Open a USB HID iostream by device reference.
    #[allow(dead_code)]
    pub(crate) fn usbhid_from_device(
        ctx: &Context,
        device: *mut ffi::dc_usbhid_device_t,
    ) -> Result<Self> {
        let mut ptr = ptr::null_mut();
        let status = unsafe { ffi::dc_usbhid_open(&mut ptr, ctx.ptr(), device) };
        Status::check(status, "failed to open USB HID iostream")?;
        Ok(Self { ptr })
    }

    /// Open an IrDA iostream.
    pub fn irda(ctx: &Context, address: u32, lsap: u32) -> Result<Self> {
        let mut ptr = ptr::null_mut();
        let status = unsafe { ffi::dc_irda_open(&mut ptr, ctx.ptr(), address, lsap) };
        Status::check(status, "failed to open IrDA iostream")?;
        Ok(Self { ptr })
    }

    /// Open a Bluetooth iostream.
    pub fn bluetooth(ctx: &Context, address: u64, port: u32) -> Result<Self> {
        let mut ptr = ptr::null_mut();
        let status = unsafe { ffi::dc_bluetooth_open(&mut ptr, ctx.ptr(), address, port) };
        Status::check(status, "failed to open Bluetooth iostream")?;
        Ok(Self { ptr })
    }

    /// Open a USB storage iostream (for mass-storage dive computers).
    pub fn usb_storage(ctx: &Context, name: &str) -> Result<Self> {
        let mut ptr = ptr::null_mut();
        let c_name = CString::new(name)?;
        let status = unsafe { ffi::dc_usb_storage_open(&mut ptr, ctx.ptr(), c_name.as_ptr()) };
        Status::check(status, "failed to open USB storage iostream")?;
        Ok(Self { ptr })
    }

    /// Wrap a raw `dc_iostream_t` pointer. Takes ownership.
    #[allow(dead_code)]
    pub(crate) fn from_raw(ptr: *mut ffi::dc_iostream_t) -> Self {
        Self { ptr }
    }

    /// Set the read timeout in milliseconds.
    /// Negative = blocking, 0 = non-blocking, positive = timed.
    pub fn set_timeout(&self, timeout_ms: i32) -> Result<()> {
        let status = unsafe { ffi::dc_iostream_set_timeout(self.ptr, timeout_ms) };
        Status::check(status, "failed to set iostream timeout")
    }

    /// Configure serial port parameters.
    pub fn configure(&self, config: &SerialConfig) -> Result<()> {
        let status = unsafe {
            ffi::dc_iostream_configure(
                self.ptr,
                config.baudrate,
                config.databits,
                config.parity as _,
                config.stopbits as _,
                config.flowcontrol as _,
            )
        };
        Status::check(status, "failed to configure iostream")
    }

    /// Read data from the stream.
    pub fn read(&self, buf: &mut [u8]) -> Result<usize> {
        let mut actual: usize = 0;
        let status = unsafe {
            ffi::dc_iostream_read(self.ptr, buf.as_mut_ptr() as *mut _, buf.len(), &mut actual)
        };
        Status::check(status, "failed to read from iostream")?;
        Ok(actual)
    }

    /// Write data to the stream.
    pub fn write(&self, data: &[u8]) -> Result<usize> {
        let mut actual: usize = 0;
        let status = unsafe {
            ffi::dc_iostream_write(self.ptr, data.as_ptr() as *const _, data.len(), &mut actual)
        };
        Status::check(status, "failed to write to iostream")?;
        Ok(actual)
    }

    /// Poll for available data. Returns `Ok(true)` if data is available,
    /// `Ok(false)` on timeout.
    pub fn poll(&self, timeout_ms: i32) -> Result<bool> {
        let status = unsafe { ffi::dc_iostream_poll(self.ptr, timeout_ms) };
        if status == ffi::DC_STATUS_SUCCESS {
            Ok(true)
        } else if status == ffi::DC_STATUS_TIMEOUT {
            Ok(false)
        } else {
            Status::check(status, "failed to poll iostream")?;
            unreachable!()
        }
    }

    /// Flush the output buffer.
    pub fn flush(&self) -> Result<()> {
        let status = unsafe { ffi::dc_iostream_flush(self.ptr) };
        Status::check(status, "failed to flush iostream")
    }

    /// Purge internal buffers.
    pub fn purge(&self, direction: Direction) -> Result<()> {
        let status = unsafe { ffi::dc_iostream_purge(self.ptr, direction as _) };
        Status::check(status, "failed to purge iostream")
    }

    /// Set the break condition.
    pub fn set_break(&self, value: bool) -> Result<()> {
        let status = unsafe { ffi::dc_iostream_set_break(self.ptr, value as u32) };
        Status::check(status, "failed to set break")
    }

    /// Set the DTR line state.
    pub fn set_dtr(&self, value: bool) -> Result<()> {
        let status = unsafe { ffi::dc_iostream_set_dtr(self.ptr, value as u32) };
        Status::check(status, "failed to set DTR")
    }

    /// Set the RTS line state.
    pub fn set_rts(&self, value: bool) -> Result<()> {
        let status = unsafe { ffi::dc_iostream_set_rts(self.ptr, value as u32) };
        Status::check(status, "failed to set RTS")
    }

    /// Get the line signal states as a bitmask.
    pub fn get_lines(&self) -> Result<u32> {
        let mut value: u32 = 0;
        let status = unsafe { ffi::dc_iostream_get_lines(self.ptr, &mut value) };
        Status::check(status, "failed to get lines")?;
        Ok(value)
    }

    /// Get the number of bytes available in the input buffer.
    pub fn available(&self) -> Result<usize> {
        let mut value: usize = 0;
        let status = unsafe { ffi::dc_iostream_get_available(self.ptr, &mut value) };
        Status::check(status, "failed to get available bytes")?;
        Ok(value)
    }
}

impl std::fmt::Debug for IoStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IoStream")
            .field("open", &!self.ptr.is_null())
            .finish()
    }
}

impl Drop for IoStream {
    fn drop(&mut self) {
        unsafe {
            if !self.ptr.is_null() {
                ffi::dc_iostream_close(self.ptr);
            }
        }
    }
}
