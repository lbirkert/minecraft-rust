use crate::assets::block::BlockModel;

#[derive(Debug, Default)]
pub struct Loader {
    block_models: Vec<BlockModel>,
}

impl Loader {
    pub fn load_default() {
        let loader = Self::default();
        loader.add_block(BlockModel::from(include_str!(
            "../../../assets/minecraft/models/block/cube.json"
        )));

        todo!()
    }

    pub fn add_block(&mut self, block: BlockModel) {
        self.block_models.push(block);
    }
}
