use std::env;
use std::fs;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: {} <input.docx> <output.pdf>", args[0]);
        return ExitCode::from(2);
    }
    let input = match fs::read(&args[1]) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("read {}: {}", args[1], e);
            return ExitCode::from(1);
        }
    };
    match approach_c_rdocx_opt3::convert(&input) {
        Ok(pdf) => {
            if let Err(e) = fs::write(&args[2], &pdf) {
                eprintln!("write {}: {}", args[2], e);
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("convert: {e}");
            ExitCode::from(1)
        }
    }
}
