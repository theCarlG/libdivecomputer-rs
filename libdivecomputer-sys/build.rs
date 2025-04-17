use std::borrow::Borrow;
use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

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

    println!("Cloning libdivecomputer");
    run_command_or_fail(
        ".",
        "cp",
        &["-a", "libdivecomputer/.", &libdc_path.display().to_string()],
    );

    let lib_root = out_dir.join("libdc");

    // Tell cargo to look for shared libraries in the specified directory
    println!("cargo:rustc-link-search={}", lib_root.join("lib").display());

    // Tell cargo to tell rustc to link the system libdivecomputer.
    println!("cargo:rustc-link-lib=divecomputer");

    if !std::fs::exists(libdc_path.join("configure"))? {
        run_command_or_fail(&libdc_path, "autoreconf", &["--install"]);
    }

    if !std::process::Command::new("./configure")
        .arg(format!("--prefix={}", lib_root.display()))
        .arg("--disable-shared")
        .current_dir(&libdc_path)
        .output()
        .expect("could not execute `configure`")
        .status
        .success()
    {
        panic!("could not configure libdivecomputer");
    }

    // if !std::fs::exists(lib_root.join("lib/libdivecomputer.a"))? {
    if !std::process::Command::new("make")
        .current_dir(&libdc_path)
        .output()
        .expect("could not exec `make`")
        .status
        .success()
    {
        panic!("could not compile libdivecomputer");
    }

    if !std::process::Command::new("make")
        .arg("install")
        .current_dir(&libdc_path)
        .output()
        .expect("could not exec `make`")
        .status
        .success()
    {
        panic!("could not install library files");
    }
    // }

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header("wrapper.h")
        .clang_arg(format!("-I{}/include", lib_root.display()))
        .clang_arg("-v")
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    Ok(())
}
