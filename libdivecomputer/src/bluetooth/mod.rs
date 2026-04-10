//! Classic Bluetooth (RFCOMM/SPP) support for Android.
//!
//! On desktop platforms (Linux, Windows, macOS) the C libdivecomputer library
//! handles classic Bluetooth natively via BlueZ or WinSock. On Android, those
//! APIs are unavailable so this module provides scanning and I/O through JNI
//! calls to Android's `android.bluetooth.*` Java API.
//!
//! The architecture mirrors the BLE module but is much simpler: classic BT is
//! blocking socket I/O, so no async runtime or event loop is needed.

#[cfg(target_os = "android")]
mod android;

#[cfg(target_os = "android")]
use std::ffi::c_void;
#[cfg(target_os = "android")]
use std::ptr;

#[cfg(target_os = "android")]
use libdivecomputer_sys as ffi;

#[cfg(target_os = "android")]
use crate::device::DeviceInfo;
#[cfg(target_os = "android")]
use crate::error::{LibError, Result};
#[cfg(target_os = "android")]
use crate::iostream::IoStream;

// ---------------------------------------------------------------------------
// Scanning
// ---------------------------------------------------------------------------

/// Scan for paired/bonded classic Bluetooth devices on Android.
///
/// Returns only bonded devices — Android does not support active classic BT
/// discovery from native code (same limitation as Subsurface).
#[cfg(target_os = "android")]
pub fn scan_bluetooth_android() -> Result<Vec<DeviceInfo>> {
    let _guard = crate::android::attach_current_thread()
        .map_err(|e| LibError::DeviceError(format!("JNI attach failed: {e}")))?;
    android::get_bonded_devices()
}

// ---------------------------------------------------------------------------
// Custom iostream transport
// ---------------------------------------------------------------------------

#[cfg(target_os = "android")]
struct BtTransport {
    socket: android::BluetoothSocket,
    timeout_ms: i32, // -1 = blocking, 0 = non-blocking, >0 = ms
}

// ---------------------------------------------------------------------------
// FFI callback functions
// ---------------------------------------------------------------------------

#[cfg(target_os = "android")]
extern "C" fn bt_close(io: *mut c_void) -> ffi::dc_status_t {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if !io.is_null() {
            #[expect(unsafe_code)]
            let _transport = unsafe { Box::from_raw(io.cast::<BtTransport>()) };
        }
        ffi::DC_STATUS_SUCCESS
    }));
    match result {
        Ok(status) => status,
        Err(_) => ffi::DC_STATUS_IO,
    }
}

#[cfg(target_os = "android")]
extern "C" fn bt_read(
    io: *mut c_void,
    data: *mut c_void,
    size: usize,
    actual: *mut usize,
) -> ffi::dc_status_t {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if io.is_null() || data.is_null() {
            return ffi::DC_STATUS_IO;
        }

        // Ensure this thread is attached to the JVM.
        let _guard = match crate::android::attach_current_thread() {
            Ok(g) => g,
            Err(_) => return ffi::DC_STATUS_IO,
        };

        #[expect(unsafe_code)]
        let transport = unsafe { &*(io.cast::<BtTransport>()) };
        #[expect(unsafe_code)]
        let buffer = unsafe { std::slice::from_raw_parts_mut(data.cast::<u8>(), size) };

        // If a timeout is set, poll for data availability first.
        if transport.timeout_ms >= 0 {
            let deadline = std::time::Instant::now()
                + std::time::Duration::from_millis(
                    u64::try_from(transport.timeout_ms).unwrap_or(0),
                );
            loop {
                match transport.socket.available() {
                    Ok(n) if n > 0 => break,
                    Ok(_) => {}
                    Err(_) => return ffi::DC_STATUS_IO,
                }
                if std::time::Instant::now() >= deadline {
                    return ffi::DC_STATUS_TIMEOUT;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }

        match transport.socket.read(buffer) {
            Ok(0) => ffi::DC_STATUS_IO, // end of stream
            Ok(bytes_read) => {
                if !actual.is_null() {
                    #[expect(unsafe_code)]
                    unsafe {
                        *actual = bytes_read;
                    }
                }
                ffi::DC_STATUS_SUCCESS
            }
            Err(_) => ffi::DC_STATUS_IO,
        }
    }));
    match result {
        Ok(status) => status,
        Err(_) => ffi::DC_STATUS_IO,
    }
}

#[cfg(target_os = "android")]
extern "C" fn bt_write(
    io: *mut c_void,
    data: *const c_void,
    size: usize,
    actual: *mut usize,
) -> ffi::dc_status_t {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if io.is_null() || data.is_null() {
            return ffi::DC_STATUS_IO;
        }

        let _guard = match crate::android::attach_current_thread() {
            Ok(g) => g,
            Err(_) => return ffi::DC_STATUS_IO,
        };

        #[expect(unsafe_code)]
        let transport = unsafe { &*(io.cast::<BtTransport>()) };
        #[expect(unsafe_code)]
        let data_slice = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), size) };

        match transport.socket.write(data_slice) {
            Ok(bytes_written) => {
                if !actual.is_null() {
                    #[expect(unsafe_code)]
                    unsafe {
                        *actual = bytes_written;
                    }
                }
                ffi::DC_STATUS_SUCCESS
            }
            Err(_) => ffi::DC_STATUS_IO,
        }
    }));
    match result {
        Ok(status) => status,
        Err(_) => ffi::DC_STATUS_IO,
    }
}

#[cfg(target_os = "android")]
extern "C" fn bt_poll(io: *mut c_void, timeout: i32) -> ffi::dc_status_t {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if io.is_null() {
            return ffi::DC_STATUS_IO;
        }

        let _guard = match crate::android::attach_current_thread() {
            Ok(g) => g,
            Err(_) => return ffi::DC_STATUS_IO,
        };

        #[expect(unsafe_code)]
        let transport = unsafe { &*(io.cast::<BtTransport>()) };

        let deadline = std::time::Instant::now()
            + std::time::Duration::from_millis(u64::try_from(timeout).unwrap_or(0));

        loop {
            match transport.socket.available() {
                Ok(n) if n > 0 => return ffi::DC_STATUS_SUCCESS,
                Ok(_) => {}
                Err(_) => return ffi::DC_STATUS_IO,
            }
            if std::time::Instant::now() >= deadline {
                return ffi::DC_STATUS_TIMEOUT;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }));
    match result {
        Ok(status) => status,
        Err(_) => ffi::DC_STATUS_IO,
    }
}

#[cfg(target_os = "android")]
extern "C" fn bt_set_timeout(io: *mut c_void, timeout: i32) -> ffi::dc_status_t {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if io.is_null() {
            return ffi::DC_STATUS_IO;
        }

        // SAFETY: we are the sole accessor of this transport from the C library's
        // perspective; libdivecomputer calls callbacks sequentially on one thread.
        #[expect(unsafe_code)]
        let transport = unsafe { &mut *(io.cast::<BtTransport>()) };
        transport.timeout_ms = timeout;
        ffi::DC_STATUS_SUCCESS
    }));
    match result {
        Ok(status) => status,
        Err(_) => ffi::DC_STATUS_IO,
    }
}

#[cfg(target_os = "android")]
extern "C" fn bt_get_available(io: *mut c_void, available: *mut usize) -> ffi::dc_status_t {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if io.is_null() || available.is_null() {
            return ffi::DC_STATUS_IO;
        }

        let _guard = match crate::android::attach_current_thread() {
            Ok(g) => g,
            Err(_) => return ffi::DC_STATUS_IO,
        };

        #[expect(unsafe_code)]
        let transport = unsafe { &*(io.cast::<BtTransport>()) };

        match transport.socket.available() {
            Ok(n) => {
                #[expect(unsafe_code)]
                unsafe {
                    *available = n;
                }
                ffi::DC_STATUS_SUCCESS
            }
            Err(_) => ffi::DC_STATUS_IO,
        }
    }));
    match result {
        Ok(status) => status,
        Err(_) => ffi::DC_STATUS_IO,
    }
}

#[cfg(target_os = "android")]
extern "C" fn bt_purge(io: *mut c_void, _direction: ffi::dc_direction_t) -> ffi::dc_status_t {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if io.is_null() {
            return ffi::DC_STATUS_IO;
        }

        let _guard = match crate::android::attach_current_thread() {
            Ok(g) => g,
            Err(_) => return ffi::DC_STATUS_IO,
        };

        #[expect(unsafe_code)]
        let transport = unsafe { &*(io.cast::<BtTransport>()) };

        // Read and discard all available bytes.
        if let Ok(n) = transport.socket.available() {
            if n > 0 {
                let mut discard = vec![0u8; n];
                let _ = transport.socket.read(&mut discard);
            }
        }
        ffi::DC_STATUS_SUCCESS
    }));
    match result {
        Ok(status) => status,
        Err(_) => ffi::DC_STATUS_IO,
    }
}

// ---------------------------------------------------------------------------
// iostream opener
// ---------------------------------------------------------------------------

/// Open a classic Bluetooth RFCOMM iostream for the given MAC address.
///
/// Creates a `BtTransport` wrapping an Android `BluetoothSocket` and registers
/// it with libdivecomputer via `dc_custom_open(DC_TRANSPORT_BLUETOOTH, ...)`.
#[cfg(target_os = "android")]
pub fn bt_iostream_open(ctx: &crate::context::Context, address: &str) -> Result<IoStream> {
    let _guard = crate::android::attach_current_thread()
        .map_err(|e| LibError::DeviceError(format!("JNI attach failed: {e}")))?;

    let socket = android::connect(address)?;
    let transport = BtTransport {
        socket,
        timeout_ms: -1,
    };
    let io_ptr = Box::into_raw(Box::new(transport)).cast::<c_void>();

    let callbacks = ffi::dc_custom_cbs_t {
        set_timeout: Some(bt_set_timeout),
        set_break: None,
        set_dtr: None,
        set_rts: None,
        get_lines: None,
        get_available: Some(bt_get_available),
        configure: None,
        poll: Some(bt_poll),
        read: Some(bt_read),
        write: Some(bt_write),
        ioctl: None,
        flush: None,
        purge: Some(bt_purge),
        sleep: None,
        close: Some(bt_close),
    };

    let mut iostream_ptr = ptr::null_mut();
    #[expect(unsafe_code)]
    let status = unsafe {
        ffi::dc_custom_open(
            &mut iostream_ptr,
            ctx.ptr(),
            ffi::DC_TRANSPORT_BLUETOOTH,
            &callbacks,
            io_ptr,
        )
    };

    if status != ffi::DC_STATUS_SUCCESS {
        // Reclaim and drop the transport on failure.
        #[expect(unsafe_code)]
        unsafe {
            drop(Box::from_raw(io_ptr.cast::<BtTransport>()));
        }
        return Err(LibError::status_with_context(
            status,
            "failed to open Bluetooth iostream",
        ));
    }

    Ok(IoStream::from_raw(iostream_ptr))
}
