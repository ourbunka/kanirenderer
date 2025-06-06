use std::env;
use kanirenderer_viewer::run;

fn main() {
    env::set_var("RUST_BACKTRACE", "1");
    println!("Hello!");
    println!("");
    println!("Loading...");
    println!("");
    println!("");
    println!("press \"WASD\" to move, hold right click to rotate camera,");
    println!("\"space\" to travel up vertically,");
    println!("\"left shift\" to travel down vertically, scroll wheel to zoom  ");
    println!("IJKL to move light, U and O to move light up and down vertically");
    println!("");
    println!("");
    println!("⚠️⚠️⚠️esc to quit kanirenderer.⚠️⚠️⚠️");
    println!("");
    println!("");
    // let current_dir =  env::current_dir();
    // match current_dir {
    //     Ok(path) => println!("current working directory : {:?}", path),
    //     Err(_) => println!("failed to get current working directory")
    // }
    let file_path = std::env::args().nth(1).expect("no file name/file path given");
    let file_type = std::env::args().nth(2).expect("no file type given, use \"default\" for meshes with directX coordinates, or \"opengl\" for meshes with opengl coordinates (for example, meshes authored & export from blender)");
    let mut fullscreen_mode = std::env::args().nth(3).unwrap_or("windowed".to_string());
    match fullscreen_mode.clone().as_str() {
        "windowed" => println!("windowed mode"),
        "fullscreen" => println!("fullscreen mode"),
        _ => fullscreen_mode = "windowed".to_string(),
    }
    let use_hdr_string = std::env::args().nth(4).unwrap_or("false".to_string());
    let mut use_hdr = false;
    match use_hdr_string.as_str(){
        "true" => {use_hdr = true}
        "false" => {use_hdr = false}
        _ => {}
    }
    println!("{:?}, {:?}, {:?}", file_path, file_type, use_hdr);
    
    pollster::block_on(run(file_path, file_type, fullscreen_mode, use_hdr));
}