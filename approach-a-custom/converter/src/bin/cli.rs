use std::env;
use std::fs;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: {} INPUT.docx OUTPUT.pdf", args[0]);
        return ExitCode::from(2);
    }
    let input = &args[1];
    let output = &args[2];

    let bytes = match fs::read(input) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("read {input}: {e}");
            return ExitCode::from(1);
        }
    };

    match approach_a_custom::convert(&bytes) {
        Ok(pdf) => {
            if let Err(e) = fs::write(output, pdf) {
                eprintln!("write {output}: {e}");
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
