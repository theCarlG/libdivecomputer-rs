//! Shared Android JNI infrastructure.
//!
//! Stores the JVM reference and provides thread-attachment helpers
//! used by both the BLE and classic Bluetooth modules.

pub static JAVAVM: std::sync::OnceLock<jni::JavaVM> = std::sync::OnceLock::new();

std::thread_local! {
    pub static JNI_ENV: std::cell::RefCell<Option<jni::AttachGuard<'static>>> =
        std::cell::RefCell::new(None);
}

pub fn init(env: jni::JNIEnv) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let vm = env.get_java_vm()?;
    let _ = JAVAVM.set(vm);
    jni_utils::init(&env)?;
    #[cfg(feature = "ble")]
    btleplug::platform::init(&env)?;
    Ok(())
}

/// Attach the current thread to the JVM and return a guard that detaches on drop.
/// Must be called on any spawned thread before using Android APIs.
pub fn attach_current_thread()
-> std::result::Result<jni::AttachGuard<'static>, Box<dyn std::error::Error>> {
    let vm = JAVAVM
        .get()
        .ok_or("JavaVM not initialized — call init() first")?;
    Ok(vm.attach_current_thread()?)
}
