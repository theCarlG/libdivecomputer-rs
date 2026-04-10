//! JNI helpers for Android classic Bluetooth (RFCOMM/SPP).
//!
//! Wraps `android.bluetooth.BluetoothAdapter`, `BluetoothDevice`, and
//! `BluetoothSocket` so that the parent module can scan for paired devices
//! and open RFCOMM connections without any Kotlin helper classes.

use jni::objects::{GlobalRef, JObject, JValue};
use jni::JNIEnv;

use crate::device::{ConnectionInfo, DeviceInfo};
use crate::error::{LibError, Result};
use crate::scanner::{format_bluetooth_address, mac_string_to_u64};
use crate::transport::Transport;

/// Standard Serial Port Profile UUID used by dive computers.
const SPP_UUID: &str = "00001101-0000-1000-8000-00805f9b34fb";

/// Wrapper holding JNI global references to a connected `BluetoothSocket`,
/// its `InputStream`, and `OutputStream`.
pub struct BluetoothSocket {
    socket: GlobalRef,
    input_stream: GlobalRef,
    output_stream: GlobalRef,
}

// SAFETY: The `GlobalRef` instances are JNI global references valid for the
// JVM lifetime. We only access them via properly attached threads.
#[expect(unsafe_code)]
unsafe impl Send for BluetoothSocket {}

fn get_env() -> Result<JNIEnv<'static>> {
    let vm = crate::android::JAVAVM
        .get()
        .ok_or_else(|| LibError::DeviceError("JavaVM not initialized".to_string()))?;
    vm.get_env()
        .map_err(|e| LibError::DeviceError(format!("Failed to get JNIEnv: {e}")))
}

fn check_and_clear_exception(env: &JNIEnv, context: &str) -> Result<()> {
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
        return Err(LibError::DeviceError(format!(
            "Java exception in {context}"
        )));
    }
    Ok(())
}

/// Return paired/bonded classic Bluetooth devices via JNI.
///
/// Calls `BluetoothAdapter.getDefaultAdapter().getBondedDevices()` and
/// filters to devices with classic BT support (`getType() & 1 != 0`).
pub fn get_bonded_devices() -> Result<Vec<DeviceInfo>> {
    let env = get_env()?;
    let mut devices = Vec::new();

    // BluetoothAdapter adapter = BluetoothAdapter.getDefaultAdapter();
    let adapter = env
        .call_static_method(
            "android/bluetooth/BluetoothAdapter",
            "getDefaultAdapter",
            "()Landroid/bluetooth/BluetoothAdapter;",
            &[],
        )
        .map_err(|e| LibError::DeviceError(format!("getDefaultAdapter failed: {e}")))?
        .l()
        .map_err(|e| LibError::DeviceError(format!("getDefaultAdapter result: {e}")))?;
    check_and_clear_exception(&env, "getDefaultAdapter")?;

    if adapter.is_null() {
        return Err(LibError::DeviceError(
            "No Bluetooth adapter available".to_string(),
        ));
    }

    // Set<BluetoothDevice> bonded = adapter.getBondedDevices();
    let bonded_set = env
        .call_method(adapter, "getBondedDevices", "()Ljava/util/Set;", &[])
        .map_err(|e| LibError::DeviceError(format!("getBondedDevices failed: {e}")))?
        .l()
        .map_err(|e| LibError::DeviceError(format!("getBondedDevices result: {e}")))?;
    check_and_clear_exception(&env, "getBondedDevices")?;

    if bonded_set.is_null() {
        return Ok(devices);
    }

    // int size = bonded.size();
    let size = env
        .call_method(bonded_set, "size", "()I", &[])
        .map_err(|e| LibError::DeviceError(format!("Set.size failed: {e}")))?
        .i()
        .map_err(|e| LibError::DeviceError(format!("Set.size result: {e}")))?;
    check_and_clear_exception(&env, "Set.size")?;

    if size == 0 {
        return Ok(devices);
    }

    // Iterator<BluetoothDevice> iter = bonded.iterator();
    let iterator = env
        .call_method(bonded_set, "iterator", "()Ljava/util/Iterator;", &[])
        .map_err(|e| LibError::DeviceError(format!("Set.iterator failed: {e}")))?
        .l()
        .map_err(|e| LibError::DeviceError(format!("Set.iterator result: {e}")))?;
    check_and_clear_exception(&env, "Set.iterator")?;

    for _ in 0..size {
        // BluetoothDevice dev = iter.next();
        let dev = env
            .call_method(iterator, "next", "()Ljava/lang/Object;", &[])
            .map_err(|e| LibError::DeviceError(format!("Iterator.next failed: {e}")))?
            .l()
            .map_err(|e| LibError::DeviceError(format!("Iterator.next result: {e}")))?;
        check_and_clear_exception(&env, "Iterator.next")?;

        if dev.is_null() {
            continue;
        }

        // int type = dev.getType();
        // 1 = DEVICE_TYPE_CLASSIC, 2 = DEVICE_TYPE_LE, 3 = DEVICE_TYPE_DUAL
        let bt_type = env
            .call_method(dev, "getType", "()I", &[])
            .map_err(|e| LibError::DeviceError(format!("getType failed: {e}")))?
            .i()
            .unwrap_or(0);
        check_and_clear_exception(&env, "getType")?;

        // Skip LE-only devices
        if bt_type & 1 == 0 {
            continue;
        }

        // String address = dev.getAddress();
        let address_jstr = env
            .call_method(dev, "getAddress", "()Ljava/lang/String;", &[])
            .map_err(|e| LibError::DeviceError(format!("getAddress failed: {e}")))?
            .l()
            .map_err(|e| LibError::DeviceError(format!("getAddress result: {e}")))?;
        check_and_clear_exception(&env, "getAddress")?;

        let address_string: String = env
            .get_string(address_jstr.into())
            .map_err(|e| LibError::DeviceError(format!("getAddress string: {e}")))?
            .into();

        // String name = dev.getName();
        let name_jstr = env
            .call_method(dev, "getName", "()Ljava/lang/String;", &[])
            .map_err(|e| LibError::DeviceError(format!("getName failed: {e}")))?
            .l()
            .map_err(|e| LibError::DeviceError(format!("getName result: {e}")))?;
        check_and_clear_exception(&env, "getName")?;

        let name = if name_jstr.is_null() {
            "Unknown Bluetooth Device".to_string()
        } else {
            let s: String = env
                .get_string(name_jstr.into())
                .map_err(|e| LibError::DeviceError(format!("getName string: {e}")))?
                .into();
            s
        };

        let address = mac_string_to_u64(&address_string).unwrap_or(0);

        devices.push(DeviceInfo {
            name: name.clone(),
            transport: Transport::Bluetooth,
            connection: ConnectionInfo::Bluetooth {
                address,
                address_string,
                name,
            },
        });
    }

    Ok(devices)
}

/// Connect to a classic Bluetooth device by MAC address using the SPP UUID.
///
/// The device must already be paired via Android Settings.
pub fn connect(address: &str) -> Result<BluetoothSocket> {
    let env = get_env()?;

    // BluetoothAdapter adapter = BluetoothAdapter.getDefaultAdapter();
    let adapter = env
        .call_static_method(
            "android/bluetooth/BluetoothAdapter",
            "getDefaultAdapter",
            "()Landroid/bluetooth/BluetoothAdapter;",
            &[],
        )
        .map_err(|e| LibError::DeviceError(format!("getDefaultAdapter failed: {e}")))?
        .l()
        .map_err(|e| LibError::DeviceError(format!("getDefaultAdapter result: {e}")))?;
    check_and_clear_exception(&env, "getDefaultAdapter")?;

    if adapter.is_null() {
        return Err(LibError::DeviceError(
            "No Bluetooth adapter available".to_string(),
        ));
    }

    // BluetoothDevice device = adapter.getRemoteDevice(address);
    let j_address = env
        .new_string(address)
        .map_err(|e| LibError::DeviceError(format!("new_string failed: {e}")))?;
    let device = env
        .call_method(
            adapter,
            "getRemoteDevice",
            "(Ljava/lang/String;)Landroid/bluetooth/BluetoothDevice;",
            &[JValue::Object(j_address.into())],
        )
        .map_err(|e| LibError::DeviceError(format!("getRemoteDevice failed: {e}")))?
        .l()
        .map_err(|e| LibError::DeviceError(format!("getRemoteDevice result: {e}")))?;
    check_and_clear_exception(&env, "getRemoteDevice")?;

    if device.is_null() {
        return Err(LibError::DeviceError(format!(
            "Remote device not found: {address}"
        )));
    }

    // UUID uuid = UUID.fromString(SPP_UUID);
    let j_uuid_str = env
        .new_string(SPP_UUID)
        .map_err(|e| LibError::DeviceError(format!("new_string UUID failed: {e}")))?;
    let uuid = env
        .call_static_method(
            "java/util/UUID",
            "fromString",
            "(Ljava/lang/String;)Ljava/util/UUID;",
            &[JValue::Object(j_uuid_str.into())],
        )
        .map_err(|e| LibError::DeviceError(format!("UUID.fromString failed: {e}")))?
        .l()
        .map_err(|e| LibError::DeviceError(format!("UUID.fromString result: {e}")))?;
    check_and_clear_exception(&env, "UUID.fromString")?;

    // BluetoothSocket socket = device.createRfcommSocketToServiceRecord(uuid);
    let socket = env
        .call_method(
            device,
            "createRfcommSocketToServiceRecord",
            "(Ljava/util/UUID;)Landroid/bluetooth/BluetoothSocket;",
            &[JValue::Object(uuid)],
        )
        .map_err(|e| {
            LibError::DeviceError(format!("createRfcommSocketToServiceRecord failed: {e}"))
        })?
        .l()
        .map_err(|e| {
            LibError::DeviceError(format!("createRfcommSocketToServiceRecord result: {e}"))
        })?;
    check_and_clear_exception(&env, "createRfcommSocketToServiceRecord")?;

    if socket.is_null() {
        return Err(LibError::DeviceError(
            "Failed to create RFCOMM socket".to_string(),
        ));
    }

    // Cancel discovery before connecting (Android recommendation).
    let _ = env.call_method(adapter, "cancelDiscovery", "()Z", &[]);
    let _ = check_and_clear_exception(&env, "cancelDiscovery");

    // socket.connect();  (blocking)
    env.call_method(socket, "connect", "()V", &[])
        .map_err(|e| LibError::DeviceError(format!("BluetoothSocket.connect failed: {e}")))?;
    check_and_clear_exception(&env, "BluetoothSocket.connect")?;

    // InputStream in = socket.getInputStream();
    let input_stream = env
        .call_method(socket, "getInputStream", "()Ljava/io/InputStream;", &[])
        .map_err(|e| LibError::DeviceError(format!("getInputStream failed: {e}")))?
        .l()
        .map_err(|e| LibError::DeviceError(format!("getInputStream result: {e}")))?;
    check_and_clear_exception(&env, "getInputStream")?;

    // OutputStream out = socket.getOutputStream();
    let output_stream = env
        .call_method(
            socket,
            "getOutputStream",
            "()Ljava/io/OutputStream;",
            &[],
        )
        .map_err(|e| LibError::DeviceError(format!("getOutputStream failed: {e}")))?
        .l()
        .map_err(|e| LibError::DeviceError(format!("getOutputStream result: {e}")))?;
    check_and_clear_exception(&env, "getOutputStream")?;

    // Store as global references so they survive across JNI frames.
    let socket_ref = env
        .new_global_ref(socket)
        .map_err(|e| LibError::DeviceError(format!("GlobalRef socket: {e}")))?;
    let input_ref = env
        .new_global_ref(input_stream)
        .map_err(|e| LibError::DeviceError(format!("GlobalRef input: {e}")))?;
    let output_ref = env
        .new_global_ref(output_stream)
        .map_err(|e| LibError::DeviceError(format!("GlobalRef output: {e}")))?;

    Ok(BluetoothSocket {
        socket: socket_ref,
        input_stream: input_ref,
        output_stream: output_ref,
    })
}

impl BluetoothSocket {
    /// Read up to `buf.len()` bytes from the input stream.
    /// Blocks until at least 1 byte is available or the stream ends.
    pub fn read(&self, buf: &mut [u8]) -> Result<usize> {
        let env = get_env()?;

        // Create a Java byte[] to receive data.
        let j_buf = env
            .new_byte_array(
                i32::try_from(buf.len())
                    .map_err(|_| LibError::DeviceError("buffer too large".to_string()))?,
            )
            .map_err(|e| LibError::DeviceError(format!("new_byte_array: {e}")))?;

        let input: JObject = self.input_stream.as_obj();

        // int n = inputStream.read(byte[], 0, len);
        let n = env
            .call_method(
                input,
                "read",
                "([BII)I",
                &[
                    JValue::Object(j_buf.into()),
                    JValue::Int(0),
                    JValue::Int(i32::try_from(buf.len()).unwrap_or(i32::MAX)),
                ],
            )
            .map_err(|e| LibError::DeviceError(format!("InputStream.read: {e}")))?
            .i()
            .map_err(|e| LibError::DeviceError(format!("InputStream.read result: {e}")))?;
        check_and_clear_exception(&env, "InputStream.read")?;

        if n < 0 {
            // -1 means end of stream
            return Ok(0);
        }

        let n = usize::try_from(n).unwrap_or(0);

        // Copy data from Java byte[] to Rust slice.
        env.get_byte_array_region(
            j_buf,
            0,
            // SAFETY: reinterpreting &mut [u8] as &mut [i8] for JNI — same layout.
            #[expect(unsafe_code)]
            unsafe {
                &mut *(std::ptr::from_mut::<[u8]>(&mut buf[..n]) as *mut [u8] as *mut [i8])
            },
        )
        .map_err(|e| LibError::DeviceError(format!("get_byte_array_region: {e}")))?;

        Ok(n)
    }

    /// Write bytes to the output stream.
    pub fn write(&self, data: &[u8]) -> Result<usize> {
        let env = get_env()?;

        let j_buf = env
            .new_byte_array(
                i32::try_from(data.len())
                    .map_err(|_| LibError::DeviceError("buffer too large".to_string()))?,
            )
            .map_err(|e| LibError::DeviceError(format!("new_byte_array: {e}")))?;

        // Copy Rust data into Java byte[].
        // SAFETY: reinterpreting &[u8] as &[i8] — same layout.
        #[expect(unsafe_code)]
        let data_i8 = unsafe { &*(std::ptr::from_ref::<[u8]>(data) as *const [u8] as *const [i8]) };
        env.set_byte_array_region(j_buf, 0, data_i8)
            .map_err(|e| LibError::DeviceError(format!("set_byte_array_region: {e}")))?;

        let output: JObject = self.output_stream.as_obj();

        // outputStream.write(byte[], 0, len);
        env.call_method(
            output,
            "write",
            "([BII)V",
            &[
                JValue::Object(j_buf.into()),
                JValue::Int(0),
                JValue::Int(i32::try_from(data.len()).unwrap_or(i32::MAX)),
            ],
        )
        .map_err(|e| LibError::DeviceError(format!("OutputStream.write: {e}")))?;
        check_and_clear_exception(&env, "OutputStream.write")?;

        Ok(data.len())
    }

    /// Return the number of bytes available without blocking.
    pub fn available(&self) -> Result<usize> {
        let env = get_env()?;
        let input: JObject = self.input_stream.as_obj();

        let n = env
            .call_method(input, "available", "()I", &[])
            .map_err(|e| LibError::DeviceError(format!("InputStream.available: {e}")))?
            .i()
            .map_err(|e| LibError::DeviceError(format!("InputStream.available result: {e}")))?;
        check_and_clear_exception(&env, "InputStream.available")?;

        Ok(usize::try_from(n).unwrap_or(0))
    }

    /// Close the socket (and its streams).
    pub fn close(&self) -> Result<()> {
        let env = get_env()?;
        let socket: JObject = self.socket.as_obj();

        let _ = env.call_method(socket, "close", "()V", &[]);
        let _ = check_and_clear_exception(&env, "BluetoothSocket.close");
        Ok(())
    }
}

impl Drop for BluetoothSocket {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
