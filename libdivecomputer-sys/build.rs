use std::borrow::Borrow;
use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use bindgen::callbacks::{ItemInfo, ParseCallbacks};

fn main() -> std::io::Result<()> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("output directory not specified"));
    let (target, target_os, target_arch) = get_target_info();

    println!("Building for target: {target} (OS: {target_os}, Arch: {target_arch})");

    // cargo-xbuild specific environment setup
    if env::var("CARGO_XBUILD").is_ok() {
        println!("cargo:rustc-env=CARGO_XBUILD=1");
        setup_xbuild_environment(&target, &target_os);
    }

    let libdc_path = out_dir.join("libdivecomputer");
    let lib_root = out_dir.join("libdc");

    // Use cross-platform copy_directory instead of Unix-only `cp -av`
    copy_directory(Path::new("libdivecomputer"), &libdc_path)?;

    // Windows doesn't have autotools — skip autoreconf/configure/make entirely
    if target_os != "windows" {
        if !std::fs::exists(libdc_path.join("configure"))? {
            run_command(&libdc_path, "autoreconf", &["--install"]);
        }
    }

    match target_os.as_str() {
        "android" => {
            setup_android_build(&libdc_path, &lib_root, &target);
            // Android uses ndk-build, so we skip the autotools build process
        }
        "linux" => setup_linux_build(&libdc_path, &lib_root),
        "macos" => setup_macos_build(&libdc_path, &lib_root),
        "ios" => setup_ios_build(&libdc_path, &lib_root, &target),
        "windows" => setup_windows_build(&libdc_path, &lib_root)?,
        _ => panic!("Unsupported target OS: {target_os}"),
    }

    // Build the library via autotools (skip for Android/Windows which use their own build systems)
    if target_os != "android" && target_os != "windows" {
        run_command(&libdc_path, "make", &[""]);
        run_command(&libdc_path, "make", &["install"]);
    }

    setup_link_libraries(&target_os, &lib_root);

    generate_bindings(&target_os, &target_arch, &lib_root, &out_dir)?;

    Ok(())
}

fn run_command<C, P, S>(dir: C, cmd: P, args: &[S])
where
    C: AsRef<Path>,
    P: AsRef<Path>,
    S: Borrow<str> + AsRef<OsStr>,
{
    run_command_with_env(dir, cmd, args, &[]);
}

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
    run_command(libdc_path, "./configure", &[prefix.as_str()]);

    run_command(libdc_path, "make", &["-C", "src", "revision.h"]);

    // Build using ndk-build with environment variables passed as arguments
    let ndk_build = Path::new(&ndk_home).join("ndk-build");
    let ndk_build_cmd = if cfg!(target_os = "windows") {
        format!("{}.cmd", ndk_build.display())
    } else {
        ndk_build.display().to_string()
    };

    run_command(
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

fn setup_macos_build(libdc_path: &Path, lib_root: &Path) {
    let prefix = format!("--prefix={}", lib_root.display());

    // macOS with static build, IOKit serial support auto-detected by configure
    run_command_with_env(
        libdc_path,
        "./configure",
        &[
            prefix.as_str(),
            "--disable-shared",
            "--enable-static",
            "--without-bluez",
        ],
        &[("CFLAGS", "-fPIC -O2"), ("LDFLAGS", "-fPIC")],
    );
}

fn setup_ios_build(libdc_path: &Path, lib_root: &Path, target: &str) {
    let prefix = format!("--prefix={}", lib_root.display());

    // Determine SDK and host triple
    let (sdk, host_triple) = if target.contains("sim") {
        (
            "iphonesimulator",
            format!(
                "{}-apple-darwin",
                target.split('-').next().unwrap_or("aarch64")
            ),
        )
    } else {
        (
            "iphoneos",
            format!(
                "{}-apple-darwin",
                target.split('-').next().unwrap_or("aarch64")
            ),
        )
    };

    // Get SDK path via xcrun
    let sdk_path = String::from_utf8(
        Command::new("xcrun")
            .args(["--sdk", sdk, "--show-sdk-path"])
            .output()
            .expect("Failed to run xcrun")
            .stdout,
    )
    .expect("Invalid UTF-8 from xcrun")
    .trim()
    .to_string();

    let cc = String::from_utf8(
        Command::new("xcrun")
            .args(["--sdk", sdk, "--find", "clang"])
            .output()
            .expect("Failed to find clang via xcrun")
            .stdout,
    )
    .expect("Invalid UTF-8 from xcrun")
    .trim()
    .to_string();

    let cflags = format!(
        "-fPIC -O2 -isysroot {sdk_path} -arch {}",
        target.split('-').next().unwrap_or("arm64")
    );
    let host_arg = format!("--host={host_triple}");

    run_command_with_env(
        libdc_path,
        "./configure",
        &[
            prefix.as_str(),
            "--disable-shared",
            "--enable-static",
            "--without-libusb",
            "--without-hidapi",
            "--without-bluez",
            host_arg.as_str(),
        ],
        &[
            ("CC", &cc),
            ("CFLAGS", &cflags),
            ("LDFLAGS", &format!("-fPIC -isysroot {sdk_path}")),
        ],
    );
}

fn setup_windows_build(libdc_path: &Path, lib_root: &Path) -> std::io::Result<()> {
    // On Windows we skip autotools entirely and use the cc crate to compile all C sources.
    // This mirrors what the MSVC .vcxproj does.

    let src_dir = libdc_path.join("src");
    let include_dir = libdc_path.join("include");

    // Create output directories
    let lib_dir = lib_root.join("lib");
    let inc_dir = lib_root.join("include");
    std::fs::create_dir_all(&lib_dir)?;

    // Generate headers before copying so they're included in the copy
    generate_config_h(&src_dir)?;
    generate_version_h(&include_dir)?;
    generate_revision_h(libdc_path, &src_dir)?;

    copy_directory(&include_dir, &inc_dir)?;

    // Collect all C source files (same list as the .vcxproj, using serial_win32 instead of serial_posix)
    let mut sources: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(&src_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("c") {
            let name = path.file_name().unwrap().to_str().unwrap();
            // Skip the POSIX serial implementation (we use serial_win32.c on Windows)
            if name == "serial_posix.c" {
                continue;
            }
            sources.push(path);
        }
    }

    let mut build = cc::Build::new();
    build
        .include(&include_dir)
        .include(&src_dir) // for config.h, revision.h, internal headers
        .define("ENABLE_LOGGING", None)
        .define("HAVE_VERSION_SUFFIX", None)
        .define("HAVE_AF_IRDA_H", None)
        .define("HAVE_WS2BTH_H", None)
        .define("HAVE__MKGMTIME", None)
        .define("_CRT_SECURE_NO_WARNINGS", None)
        .warnings(false);

    // Optional libusb support via environment variable
    // Supports vcpkg layout (include/libusb-1.0/libusb.h) and manual installs
    if let Ok(libusb_dir) = env::var("LIBUSB_DIR") {
        build.define("HAVE_LIBUSB", None);
        let libusb_include = PathBuf::from(&libusb_dir).join("include");
        // libdivecomputer includes <libusb.h> directly, but vcpkg/releases
        // put it under include/libusb-1.0/libusb.h
        let nested = libusb_include.join("libusb-1.0");
        if nested.exists() {
            build.include(&nested);
        }
        build.include(&libusb_include);
        // Check for VS2022 static lib layout (official releases)
        let vs_lib = PathBuf::from(&libusb_dir).join("VS2022").join("MS64").join("static");
        if vs_lib.exists() {
            println!("cargo:rustc-link-search=native={}", vs_lib.display());
        }
        println!("cargo:rustc-link-search=native={libusb_dir}/lib");
        println!("cargo:rustc-link-lib=static=libusb-1.0");
    }

    // Optional hidapi support via environment variable
    // Supports vcpkg layout (include/hidapi/hidapi.h) and manual installs
    if let Ok(hidapi_dir) = env::var("HIDAPI_DIR") {
        build.define("HAVE_HIDAPI", None);
        let hidapi_include = PathBuf::from(&hidapi_dir).join("include");
        // libdivecomputer includes <hidapi.h> directly, but vcpkg/releases
        // put it under include/hidapi/hidapi.h
        let nested = hidapi_include.join("hidapi");
        if nested.exists() {
            build.include(&nested);
        }
        build.include(&hidapi_include);
        // Check for x64 lib layout (official releases)
        let x64_lib = PathBuf::from(&hidapi_dir).join("x64");
        if x64_lib.exists() {
            println!("cargo:rustc-link-search=native={}", x64_lib.display());
        }
        println!("cargo:rustc-link-search=native={hidapi_dir}/lib");
        println!("cargo:rustc-link-lib=static=hidapi");
    }

    for src in &sources {
        build.file(src);
    }

    build.compile("divecomputer");

    Ok(())
}

fn generate_config_h(src_dir: &Path) -> std::io::Result<()> {
    // Minimal config.h for Windows — mirrors what autotools would detect on MSVC
    let config = r#"/* config.h - Generated by build.rs for Windows */
#ifndef CONFIG_H
#define CONFIG_H

/* Enable logging support */
#define ENABLE_LOGGING 1

/* Version suffix present */
#define HAVE_VERSION_SUFFIX 1

/* Windows IrDA support via af_irda.h */
#define HAVE_AF_IRDA_H 1

/* Windows Bluetooth support via ws2bth.h */
#define HAVE_WS2BTH_H 1

/* Windows has _mkgmtime */
#define HAVE__MKGMTIME 1

#endif /* CONFIG_H */
"#;
    std::fs::write(src_dir.join("config.h"), config)
}

fn generate_version_h(include_dir: &Path) -> std::io::Result<()> {
    // Read version numbers from configure.ac (they're m4_define'd at the top)
    let version_h_in =
        std::fs::read_to_string(include_dir.join("libdivecomputer").join("version.h.in"))?;

    let version_h = version_h_in
        .replace("@DC_VERSION@", "0.10.0-Divr")
        .replace("@DC_VERSION_MAJOR@", "0")
        .replace("@DC_VERSION_MINOR@", "10")
        .replace("@DC_VERSION_MICRO@", "0");

    std::fs::write(
        include_dir.join("libdivecomputer").join("version.h"),
        version_h,
    )
}

fn generate_revision_h(libdc_path: &Path, src_dir: &Path) -> std::io::Result<()> {
    // Try to read the revision file, fall back to empty string
    let revision = std::fs::read_to_string(libdc_path.join("revision"))
        .unwrap_or_default()
        .trim()
        .to_string();

    std::fs::write(
        src_dir.join("revision.h"),
        format!("#define DC_VERSION_REVISION \"{revision}\"\n"),
    )
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
        "android" => {
            // Android libraries - link with ndk-build output
            // For Android, we use the shared library produced by ndk-build
            println!("cargo:rustc-link-search={}", lib_root.join("lib").display());
            println!("cargo:rustc-link-lib=dylib=divecomputer");
            println!("cargo:rustc-link-lib=log");
            println!("cargo:rustc-link-lib=dylib=c++_shared");
        }
        "macos" => {
            println!("cargo:rustc-link-search={}", lib_root.join("lib").display());
            println!("cargo:rustc-link-lib=static=divecomputer");
            // macOS frameworks for serial/USB
            println!("cargo:rustc-link-lib=framework=IOKit");
            println!("cargo:rustc-link-lib=framework=CoreFoundation");
            // Optional: libusb/hidapi if installed (e.g. via Homebrew)
            if env::var("LIBUSB_DIR").is_ok() || pkg_config_exists("libusb-1.0") {
                println!("cargo:rustc-link-lib=usb-1.0");
            }
            if env::var("HIDAPI_DIR").is_ok() || pkg_config_exists("hidapi") {
                println!("cargo:rustc-link-lib=hidapi");
            }
        }
        "ios" => {
            println!("cargo:rustc-link-search={}", lib_root.join("lib").display());
            println!("cargo:rustc-link-lib=static=divecomputer");
            println!("cargo:rustc-link-lib=framework=CoreFoundation");
        }
        "windows" => {
            // cc crate already emits cargo:rustc-link-lib=static=divecomputer
            // We just need the Windows system libraries
            println!("cargo:rustc-link-lib=ws2_32");
            println!("cargo:rustc-link-lib=setupapi");
        }
        _ => {}
    }
}

fn pkg_config_exists(lib: &str) -> bool {
    Command::new("pkg-config")
        .args(["--exists", lib])
        .status()
        .is_ok_and(|s| s.success())
}

fn get_clang_args(target_os: &str, target_arch: &str, lib_root: &Path) -> Vec<String> {
    let mut args = vec![
        format!("-I{}/include", lib_root.display()),
        "-v".to_string(),
    ];

    // Add target-specific clang arguments
    match target_os {
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
        "windows" => {
            // MSVC uses signed enums by default, but libdivecomputer's Rust wrapper expects
            // unsigned enum types. Use a Linux target hint so bindgen generates unsigned enums.
            args.push("--target=x86_64-unknown-linux-gnu".to_string());
        }
        _ => {}
    }

    args
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

fn setup_xbuild_environment(target: &str, target_os: &str) {
    println!("cargo:rustc-env=XBUILD_TARGET={target}");

    if target_os == "android" {
        if let Ok(ndk_home) = env::var("ANDROID_NDK_HOME") {
            println!("cargo:rustc-env=XBUILD_ANDROID_NDK={ndk_home}");
        }
    }
}

fn generate_bindings(
    target_os: &str,
    target_arch: &str,
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

    let clang_args = get_clang_args(target_os, target_arch, lib_root);

    let mut builder = bindgen::Builder::default()
        .header("wrapper.h")
        .wrap_unsafe_ops(true)
        .prepend_enum_name(false)
        .parse_callbacks(Box::new(CB))
        .clang_macro_fallback()
        .layout_tests(false)
        .derive_debug(true)
        .derive_default(true);

    for arg in clang_args {
        builder = builder.clang_arg(arg);
    }

    let bindings = builder.generate().expect("Unable to generate bindings");

    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    Ok(())
}
