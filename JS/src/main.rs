use std::{env, fs, process};
use bumblebee_js::Runtime;

fn main() {
    let Some(path) = env::args().nth(1) else {
        eprintln!("Usage: bumblebee-js <script.js>");
        process::exit(2);
    };
    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("Cannot read {path}: {error}");
            process::exit(1);
        }
    };

    let runtime = match Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("{error}");
            process::exit(1);
        }
    };
    match runtime.execute(&source) {
        Ok(result) => println!("{result}"),
        Err(error) => {
            eprintln!("{error}");
            process::exit(1);
        }
    }
}
