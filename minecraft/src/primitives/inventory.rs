use crate::primitives::ItemData;

pub struct Inventory {
    // TODO: model slots
    pub slots: [ItemData; 9],
    pub selected_slot: usize,
}

impl Inventory {
    pub fn selected(&self) -> &ItemData {
        &self.slots[self.selected_slot]
    }
}

impl Default for Inventory {
    fn default() -> Self {
        Self {
            selected_slot: 0,
            slots: [
                ItemData::AcaciaLogBlock,
                ItemData::AcaciaLeavesBlock,
                ItemData::AcaciaPlanksBlock,
                ItemData::None,
                ItemData::None,
                ItemData::None,
                ItemData::None,
                ItemData::None,
                ItemData::None,
            ],
        }
    }
}
