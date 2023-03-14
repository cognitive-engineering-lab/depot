use std::collections::VecDeque;

pub struct LogBuffer {
  deque: VecDeque<u8>,
  max_capacity: usize,
}

const DEFAULT_MAX_CAPACITY: usize = 2048;

impl LogBuffer {
  pub fn new() -> Self {
    LogBuffer {
      deque: VecDeque::new(),
      max_capacity: DEFAULT_MAX_CAPACITY,
    }
  }

  #[cfg(test)]
  pub fn with_max_capacity(max_capacity: usize) -> Self {
    LogBuffer {
      deque: VecDeque::new(),
      max_capacity,
    }
  }

  pub fn push(&mut self, mut bytes: &[u8]) {
    if bytes.len() > self.max_capacity {
      bytes = &bytes[bytes.len() - self.max_capacity..];
    }
    if self.deque.len() + bytes.len() > self.max_capacity {
      let remaining_capacity = self.max_capacity - self.deque.len();
      let to_remove = bytes.len() - remaining_capacity;
      self.deque.rotate_left(to_remove);
      self.deque.truncate(self.deque.len() - to_remove);
    }

    self.deque.extend(bytes);
  }

  pub fn contents(&self) -> (&[u8], &[u8]) {
    self.deque.as_slices()
  }
}

#[test]
fn test_log_buffer() {
  let mut buffer = LogBuffer::with_max_capacity(4);

  macro_rules! contents {
    () => {{
      let (l, r) = buffer.contents();
      (l.iter().chain(r.iter())).copied().collect::<Vec<_>>()
    }};
  }

  buffer.push(&[0, 1]);
  assert_eq!(contents!(), vec![0, 1]);

  buffer.push(&[2]);
  assert_eq!(contents!(), vec![0, 1, 2]);

  buffer.push(&[3, 4]);
  assert_eq!(contents!(), vec![1, 2, 3, 4]);

  buffer.push(&[5, 6]);
  assert_eq!(contents!(), vec![3, 4, 5, 6]);

  buffer.push(&[7, 8, 9, 10, 11]);
  assert_eq!(contents!(), vec![8, 9, 10, 11])
}
