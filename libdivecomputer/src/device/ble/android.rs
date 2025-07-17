pub static JAVAVM: std::sync::OnceLock<JavaVM> = std::sync::OnceLock::new();

use jni::{AttachGuard, JavaVM};

std::thread_local! {
  pub static JNI_ENV: std::cell::RefCell<Option<AttachGuard<'static>>> = std::cell::RefCell::new(None);
}

pub fn init(env: jni::JNIEnv) -> Result<(), Box<dyn std::error::Error>> {
    jni_utils::init(&env)?;
    btleplug::platform::init(&env)?;

    Ok(())
}
