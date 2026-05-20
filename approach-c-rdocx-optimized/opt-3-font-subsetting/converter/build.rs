//! Build-time font subsetting via pyftsubset.
//!
//! For each TTF in the source dir, run pyftsubset to drop everything outside
//! Latin Unicode coverage + common punctuation/currency. Writes outputs to
//! `$OUT_DIR/fonts/<Name>.ttf` so src/lib.rs can include_bytes! them.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// All TTFs shipped by rdocx-layout 0.1.2.
const FONTS: &[&str] = &[
    "Carlito-Regular",
    "Carlito-Bold",
    "Carlito-Italic",
    "Carlito-BoldItalic",
    "Caladea-Regular",
    "Caladea-Bold",
    "Caladea-Italic",
    "Caladea-BoldItalic",
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

// Latin (Basic + Latin-1 + Ext-A + Ext-B) + general punctuation + currency + super/subscript.
const UNICODE_RANGES: &str = "U+0000-024F,U+2000-206F,U+20A0-20CF,U+2070-209F";

fn find_src_dir() -> PathBuf {
    // Allow override.
    if let Ok(p) = env::var("RDOCX_FONTS_DIR") {
        return PathBuf::from(p);
    }
    // Default: rdocx-layout-0.1.2 in the cargo registry next to this crate.
    // Walk up cargo-home candidates.
    let home = env::var("CARGO_HOME")
        .ok()
        .or_else(|| env::var("HOME").ok().map(|h| format!("{h}/.cargo")))
        .expect("HOME or CARGO_HOME must be set");
    let registry = Path::new(&home).join("registry/src");
    let entries = fs::read_dir(&registry).expect("read cargo registry src");
    for e in entries.flatten() {
        let candidate = e.path().join("rdocx-layout-0.1.2/fonts");
        if candidate.is_dir() {
            return candidate;
        }
    }
    panic!("could not locate rdocx-layout-0.1.2/fonts under {registry:?}");
}

fn find_pyftsubset() -> String {
    if let Ok(p) = env::var("PYFTSUBSET") {
        return p;
    }
    // Try common locations.
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
    // ~/Library/Python/*/bin/pyftsubset (macOS user installs).
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
