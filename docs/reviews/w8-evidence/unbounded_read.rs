// Isolated reproduction of gpui-pre 0.3.9 ImageAssetLoader's Resource::Path arm.
fn main() {
    let path = std::env::args().nth(1).expect("path");
    match std::fs::read(path) {
        Ok(bytes) => println!("read {} bytes", bytes.len()),
        Err(error) => eprintln!("read error: {error}"),
    }
}
