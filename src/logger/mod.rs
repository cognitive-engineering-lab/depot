use anyhow::Result;

use std::collections::HashMap;

use crate::workspace::package::PackageIndex;

use self::ringbuffer::RingBuffer;

mod ringbuffer;
pub mod ui;

pub struct Logger {
  logs: HashMap<PackageIndex, HashMap<String, RingBuffer<String>>>,
}

impl Logger {
  pub fn new() -> Result<Self> {
    Ok(Logger {
      logs: HashMap::default(),
    })
  }

  pub fn register_log(&mut self, index: PackageIndex, process: &str) {
    self
      .logs
      .entry(index)
      .or_default()
      .insert(process.to_string(), RingBuffer::new());
  }

  pub fn logger(&mut self, index: PackageIndex, process: &str) -> &mut RingBuffer<String> {
    self.logs.get_mut(&index).unwrap().get_mut(process).unwrap()
  }
}
