install:
  cargo install --path crates/depot --locked

watch:
  cargo watch -x 'install --path crates/depot --debug --locked --offline'