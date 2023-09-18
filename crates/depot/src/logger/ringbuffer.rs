use std::collections::{vec_deque, VecDeque};

pub struct RingBuffer<T> {
  data: VecDeque<T>,
  max_capacity: usize,
}

const DEFAULT_MAX_CAPACITY: usize = 1024;

#[allow(unused)]
impl<T> RingBuffer<T> {
  pub fn new() -> Self {
    RingBuffer {
      data: VecDeque::new(),
      max_capacity: DEFAULT_MAX_CAPACITY,
    }
  }

  pub fn with_max_capacity(max_capacity: usize) -> Self {
    RingBuffer {
      data: VecDeque::new(),
      max_capacity,
    }
  }

  pub fn push(&mut self, log: T) {
    if self.data.len() == self.max_capacity {
      self.data.pop_front();
    }
    self.data.push_back(log);
  }

  pub fn iter(&self) -> vec_deque::Iter<'_, T> {
    self.data.iter()
  }

  pub fn clear(&mut self) {
    self.data.clear();
  }

  pub fn len(&self) -> usize {
    self.data.len()
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
      buffer.iter().copied().collect::<Vec<_>>()
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
