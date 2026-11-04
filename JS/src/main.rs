use std::{env, fs, process};
use bumblebee_js::{compile_aot, Runtime};

fn main() {
    let mut args = env::args().skip(1);
    let first = args.next();
    let aot = first.as_deref() == Some("--aot");
    let path = if aot { args.next() } else { first };
    let Some(path) = path else {
        eprintln!("Usage: bumblebee-js [--aot] <script.js>");
        process::exit(2);
    };
    let source = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => { eprintln!("Cannot read {path}: {e}"); process::exit(1); }
    };

    if aot {
        match compile_aot(&source).and_then(|program| program.run()) {
            Ok(value) => println!("{value:?}"),
            Err(error) => { eprintln!("AOT compile/runtime error: {error}"); process::exit(1); }
        }
    } else {
        // Kept temporarily while the remaining language constructs are lowered
        // into bytecode. New AOT support is exercised with --aot.
        let mut runtime = Runtime::new();
        match runtime.execute(&source) {
            Ok(value) => println!("{value:?}"),
            Err(error) => { eprintln!("{error}"); process::exit(1); }
        }
    }
}
