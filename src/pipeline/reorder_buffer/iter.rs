use super::ReorderBufferEntry;
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone)]
pub struct Iter<'a> {
    pub index_queue: &'a VecDeque<usize>,
    pub index_map: &'a HashMap<usize, usize>,
    pub buf: &'a Vec<(usize, ReorderBufferEntry)>,
    pub cur_head: usize,
    pub cur_tail: usize,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a (usize, ReorderBufferEntry);

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur_head == self.cur_tail {
            return None;
        }
        let idx = *self.index_queue.get(self.cur_head).unwrap();
        self.cur_head += 1;
        let raw_idx = *self.index_map.get(&idx).unwrap();
        self.buf.get(raw_idx)
    }
}

impl<'a> ExactSizeIterator for Iter<'a> {
    fn len(&self) -> usize {
        self.cur_tail - self.cur_head
    }
}

impl<'a> DoubleEndedIterator for Iter<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.cur_head == self.cur_tail {
            return None;
        }
        let idx = self.index_queue.get(self.cur_tail-1).unwrap();
        self.cur_tail -= 1;
        let raw_idx = *self.index_map.get(&idx).unwrap();
        self.buf.get(raw_idx)
    }
}