mod app;
mod renderer;
mod view3d;

use app::App;
use std::path::Path;
use winit::event_loop::EventLoop;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let argument = std::env::args().nth(1);
    if argument.as_deref() == Some("--help") {
        println!("Usage: puppycad [PROJECT.pcad]\n       puppycad --roundtrip IN.pcad OUT.pcad");
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
    let mut app = match argument {
        Some(path) => App::with_project(Path::new(&path))?,
        None => App::default(),
    };
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;
    Ok(())
}
