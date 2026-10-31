use std::collections::HashMap;

use image::{DynamicImage, ImageReader, RgbaImage};
use log::info;
use tex_packer_core::{InputImage, OfflineConfig, OfflinePacker, PageConfig};

use crate::assets::loader::AssetsLoader;

#[derive(Default)]
pub struct TextureAtlas {
    pub atlas: RgbaImage,
    pub textures: HashMap<String, [u32; 4]>,
}

impl TextureAtlas {
    pub fn build(textures: HashMap<String, RgbaImage>) -> Self {
        let page_config = PageConfig::builder()
            .allow_rotation(false)
            .texture_padding(0)
            .build()
            .unwrap();
        let config = OfflineConfig::builder()
            .page_config(page_config)
            .build()
            .unwrap();
        let packer = OfflinePacker::new(config);
        let images = textures
            .into_iter()
            .map(|(path, image)| InputImage {
                key: path,
                image: DynamicImage::ImageRgba8(image),
            })
            .collect();

        let output = packer.pack_images(images).unwrap();
        assert_eq!(output.pages().len(), 1); // TODO: for now we don't support multiple pages
        assert_eq!(output.atlas().pages().len(), 1); // TODO: for now we don't support multiple pages

        let mut textures = HashMap::new();
        let page = &output.atlas().pages()[0];
        for frame in page.frames() {
            let region = page.region(frame.region_id()).unwrap();
            let rect = region.allocation();
            textures.insert(frame.key().to_string(), [rect.x, rect.y, rect.w, rect.h]);
        }

        // for debugging
        for rendered in output.pages() {
            rendered
                .rgba()
                .save(format!("atlas-{}.png", rendered.page_id()))
                .unwrap();
        }

        let atlas = output.into_parts().1.remove(0);
        Self {
            atlas: atlas.into_rgba(),
            textures,
        }
    }
}

#[derive(Default)]
pub struct TextureRegistry {
    pub terrain_atlas: TextureAtlas,
    pub gui_atlas: TextureAtlas,
    pub font_atlas: TextureAtlas,
}

impl TextureRegistry {}

pub struct TextureLoader {
    gui_textures: HashMap<String, RgbaImage>,
    terrain_textures: HashMap<String, RgbaImage>,
    font_textures: HashMap<String, RgbaImage>,
    loader: AssetsLoader,
}

impl TextureLoader {
    pub fn new(loader: AssetsLoader) -> Self {
        Self {
            gui_textures: Default::default(),
            terrain_textures: Default::default(),
            font_textures: Default::default(),
            loader,
        }
    }

    pub fn finish(self) -> TextureRegistry {
        // TODO: split up textures:
        TextureRegistry {
            terrain_atlas: TextureAtlas::build(self.terrain_textures),
            gui_atlas: TextureAtlas::build(self.gui_textures),
            font_atlas: TextureAtlas::build(self.font_textures),
        }
    }

    pub fn load_terrain_texture(&mut self, key: String) {
        if self.terrain_textures.contains_key(&key) {
            return;
        }

        let path = format!("assets/minecraft/textures/{}.png", key);
        info!("loading terrain texture {} -> {}", key, path);

        let path = self.loader.path(&path).unwrap();
        let image_reader = ImageReader::open(path).unwrap();
        let image = image_reader.decode().unwrap().to_rgba8();

        self.terrain_textures.insert(key.to_string(), image);
    }

    pub fn load_gui_texture(&mut self, key: String) {
        if self.gui_textures.contains_key(&key) {
            return;
        }

        let path = format!("assets/minecraft/textures/{}.png", key);
        info!("loading gui texture {} -> {}", key, path);

        let path = self.loader.path(&path).unwrap();
        let image_reader = ImageReader::open(path).unwrap();
        let image = image_reader.decode().unwrap().to_rgba8();

        self.gui_textures.insert(key.to_string(), image);
    }

    pub fn load_font_texture(&mut self, key: String) {
        if self.font_textures.contains_key(&key) {
            return;
        }

        let path = format!("assets/minecraft/textures/{}.png", key);
        info!("loading gui texture {} -> {}", key, path);

        let path = self.loader.path(&path).unwrap();
        let image_reader = ImageReader::open(path).unwrap();
        let image = image_reader.decode().unwrap().to_rgba8();

        self.font_textures.insert(key.to_string(), image);
    }
}
