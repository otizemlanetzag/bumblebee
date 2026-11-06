use bumblebee_renderer::{Color, Framebuffer, Palette};
use std::env;
use std::path::Path;

fn usage() {
    eprintln!("Bumblebee pixel renderer");
    eprintln!("  bumblebee-renderer generate-palette <palette.png>");
    eprintln!("  bumblebee-renderer demo <palette.png> <output.png>");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("generate-palette") if args.len() == 3 => {
            match Palette::generate_png(&args[2]) {
                Ok(()) => println!("Created complete RGB palette: {}", args[2]),
                Err(error) => fail(error),
            }
        }
        Some("demo") if args.len() == 4 => {
            let palette_path = Path::new(&args[2]);
            let output_path = Path::new(&args[3]);
            let palette = match Palette::load(palette_path) {
                Ok(value) => value,
                Err(error) => fail(error),
            };

            let mut frame = match Framebuffer::new(800, 480) {
                Ok(value) => value,
                Err(error) => fail(error),
            };

            frame.clear(&palette, Color::from_hex("#20242B").unwrap());
            frame.linear_gradient_rect(
                &palette, 40, 40, 720, 100,
                Color::from_hex("#13A4E8").unwrap(),
                Color::from_hex("#8A2BE2").unwrap(),
            );
            frame.fill_rect(&palette, 40, 180, 220, 90, Color::from_hex("#F0F2F5").unwrap());
            frame.stroke_rect(&palette, 40, 180, 220, 90, 3, Color::from_hex("#FFFFFF").unwrap());
            frame.fill_circle(&palette, 390, 225, 48, Color::from_hex("#31D17C").unwrap());
            frame.draw_line(&palette, 500, 180, 740, 270, Color::from_hex("#FFB020").unwrap());
            frame.save_png(output_path).unwrap_or_else(|e| fail(e));
            println!("Rendered demo to {}", output_path.display());
        }
        _ => usage(),
    }
}

fn fail(error: String) -> ! {
    eprintln!("error: {error}");
    std::process::exit(1);
}
