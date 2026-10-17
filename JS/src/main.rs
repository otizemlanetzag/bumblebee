use std::{env, fs, process};
use bumblebee_js::Runtime;

fn main() {
    let mut args = env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("Usage: bumblebee-js <script.js>");
        process::exit(2);
    };
    let source = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => { eprintln!("Cannot read {path}: {e}"); process::exit(1); }
    };
    let mut runtime = Runtime::new();
    match runtime.execute(&source) {
        Ok(value) => println!("{}", match value {
            bumblebee_js::Value::Undefined => "undefined".to_string(),
            other => format!("{other:?}"),
        }),
        Err(error) => { eprintln!("{error}"); process::exit(1); }
    }
}
