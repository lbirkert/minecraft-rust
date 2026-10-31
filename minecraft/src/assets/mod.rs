pub mod loader;
pub mod model;
pub mod paths;
pub mod texture;

pub use loader::AssetsLoader;
pub use model::ModelLoader;
pub use model::ModelRegistry;
pub use texture::TextureLoader;
pub use texture::TextureRegistry;

pub struct Assets {
    pub texture_registry: TextureRegistry,
    pub model_registry: ModelRegistry,
}

impl Assets {
    pub fn load(loader: AssetsLoader) -> Self {
        let mut model_loader = ModelLoader::new(loader.clone());
        model_loader.load("block/grass_normal".to_string());
        model_loader.load("block/stone".to_string());
        model_loader.load("block/dirt".to_string());
        model_loader.load("block/bedrock".to_string());
        model_loader.load("block/acacia_stairs".to_string());
        model_loader.load("block/acacia_leaves".to_string());
        model_loader.load("block/acacia_log".to_string());
        model_loader.load("block/acacia_planks".to_string());

        // load terrain textures
        let mut texture_loader = TextureLoader::new(loader);
        for key in model_loader.get_textures() {
            texture_loader.load_terrain_texture(key);
        }

        // load gui textures
        texture_loader.load_gui_texture("gui/container/inventory".to_string());
        texture_loader
            .load_gui_texture("gui/container/creative_inventory/tab_inventory".to_string());
        texture_loader.load_gui_texture("gui/widgets".to_string());

        // load font textures
        texture_loader.load_font_texture("font/ascii".to_string());

        let texture_registry = texture_loader.finish();
        let model_registry = model_loader.finish(&texture_registry);
        Assets {
            texture_registry,
            model_registry,
        }
    }
}
