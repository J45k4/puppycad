mod app;
mod renderer;
mod view3d;

use app::App;
use std::path::Path;
use winit::event_loop::EventLoop;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let argument = std::env::args().nth(1);
    if argument.as_deref() == Some("--help") {
        println!(
            "Usage: puppycad [PROJECT.pcad]\n       puppycad --roundtrip IN.pcad OUT.pcad\n       puppycad --export IN.pcad OUT.stl|OUT.3mf\n       puppycad --print-shortcuts"
        );
        return Ok(());
    }
    if argument.as_deref() == Some("--print-shortcuts") {
        print!("{}", app::shortcuts_markdown());
        return Ok(());
    }
    if argument.as_deref() == Some("--roundtrip") {
        let args: Vec<String> = std::env::args().skip(2).collect();
        let [input, output] = args.as_slice() else {
            eprintln!("Usage: puppycad --roundtrip IN.pcad OUT.pcad");
            std::process::exit(2);
        };
        if let Err(error) = puppycad::roundtrip(Path::new(input), Path::new(output)) {
            eprintln!("roundtrip failed: {error}");
            std::process::exit(1);
        }
        return Ok(());
    }
    if argument.as_deref() == Some("--export") {
        let args: Vec<String> = std::env::args().skip(2).collect();
        let [input, output] = args.as_slice() else {
            eprintln!("Usage: puppycad --export IN.pcad OUT.stl|OUT.3mf");
            std::process::exit(2);
        };
        match puppycad::export(Path::new(input), Path::new(output)) {
            Ok(paths) => {
                for path in paths {
                    println!("{}", path.display());
                }
            }
            Err(error) => {
                eprintln!("export failed: {error}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    let mut app = match argument {
        Some(path) => App::with_project(Path::new(&path))?,
        None => App::default(),
    };
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;
    Ok(())
}
