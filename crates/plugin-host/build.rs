use std::{env, path::PathBuf, process::Command};

fn run(command: &mut Command) {
    let result = command.output().expect("run native bridge build");
    assert!(
        result.status.success(),
        "native bridge build failed:\n{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

fn main() {
    assert_eq!(
        env::var("CARGO_CFG_TARGET_OS").unwrap(),
        "macos",
        "T02 SDK bridge currently targets macOS"
    );
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("vst3");
    run(Command::new("cmake")
        .arg("-S")
        .arg(root.join("native"))
        .arg("-B")
        .arg(&out)
        .args([
            "-DCMAKE_BUILD_TYPE=Release",
            "-DCMAKE_OSX_ARCHITECTURES=arm64",
        ]));
    run(Command::new("cmake").arg("--build").arg(&out).args([
        "--target",
        "clef_vst3",
        "--parallel",
        "4",
    ]));
    println!(
        "cargo:rustc-link-search=native={}",
        out.join("lib").display()
    );
    println!("cargo:rustc-link-lib=static=clef_vst3");
    println!("cargo:rustc-link-lib=c++");
    println!("cargo:rustc-link-lib=framework=Cocoa");
    println!("cargo:rustc-link-lib=framework=QuartzCore");
    println!("cargo:rustc-link-lib=framework=CoreFoundation");
    println!("cargo:rerun-if-changed=../../native");
    println!("cargo:rerun-if-changed=../../vendor/vst3sdk");
}
