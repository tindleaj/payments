fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::env::set_var("RUST_BACKTRACE", "1");
    let path = std::env::args().nth(1).expect("Invalid argument passed");
    let mut verbose = false;

    if let Some(arg) = std::env::args().nth(2) {
        verbose = arg == "verbose".to_string();
    };

    Ok(payments::run(&path, verbose)?)
}
