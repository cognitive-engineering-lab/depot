#[tokio::main]
async fn main() {
  env_logger::init();
  if let Err(e) = depot_js::run().await {
    eprintln!("Depot failed with the error:\n");
    if cfg!(debug_assertions) {
      eprintln!("{e:?}");
    } else {
      eprintln!("{e}");
    }
    std::process::exit(1);
  }
}
