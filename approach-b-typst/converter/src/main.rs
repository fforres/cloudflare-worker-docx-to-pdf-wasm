//! Native CLI for approach-b: `approach-b-converter INPUT.docx OUTPUT.pdf`.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: {} INPUT.docx OUTPUT.pdf", args[0]);
        return ExitCode::from(2);
    }
    let input = PathBuf::from(&args[1]);
    let output = PathBuf::from(&args[2]);

    let bytes = match std::fs::read(&input) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: reading {:?}: {}", input, e);
            return ExitCode::from(1);
        }
    };

    let pdf = match approach_b_converter::convert_docx(&bytes) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: convert: {}", e);
            return ExitCode::from(1);
        }
    };

    if let Err(e) = std::fs::write(&output, &pdf) {
        eprintln!("error: writing {:?}: {}", output, e);
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
