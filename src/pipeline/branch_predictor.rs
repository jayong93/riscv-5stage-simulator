use std::collections::HashMap;

#[derive(Default, Debug, Clone)]
pub struct BranchPredictor {
    branch_map: HashMap<u32, (bool, bool)>,
}

impl BranchPredictor {
    pub fn predict(&mut self, pc: u32) -> bool {
        self.branch_map.entry(pc).or_insert((false, false)).0
    }

    pub fn update(&mut self, pc: u32, is_taken: u32) {
        let val = self.branch_map.get_mut(&pc).unwrap();
        match (*val, is_taken) {
            ((_, true), 0) => {
                *val = (val.0, false);
            }
            ((_, false), 0) => {
                *val = (false, false);
            }
            ((_, true), 1) => {
                *val = (true, true);
            }
            ((_, false), 1) => {
                *val = (val.0, true);
            }
            _ => unreachable!(),
        }
    }
}
