//! opt-6 build script: subset Liberation TTFs via pyftsubset.
//!
//! Stacks opt-3's subsetting pipeline onto opt-5's Liberation-only bundle.
//! Only the 12 Liberation faces (Sans/Serif/Mono × 4 styles) are subset.
//! Source fonts are taken from this crate's checked-in `fonts/` dir, which
//! were copied from rdocx-layout 0.1.2's bundled fonts.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// The 12 Liberation faces bundled in opt-5 (Carlito/Caladea deliberately excluded).
const FONTS: &[&str] = &[
    "LiberationSans-Regular",
    "LiberationSans-Bold",
    "LiberationSans-Italic",
    "LiberationSans-BoldItalic",
    "LiberationSerif-Regular",
    "LiberationSerif-Bold",
    "LiberationSerif-Italic",
    "LiberationSerif-BoldItalic",
    "LiberationMono-Regular",
    "LiberationMono-Bold",
    "LiberationMono-Italic",
    "LiberationMono-BoldItalic",
];

// Same coverage opt-3 used: Latin (Basic + Latin-1 + Ext-A + Ext-B) + general
// punctuation + currency + super/subscript.
const UNICODE_RANGES: &str = "U+0000-024F,U+2000-206F,U+20A0-20CF,U+2070-209F";

fn find_src_dir() -> PathBuf {
    if let Ok(p) = env::var("RDOCX_FONTS_DIR") {
        return PathBuf::from(p);
    }
    // Default: the `fonts/` dir alongside this build.rs (carried over from opt-5).
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let local = Path::new(&manifest).join("fonts");
    if local.is_dir() {
        return local;
    }
    panic!("could not locate Liberation fonts dir; expected {local:?}");
}

fn find_pyftsubset() -> String {
    if let Ok(p) = env::var("PYFTSUBSET") {
        return p;
    }
    let candidates = [
        "pyftsubset",
        "/opt/homebrew/bin/pyftsubset",
        "/usr/local/bin/pyftsubset",
    ];
    for c in candidates {
        if Command::new(c).arg("--help").output().is_ok() {
            return c.to_string();
        }
    }
    if let Ok(home) = env::var("HOME") {
        let py_root = Path::new(&home).join("Library/Python");
        if let Ok(entries) = fs::read_dir(&py_root) {
            for e in entries.flatten() {
                let p = e.path().join("bin/pyftsubset");
                if p.is_file() {
                    return p.to_string_lossy().into_owned();
                }
            }
        }
    }
    panic!("pyftsubset not found; install via `pip3 install --user fonttools brotli` or set PYFTSUBSET");
}

fn main() {
    let src_dir = find_src_dir();
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let fonts_out = out_dir.join("fonts");
    fs::create_dir_all(&fonts_out).expect("create OUT_DIR/fonts");

    let pyft = find_pyftsubset();

    for name in FONTS {
        let src = src_dir.join(format!("{name}.ttf"));
        let dst = fonts_out.join(format!("{name}.ttf"));
        println!("cargo:rerun-if-changed={}", src.display());

        let status = Command::new(&pyft)
            .arg(&src)
            .arg(format!("--output-file={}", dst.display()))
            .arg(format!("--unicodes={UNICODE_RANGES}"))
            .arg("--layout-features=*")
            .arg("--no-hinting")
            .arg("--desubroutinize")
            .arg("--drop-tables+=FFTM")
            .status()
            .expect("spawn pyftsubset");
        assert!(status.success(), "pyftsubset failed on {name}");

        let in_sz = fs::metadata(&src).map(|m| m.len()).unwrap_or(0);
        let out_sz = fs::metadata(&dst).map(|m| m.len()).unwrap_or(0);
        println!(
            "cargo:warning=subset {name}: {in_sz} -> {out_sz} bytes ({}%)",
            if in_sz > 0 { out_sz * 100 / in_sz } else { 0 }
        );
    }

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=PYFTSUBSET");
    println!("cargo:rerun-if-env-changed=RDOCX_FONTS_DIR");
}
