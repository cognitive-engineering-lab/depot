use std::collections::VecDeque;

pub struct RingBuffer<T> {
  lines: VecDeque<T>,
  max_capacity: usize,
}

const DEFAULT_MAX_CAPACITY: usize = 1024;

impl<T> RingBuffer<T> {
  pub fn new() -> Self {
    RingBuffer {
      lines: VecDeque::new(),
      max_capacity: DEFAULT_MAX_CAPACITY,
    }
  }

  #[cfg(test)]
  pub fn with_max_capacity(max_capacity: usize) -> Self {
    RingBuffer {
      lines: VecDeque::new(),
      max_capacity,
    }
  }

  pub fn push(&mut self, log: T) {
    if self.lines.len() == self.max_capacity {
      self.lines.pop_front();
    }
    self.lines.push_back(log);
  }

  pub fn lines(&self) -> impl Iterator<Item = &T> + '_ {
    let (first, second) = self.lines.as_slices();
    first.iter().chain(second.iter())
  }

  pub fn clear(&mut self) {
    self.lines.clear();
  }
}

#[test]
fn test_log_buffer() {
  let mut buffer = RingBuffer::with_max_capacity(4);

  macro_rules! extend {
    ($in:expr) => {
      for x in $in {
        buffer.push(x);
      }
    };
  }

  macro_rules! contents {
    () => {
      buffer.lines().copied().collect::<Vec<_>>()
    };
  }

  extend!([0, 1]);
  assert_eq!(contents!(), vec![0, 1]);

  extend!([2]);
  assert_eq!(contents!(), vec![0, 1, 2]);

  extend!([3, 4]);
  assert_eq!(contents!(), vec![1, 2, 3, 4]);

  extend!([5, 6]);
  assert_eq!(contents!(), vec![3, 4, 5, 6]);

  extend!([7, 8, 9, 10, 11]);
  assert_eq!(contents!(), vec![8, 9, 10, 11])
}
