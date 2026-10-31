use crate::primitives::BlockData;

pub enum ItemData {
    StoneBlock,
    GrassBlock,
    DirtBlock,
    AcaciaStairsBlock,
    AcaciaLogBlock,
    AcaciaLeavesBlock,
    AcaciaPlanksBlock,
    None,
}

impl ItemData {
    // TODO: also needs orientation
    pub fn get_block_data(&self) -> Option<BlockData> {
        match self {
            ItemData::StoneBlock => Some(BlockData::Stone),
            ItemData::GrassBlock => Some(BlockData::Grass),
            ItemData::DirtBlock => Some(BlockData::Dirt),
            ItemData::AcaciaStairsBlock => Some(BlockData::AcaciaStairs),
            ItemData::AcaciaLogBlock => Some(BlockData::AcaciaLog),
            ItemData::AcaciaPlanksBlock => Some(BlockData::AcaciaPlanks),
            ItemData::AcaciaLeavesBlock => Some(BlockData::AcaciaLeaves),
            _ => None,
        }
    }
}
