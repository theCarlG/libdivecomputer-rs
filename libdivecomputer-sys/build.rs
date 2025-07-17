// Keep the original function for simple cases without environment variables
fn run_command_or_fail<C, P, S>(dir: C, cmd: P, args: &[S])
where
    C: AsRef<Path>,
    P: AsRef<Path>,
    S: Borrow<str> + AsRef<OsStr>,
{
    run_command_with_env(dir, cmd, args, &[]);
}
use std::borrow::Borrow;
use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use bindgen::callbacks::{ItemInfo, ParseCallbacks};

fn run_command_with_env<C, P, S>(dir: C, cmd: P, args: &[S], env_vars: &[(&str, &str)])
where
    C: AsRef<Path>,
    P: AsRef<Path>,
    S: Borrow<str> + AsRef<OsStr>,
{
    let cmd_path = cmd.as_ref();
    let cmd_path = if cmd_path.components().count() > 1 && cmd_path.is_relative() {
        dir.as_ref()
            .join(cmd_path)
            .canonicalize()
            .expect("canonicalization failed")
    } else {
        PathBuf::from(cmd_path)
    };

    eprintln!(
        "Running command: \"{} {}\" in dir: {} with env: {:?}",
        cmd_path.display(),
        args.join(" "),
        dir.as_ref().display(),
        env_vars
    );

    let mut command = Command::new(cmd_path);
    command.current_dir(dir).args(args);

    // Add environment variables safely
    for (key, value) in env_vars {
        command.env(key, value);
    }

    let ret = command.status();
    match ret.map(|status| (status.success(), status.code())) {
        Ok((true, _)) => (),
        Ok((false, Some(c))) => panic!("Command failed with error code {c}"),
        Ok((false, None)) => panic!("Command got killed"),
        Err(e) => panic!("Command failed with error: {e}"),
    }
}

fn get_target_info() -> (String, String, String) {
    let target = env::var("TARGET").unwrap();
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    (target, target_os, target_arch)
}

fn setup_ios_build(libdc_path: &Path, lib_root: &Path, target: &str) {
    // iOS cross-compilation setup
    let (sdk, arch, min_version) = match target {
        "aarch64-apple-ios" => ("iphoneos", "arm64", "11.0"),
        "x86_64-apple-ios" => ("iphonesimulator", "x86_64", "11.0"),
        "aarch64-apple-ios-sim" => ("iphonesimulator", "arm64", "14.0"),
        _ => panic!("Unsupported iOS target: {target}"),
    };

    // Get SDK path
    let sdk_path = Command::new("xcrun")
        .args(["--sdk", sdk, "--show-sdk-path"])
        .output()
        .expect("Failed to get iOS SDK path")
        .stdout;
    let binding = String::from_utf8(sdk_path).unwrap();
    let sdk_path = binding.trim();

    // Set up cross-compilation environment using Command env() instead of env::set_var
    let cc = format!("xcrun --sdk {sdk} clang");
    let ar = "xcrun ar".to_string();
    let ranlib = "xcrun ranlib".to_string();

    let cflags = format!(
        "-arch {arch} -isysroot {sdk_path} -mios-version-min={min_version} -fembed-bitcode"
    );

    let prefix = format!("--prefix={}", lib_root.display());
    let host_flag = format!("--host={arch}-apple-darwin");

    // Use Command with .env() to pass environment variables safely
    let status = Command::new("./configure")
        .current_dir(libdc_path)
        .env("CC", &cc)
        .env("AR", &ar)
        .env("RANLIB", &ranlib)
        .env("CFLAGS", &cflags)
        .env("LDFLAGS", &cflags)
        .args([&prefix, &host_flag, "--disable-shared"])
        .status()
        .expect("Failed to run configure");

    if !status.success() {
        panic!("Configure failed for iOS target: {target}");
    }
}

fn setup_android_build(libdc_path: &Path, lib_root: &Path, target: &str) {
    let ndk_home = env::var("ANDROID_NDK_HOME")
        .or_else(|_| env::var("NDK_HOME"))
        .expect("ANDROID_NDK_HOME or NDK_HOME must be set for Android builds");

    println!("cargo:rustc-env=ANDROID_NDK_HOME={ndk_home}");

    // Use the existing Android.mk build system
    let android_mk_path = libdc_path.join("contrib").join("android");

    // Map Rust target to Android ABI
    let android_abi = match target {
        "aarch64-linux-android" => "arm64-v8a",
        "armv7-linux-androideabi" => "armeabi-v7a",
        "i686-linux-android" => "x86",
        "x86_64-linux-android" => "x86_64",
        _ => panic!("Unsupported Android target: {target}"),
    };

    println!("cargo:rustc-env=ANDROID_ABI={android_abi}");

    let prefix = format!("--prefix={}", lib_root.display());
    run_command_or_fail(libdc_path, "./configure", &[prefix.as_str()]);

    run_command_or_fail(libdc_path, "make", &["-C", "src", "revision.h"]);

    // Build using ndk-build with environment variables passed as arguments
    let ndk_build = Path::new(&ndk_home).join("ndk-build");
    let ndk_build_cmd = if cfg!(target_os = "windows") {
        format!("{}.cmd", ndk_build.display())
    } else {
        ndk_build.display().to_string()
    };

    run_command_or_fail(
        libdc_path,
        &ndk_build_cmd,
        &[
            format!("NDK_PROJECT_PATH={}", libdc_path.display()).as_str(),
            format!(
                "APP_BUILD_SCRIPT={}",
                android_mk_path.join("Android.mk").display()
            )
            .as_str(),
            format!("APP_ABI={android_abi}").as_str(),
            "APP_PLATFORM=android-21",
            "APP_STL=c++_shared",
            "-j4",
        ],
    );

    // Copy built libraries to our lib_root and ensure proper linking setup
    let libs_path = libdc_path.join("libs").join(android_abi);
    if libs_path.exists() {
        std::fs::create_dir_all(lib_root.join("lib")).expect("Failed to create lib directory");

        // Copy the shared library that ndk-build produces
        let src_lib = libs_path.join("libdivecomputer.so");
        let dst_lib = lib_root.join("lib").join("libdivecomputer.so");

        if src_lib.exists() {
            std::fs::copy(&src_lib, &dst_lib).expect("Failed to copy libdivecomputer.so");
            println!(
                "cargo:rustc-link-search=native={}",
                lib_root.join("lib").display()
            );
            println!("cargo:rustc-link-lib=dylib=divecomputer");
        } else {
            panic!("libdivecomputer.so not found at {}", src_lib.display());
        }

        // Also copy libc++_shared.so if it exists
        let src_cpp = libs_path.join("libc++_shared.so");
        let dst_cpp = lib_root.join("lib").join("libc++_shared.so");
        if src_cpp.exists() {
            let _ = std::fs::copy(&src_cpp, &dst_cpp);
        }

        // Copy headers from the source
        let include_src = libdc_path.join("include");
        let include_dst = lib_root.join("include");
        if include_src.exists() {
            copy_directory(&include_src, &include_dst).expect("Failed to copy headers");
        }
    } else {
        panic!(
            "Android build output directory not found: {}",
            libs_path.display()
        );
    }
}

fn copy_directory(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_directory(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

fn setup_windows_build(libdc_path: &Path, lib_root: &Path, target_arch: &str) {
    // For Windows, we'll configure with USB and Bluetooth support
    let prefix = format!("--prefix={}", lib_root.display());
    let host = match target_arch {
        "x86_64" => "x86_64-w64-mingw32",
        "i686" => "i686-w64-mingw32",
        "aarch64" => "aarch64-w64-mingw32",
        _ => panic!("Unsupported Windows architecture: {target_arch}"),
    };

    // Check for MSYS2/vcpkg dependencies
    println!("cargo:rustc-link-search=native=C:/msys64/mingw64/lib");
    println!("cargo:rustc-link-search=native=C:/vcpkg/installed/x64-windows/lib");

    run_command_or_fail(
        libdc_path,
        "./configure",
        &[
            prefix.as_str(),
            format!("--host={host}").as_str(),
            "--disable-shared",
            "--enable-static",
        ],
    );
}

fn setup_macos_build(libdc_path: &Path, lib_root: &Path, target: &str) {
    let prefix = format!("--prefix={}", lib_root.display());

    // macOS-specific configuration with USB and Bluetooth
    let mut configure_args = vec![&prefix, "--disable-shared", "--enable-static"];

    // For cross-compilation on macOS, use Command with .env() instead of env::set_var
    let mut cmd = Command::new("./configure");
    cmd.current_dir(libdc_path);

    if target.contains("aarch64") {
        let cflags = "-arch arm64 -mmacosx-version-min=11.0";
        let ldflags = "-arch arm64 -mmacosx-version-min=11.0";
        cmd.env("CFLAGS", cflags);
        cmd.env("LDFLAGS", ldflags);
        configure_args.push("--host=aarch64-apple-darwin");
    } else if target.contains("x86_64") {
        let cflags = "-arch x86_64 -mmacosx-version-min=10.15";
        let ldflags = "-arch x86_64 -mmacosx-version-min=10.15";
        cmd.env("CFLAGS", cflags);
        cmd.env("LDFLAGS", ldflags);
        configure_args.push("--host=x86_64-apple-darwin");
    }

    let status = cmd
        .args(&configure_args)
        .status()
        .expect("Failed to run configure");

    if !status.success() {
        panic!("Configure failed for macOS target: {target}");
    }
}

fn setup_linux_build(libdc_path: &Path, lib_root: &Path) {
    let prefix = format!("--prefix={}", lib_root.display());

    // Linux with full USB and Bluetooth support
    run_command_with_env(
        libdc_path,
        "./configure",
        &[prefix.as_str(), "--disable-shared", "--enable-static"],
        &[("CFLAGS", "-fPIC -O2"), ("LDFLAGS", "-fPIC")],
    );
}

fn setup_link_libraries(target_os: &str, lib_root: &Path) {
    // Add our built library
    println!(
        "cargo:rustc-link-search=native={}",
        lib_root.join("lib").display()
    );

    match target_os {
        "linux" => {
            // Linux system libraries for USB and Bluetooth
            println!("cargo:rustc-link-search={}", lib_root.join("lib").display());
            println!("cargo:rustc-link-search=/usr/lib");
            println!("cargo:rustc-link-lib=dbus-1");
            println!("cargo:rustc-link-lib=usb-1.0");
            println!("cargo:rustc-link-lib=mtp");
            println!("cargo:rustc-link-lib=bluetooth");
            println!("cargo:rustc-link-lib=static=divecomputer");
        }
        "macos" => {
            // macOS frameworks for USB and Bluetooth
            println!("cargo:rustc-link-search={}", lib_root.join("lib").display());
            println!("cargo:rustc-link-lib=static=divecomputer");
            println!("cargo:rustc-link-lib=framework=IOKit");
            println!("cargo:rustc-link-lib=framework=CoreFoundation");
            println!("cargo:rustc-link-lib=framework=IOBluetooth");
            println!("cargo:rustc-link-lib=framework=CoreBluetooth");
            println!("cargo:rustc-link-lib=c++");
        }
        "ios" => {
            // iOS frameworks - primarily Bluetooth, no USB
            println!("cargo:rustc-link-search={}", lib_root.join("lib").display());
            println!("cargo:rustc-link-lib=static=divecomputer");
            println!("cargo:rustc-link-lib=framework=CoreBluetooth");
            println!("cargo:rustc-link-lib=framework=CoreFoundation");
            println!("cargo:rustc-link-lib=framework=Foundation");
            println!("cargo:rustc-link-lib=c++");
        }
        "windows" => {
            // Windows libraries for USB and Bluetooth
            println!("cargo:rustc-link-search={}", lib_root.join("lib").display());
            println!("cargo:rustc-link-lib=static=divecomputer");
            println!("cargo:rustc-link-lib=ws2_32");
            println!("cargo:rustc-link-lib=setupapi");
            println!("cargo:rustc-link-lib=winusb");
            println!("cargo:rustc-link-lib=bthprops");
            println!("cargo:rustc-link-lib=kernel32");
            println!("cargo:rustc-link-lib=user32");
            println!("cargo:rustc-link-lib=advapi32");
        }
        "android" => {
            // Android libraries - link with ndk-build output
            // For Android, we use the shared library produced by ndk-build
            println!("cargo:rustc-link-search={}", lib_root.join("lib").display());
            println!("cargo:rustc-link-lib=dylib=divecomputer");
            println!("cargo:rustc-link-lib=log");
            println!("cargo:rustc-link-lib=dylib=c++_shared");
        }
        _ => {}
    }
}

fn get_clang_args(
    target_os: &str,
    target_arch: &str,
    target: &str,
    lib_root: &Path,
) -> Vec<String> {
    let mut args = vec![
        format!("-I{}/include", lib_root.display()),
        "-v".to_string(),
    ];

    // Add target-specific clang arguments
    match target_os {
        "ios" => {
            let (sdk, arch, min_version) = match target {
                "aarch64-apple-ios" => ("iphoneos", "arm64", "11.0"),
                "x86_64-apple-ios" => ("iphonesimulator", "x86_64", "11.0"),
                "aarch64-apple-ios-sim" => ("iphonesimulator", "arm64", "14.0"),
                _ => panic!("Unsupported iOS target: {target}"),
            };

            let sdk_path = Command::new("xcrun")
                .args(["--sdk", sdk, "--show-sdk-path"])
                .output()
                .expect("Failed to get iOS SDK path")
                .stdout;
            let bindings = String::from_utf8(sdk_path).unwrap();
            let sdk_path = bindings.trim();

            args.push(format!("-arch {arch}"));
            args.push(format!("-isysroot {sdk_path}"));
            args.push(format!("-mios-version-min={min_version}"));
            args.push("-fembed-bitcode".to_string());
        }
        "android" => {
            let ndk_home = env::var("ANDROID_NDK_HOME")
                .or_else(|_| env::var("NDK_HOME"))
                .expect("ANDROID_NDK_HOME required for Android");

            let host_tag = if cfg!(target_os = "windows") {
                "windows-x86_64"
            } else if cfg!(target_os = "macos") {
                "darwin-x86_64"
            } else {
                "linux-x86_64"
            };

            let sysroot = format!("{ndk_home}/toolchains/llvm/prebuilt/{host_tag}/sysroot");
            args.push(format!("--sysroot={sysroot}"));

            match target_arch {
                "aarch64" => {
                    args.push("-target".to_string());
                    args.push("aarch64-linux-android21".to_string());
                }
                "arm" => {
                    args.push("-target".to_string());
                    args.push("armv7a-linux-androideabi16".to_string());
                }
                "x86_64" => {
                    args.push("-target".to_string());
                    args.push("x86_64-linux-android21".to_string());
                }
                "x86" => {
                    args.push("-target".to_string());
                    args.push("i686-linux-android16".to_string());
                }
                _ => {}
            }
        }
        "windows" => match target_arch {
            "x86_64" => {
                args.push("-target".to_string());
                args.push("x86_64-pc-windows-gnu".to_string());
            }
            "i686" => {
                args.push("-target".to_string());
                args.push("i686-pc-windows-gnu".to_string());
            }
            _ => {}
        },
        "macos" => {
            if target.contains("aarch64") {
                args.push("-arch".to_string());
                args.push("arm64".to_string());
                args.push("-mmacosx-version-min=11.0".to_string());
            } else if target.contains("x86_64") {
                args.push("-arch".to_string());
                args.push("x86_64".to_string());
                args.push("-mmacosx-version-min=10.15".to_string());
            }
        }
        _ => {}
    }

    args
}

fn main() -> std::io::Result<()> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("output directory not specified"));
    let (target, target_os, target_arch) = get_target_info();

    println!(
        "Building for target: {target} (OS: {target_os}, Arch: {target_arch})"
    );

    // cargo-xbuild specific environment setup
    if env::var("CARGO_XBUILD").is_ok() {
        println!("cargo:rustc-env=CARGO_XBUILD=1");
        setup_xbuild_environment(&target, &target_os);
    }

    let libdc_path = out_dir.join("libdivecomputer");
    let lib_root = out_dir.join("libdc");

    // Copy libdivecomputer source
    let copy_cmd = if cfg!(target_os = "windows") {
        "xcopy"
    } else {
        "cp"
    };
    let libdc_path_disp = libdc_path.display().to_string();
    let copy_args = if cfg!(target_os = "windows") {
        vec!["/E", "/I", "libdivecomputer", libdc_path_disp.as_str()]
    } else {
        vec!["-av", "libdivecomputer/.", libdc_path_disp.as_str()]
    };

    run_command_or_fail(".", copy_cmd, &copy_args);

    // Run autoreconf if configure doesn't exist
    if !std::fs::exists(libdc_path.join("configure"))? {
        run_command_or_fail(&libdc_path, "autoreconf", &["--install"]);
    }

    // Platform-specific build configuration
    match target_os.as_str() {
        "android" => {
            setup_android_build(&libdc_path, &lib_root, &target);
            // Android uses ndk-build, so we skip the autotools build process
        }
        "ios" => setup_ios_build(&libdc_path, &lib_root, &target),
        "windows" => setup_windows_build(&libdc_path, &lib_root, &target_arch),
        "macos" => setup_macos_build(&libdc_path, &lib_root, &target),
        "linux" => setup_linux_build(&libdc_path, &lib_root),
        _ => panic!("Unsupported target OS: {target_os}"),
    }

    // Build the library (skip for Android as ndk-build handles everything)
    if target_os != "android" {
        let make_cmd = if target_os == "windows" {
            "mingw32-make"
        } else {
            "make"
        };
        run_command_or_fail(&libdc_path, make_cmd, &[""]);
        run_command_or_fail(&libdc_path, make_cmd, &["install"]);
    }

    // Set up linking
    setup_link_libraries(&target_os, &lib_root);

    // Generate bindings with cargo-xbuild considerations
    generate_bindings(&target_os, &target_arch, &target, &lib_root, &out_dir)?;

    Ok(())
}

fn setup_xbuild_environment(target: &str, target_os: &str) {
    println!("cargo:rustc-env=XBUILD_TARGET={target}");

    // Set up xbuild-specific environment variables via cargo:rustc-env
    match target_os {
        "android" => {
            if let Ok(ndk_home) = env::var("ANDROID_NDK_HOME") {
                println!("cargo:rustc-env=XBUILD_ANDROID_NDK={ndk_home}");
            }
        }
        "ios" => {
            println!("cargo:rustc-env=XBUILD_IOS=1");
        }
        _ => {}
    }
}

fn generate_bindings(
    target_os: &str,
    target_arch: &str,
    target: &str,
    lib_root: &Path,
    out_dir: &Path,
) -> std::io::Result<()> {
    #[derive(Debug)]
    struct CB;

    impl ParseCallbacks for CB {
        fn item_name(&self, item_info: ItemInfo<'_>) -> Option<String> {
            match item_info.name {
                "SAMPLE_EVENT_STRING" => Some("SAMPLE_EVENT_STRING_DEFAULT".to_string()),
                "DC_TRANSPORT_USBSTORAGE" => Some("DC_TRANSPORT_USBSTORAGE_DEFAULT".to_string()),
                "DC_SAMPLE_TTS" => Some("DC_SAMPLE_TTS_DEFAULT".to_string()),
                "DC_FIELD_STRING" => Some("DC_FIELD_STRING_DEFAULT".to_string()),
                _ => None,
            }
        }
    }

    let clang_args = get_clang_args(target_os, target_arch, target, lib_root);

    let mut builder = bindgen::Builder::default()
        .header("wrapper.h")
        .wrap_unsafe_ops(true)
        .prepend_enum_name(false)
        .parse_callbacks(Box::new(CB))
        .clang_macro_fallback()
        // cargo-xbuild specific settings
        .layout_tests(false) // Disable layout tests for cross-compilation
        .derive_debug(true)
        .derive_default(true);

    // Add clang arguments
    for arg in clang_args {
        builder = builder.clang_arg(arg);
    }

    let bindings = builder.generate().expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    Ok(())
}
