use std::borrow::Borrow;
use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use bindgen::callbacks::{ItemInfo, ParseCallbacks};

fn run_command_or_fail<C, P, S>(dir: C, cmd: P, args: &[S])
where
    C: AsRef<Path>,
    P: AsRef<Path>,
    S: Borrow<str> + AsRef<OsStr>,
{
    let cmd = cmd.as_ref();
    let cmd = if cmd.components().count() > 1 && cmd.is_relative() {
        // If `cmd` is a relative path (and not a bare command that should be
        // looked up in PATH), absolutize it relative to `dir`, as otherwise the
        // behavior of std::process::Command is undefined.
        // https://github.com/rust-lang/rust/issues/37868
        dir.as_ref()
            .join(cmd)
            .canonicalize()
            .expect("canonicalization failed")
    } else {
        PathBuf::from(cmd)
    };
    eprintln!(
        "Running command: \"{} {}\" in dir: {}",
        cmd.display(),
        args.join(" "),
        dir.as_ref().display()
    );
    let ret = Command::new(cmd).current_dir(dir).args(args).status();
    match ret.map(|status| (status.success(), status.code())) {
        Ok((true, _)) => (),
        Ok((false, Some(c))) => panic!("Command failed with error code {}", c),
        Ok((false, None)) => panic!("Command got killed"),
        Err(e) => panic!("Command failed with error: {}", e),
    }
}

fn main() -> std::io::Result<()> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("output directory not specified"));

    let libdc_path = out_dir.join("libdivecomputer");

    run_command_or_fail(
        ".",
        "cp",
        &[
            "-av",
            "libdivecomputer/.",
            &libdc_path.display().to_string(),
        ],
    );

    let lib_root = out_dir.join("libdc");

    println!("cargo:rustc-link-search={}", lib_root.join("lib").display());
    println!("cargo:rustc-link-search=/usr/lib");
    println!("cargo:rustc-link-lib=dbus-1");
    println!("cargo:rustc-link-lib=usb-1.0");
    println!("cargo:rustc-link-lib=mtp");
    println!("cargo:rustc-link-lib=bluetooth");
    println!("cargo:rustc-link-lib=divecomputer");

    if !std::fs::exists(libdc_path.join("configure"))? {
        run_command_or_fail(&libdc_path, "autoreconf", &["--install"]);
    }

    let prefix = &format!("--prefix={}", lib_root.display());
    run_command_or_fail(
        &libdc_path,
        "./configure",
        &[prefix.as_str(), "--disable-shared"],
    );

    run_command_or_fail(&libdc_path, "make", &[""]);
    run_command_or_fail(&libdc_path, "make", &["install"]);

    #[derive(Debug)]
    struct CB;

    impl ParseCallbacks for CB {
        fn item_name(&self, item_info: ItemInfo<'_>) -> Option<String> {
            // Prevent collision of constants, we can probably skip these, I'll have to investigate
            // the bindgen docs
            match item_info.name {
                "SAMPLE_EVENT_STRING" => Some("SAMPLE_EVENT_STRING_DEFAULT".to_string()),
                "DC_TRANSPORT_USBSTORAGE" => Some("DC_TRANSPORT_USBSTORAGE_DEFAULT".to_string()),
                "DC_SAMPLE_TTS" => Some("DC_SAMPLE_TTS_DEFAULT".to_string()),
                "DC_FIELD_STRING" => Some("DC_FIELD_STRING_DEFAULT".to_string()),
                _ => None,
            }
        }
    }

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .wrap_unsafe_ops(true)
        .prepend_enum_name(false)
        .clang_arg(format!("-I{}/include", lib_root.display()))
        .clang_arg("-v")
        .parse_callbacks(Box::new(CB))
        .clang_macro_fallback()
        .generate()
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    Ok(())
}
