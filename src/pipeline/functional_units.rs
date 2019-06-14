use super::reservation_staion::RSEntry;
use super::load_buffer::LoadBufferEntry;
use super::reorder_buffer::ReorderBufferEntry;

#[derive(Debug, Default)]
pub struct FunctionalUnits {

}

impl FunctionalUnits {
    pub fn execute_general(&mut self, entry: &RSEntry) -> Option<u32> {
        unimplemented!()
    }

    pub fn execute_load(&mut self, entry: &LoadBufferEntry) -> Option<u32> {
        unimplemented!()
    }

    pub fn execute_store(&mut self, entry: &ReorderBufferEntry) -> Option<()> {
        unimplemented!()
    }
}