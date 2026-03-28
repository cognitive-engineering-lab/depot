#[tokio::main]
async fn main() {
  env_logger::init();
  if let Err(e) = depot_js::run().await {
    eprintln!("Depot failed with the error: {e:?}");
    std::process::exit(1);
  }
}
