use super::ReorderBufferEntry;

pub struct Iter<'a> {
    pub rob: &'a Vec<ReorderBufferEntry>,
    pub head: usize,
    pub tail: usize,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a ReorderBufferEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.head == self.tail {
            return None;
        }
        let old_head = self.head;
        self.head = (self.head + 1) % self.rob.len();
        self.rob.get(old_head)
    }
}

impl<'a> ExactSizeIterator for Iter<'a> {
    fn len(&self) -> usize {
        let tail = if self.tail < self.head {
            self.tail + self.rob.len()
        } else {
            self.tail
        };
        tail - self.head
    }
}

impl<'a> DoubleEndedIterator for Iter<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.head == self.tail {
            return None;
        }
        let old_tail = self.tail;
        self.tail = if self.tail == 0 {
            self.rob.len() - 1
        } else {
            self.tail - 1
        };
        self.rob.get(old_tail)
    }
}