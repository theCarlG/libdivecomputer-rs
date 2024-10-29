use std::env;
use std::path::{self, PathBuf};

fn main() -> std::io::Result<()> {
    let pwd = path::absolute("./").unwrap();
    let libdc_path = pwd.join("libdivecomputer");
    let lib_root = pwd.join("libdc");
    // Tell cargo to look for shared libraries in the specified directory
    println!("cargo:rustc-link-search={}", lib_root.join("lib").display());

    // Tell cargo to tell rustc to link the system libdivecomputer.
    println!("cargo:rustc-link-lib=divecomputer");

    if !std::fs::exists(libdc_path.join("configure"))? {
        if !std::process::Command::new("autoreconf")
            .arg("--install")
            .current_dir(&libdc_path)
            .output()
            .expect("could not spawn `autoreconf`")
            .status
            .success()
        {
            panic!("could not run autoreconf");
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
    }

    if !std::fs::exists(lib_root.join("lib/libdivecomputer.a"))? {
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
    }

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header("wrapper.h")
        .clang_arg(format!("-I{}/libdc/include", pwd.display()))
        .clang_arg("-v")
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    Ok(())
}
