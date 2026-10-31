use std::collections::{HashMap, HashSet};

use log::{info, warn};
use nalgebra::Vector3;
use serde::Deserialize;

use crate::{
    assets::{TextureRegistry, loader::AssetsLoader},
    renderer::terrain,
};

pub struct ModelRegistry {
    pub models: HashMap<String, Model>,
    pub quads: Vec<terrain::Quad>,
}

pub struct Model {
    pub offset: u32,
    pub length: u32,
    pub quads: Vec<ModelQuad>,
}

pub struct ModelQuad {
    pub tintindex: Option<u32>,
    pub cullface: Cullface,
    pub verticies: [Vector3<f32>; 4],
}

impl ModelQuad {
    pub fn occludes(a: &ModelQuad, b: &ModelQuad) -> bool {
        let bounds = |quad: &ModelQuad| {
            let mut min = [f32::INFINITY; 3];
            let mut max = [f32::NEG_INFINITY; 3];

            for v in &quad.verticies {
                let p = [v.x, v.y, v.z];

                for i in 0..3 {
                    min[i] = min[i].min(p[i]);
                    max[i] = max[i].max(p[i]);
                }
            }

            (min, max)
        };

        let (amin, amax) = bounds(a);
        let (bmin, bmax) = bounds(b);

        let eps = 1e-6;

        // Same plane: at least one coordinate must be constant
        // and equal for both quads.
        let same_plane = (0..3).any(|axis| {
            (amax[axis] - amin[axis]).abs() < eps
                && (bmax[axis] - bmin[axis]).abs() < eps
                && (amin[axis] - bmin[axis]).abs() < eps
        });

        if !same_plane {
            return false;
        }

        // A's bounding rectangle fully contains B's.
        (0..3).all(|axis| amin[axis] <= bmin[axis] + eps && amax[axis] >= bmax[axis] - eps)
    }
}

pub struct ModelLoader {
    models: HashMap<String, ModelJson>,
    loader: AssetsLoader,
}

impl ModelLoader {
    pub fn new(loader: AssetsLoader) -> ModelLoader {
        ModelLoader {
            models: Default::default(),
            loader,
        }
    }

    // loads a model.json
    pub fn load(&mut self, key: String) {
        if self.models.contains_key(&key) {
            return;
        }

        let path = format!("assets/minecraft/models/{}.json", &key);
        info!("loading model {} -> {}...", key, path);

        let contents = self.loader.load(&path).unwrap();
        let model: ModelJson = serde_json::from_slice(&contents).unwrap();

        if let Some(ref key) = model.parent {
            self.load(key.clone())
        }

        self.models.insert(key, model);
    }

    // get textures required by loaded models
    pub fn get_textures(&self) -> HashSet<String> {
        let mut textures = HashSet::new();
        for model in self.models.values() {
            let m_textures = model
                .textures
                .iter()
                .filter(|(_, key)| !key.starts_with("#"))
                .map(|(_, key)| key.clone());
            textures.extend(m_textures);
        }
        textures
    }

    pub fn finish(self, texture_registry: &TextureRegistry) -> ModelRegistry {
        let mut models = HashMap::new();
        let mut quads = Vec::new();
        for key in self.models.keys() {
            if let Some(model) =
                self.build_model(key, &mut HashMap::new(), texture_registry, &mut quads)
            {
                models.insert(key.clone(), model);
            };
        }

        ModelRegistry { models, quads }
    }

    fn build_model(
        &self,
        key: &str,
        texture_defs: &mut HashMap<String, String>,
        texture_registry: &TextureRegistry,
        quads: &mut Vec<terrain::Quad>,
    ) -> Option<Model> {
        let is_parent = !texture_defs.is_empty();

        let model = self.models.get(key).unwrap();

        // collect texture definitions
        for (texture_key, texture) in model.textures.iter() {
            if texture.starts_with("#") {
                let Some(texture) = texture_defs.get(texture) else {
                    warn!(
                        "skip building model {} due to missing texture definition {}",
                        key, texture
                    );
                    return None;
                };
                texture_defs.insert("#".to_string() + texture_key, texture.clone());
                continue;
            }

            if texture_defs.contains_key(texture_key) {
                continue;
            }

            texture_defs.insert("#".to_string() + texture_key, texture.clone());
        }

        // check welldefined
        for element in model.elements.iter() {
            for face in element.faces.values() {
                if !texture_defs.contains_key(&face.texture) {
                    warn!(
                        "skip building model {} due to missing texture {}",
                        key, face.texture
                    );
                    return None;
                }
            }
        }

        if !is_parent {
            info!("building model {}", key);
        }

        // build parent model
        match model.parent {
            Some(ref parent) => self.build_model(parent, texture_defs, texture_registry, quads),
            None => {
                let offset = quads.len() as u32;

                let mut mquads = Vec::new();

                for element in model.elements.iter() {
                    Self::try_push_quad(
                        quads,
                        &mut mquads,
                        element,
                        texture_defs,
                        texture_registry,
                        FaceIdJson::East,
                    );
                    Self::try_push_quad(
                        quads,
                        &mut mquads,
                        element,
                        texture_defs,
                        texture_registry,
                        FaceIdJson::West,
                    );
                    Self::try_push_quad(
                        quads,
                        &mut mquads,
                        element,
                        texture_defs,
                        texture_registry,
                        FaceIdJson::Up,
                    );
                    Self::try_push_quad(
                        quads,
                        &mut mquads,
                        element,
                        texture_defs,
                        texture_registry,
                        FaceIdJson::Down,
                    );
                    Self::try_push_quad(
                        quads,
                        &mut mquads,
                        element,
                        texture_defs,
                        texture_registry,
                        FaceIdJson::South,
                    );
                    Self::try_push_quad(
                        quads,
                        &mut mquads,
                        element,
                        texture_defs,
                        texture_registry,
                        FaceIdJson::North,
                    );
                }

                Some(Model {
                    offset,
                    length: quads.len() as u32 - offset,
                    quads: mquads,
                })
            }
        }
    }

    fn try_push_quad(
        quads: &mut Vec<terrain::Quad>,
        model_quads: &mut Vec<ModelQuad>,
        element: &ElementJson,
        textures_defs: &HashMap<String, String>,
        texture_registry: &TextureRegistry,
        face_id: FaceIdJson,
    ) {
        let Some(face) = element.faces.get(&face_id) else {
            return;
        };

        let [fx, fy, fz] = element.from;
        let [tx, ty, tz] = element.to;

        let v_dne = [tx, fy, fz];
        let v_dnw = [fx, fy, fz];
        let v_dse = [tx, fy, tz];
        let v_dsw = [fx, fy, tz];

        let v_une = [tx, ty, fz];
        let v_unw = [fx, ty, fz];
        let v_use = [tx, ty, tz];
        let v_usw = [fx, ty, tz];

        let verticies: [[f32; 3]; 4] = match face_id {
            FaceIdJson::Up => [v_usw, v_use, v_une, v_unw],
            FaceIdJson::Down => [v_dsw, v_dnw, v_dne, v_dse],
            FaceIdJson::North => [v_unw, v_une, v_dne, v_dnw],
            FaceIdJson::South => [v_use, v_usw, v_dsw, v_dse],
            FaceIdJson::East => [v_une, v_use, v_dse, v_dne],
            FaceIdJson::West => [v_usw, v_unw, v_dnw, v_dsw],
        };

        let uv = face.uv.unwrap_or(match face_id {
            FaceIdJson::Up | FaceIdJson::Down => [0.0, 0.0, tx - fx, tz - fz],
            FaceIdJson::North | FaceIdJson::South => [0.0, 0.0, tx - fx, ty - fy],
            FaceIdJson::East | FaceIdJson::West => [0.0, 0.0, tz - fz, ty - fy],
        });

        let texture = textures_defs.get(&face.texture).unwrap();
        let texture = texture_registry
            .terrain_atlas
            .textures
            .get(texture)
            .unwrap();

        let tint = if face.tintindex.is_some() {
            [0.3, 0.7, 0.0, 1.0]
        } else {
            [1.0, 1.0, 1.0, 1.0]
        };

        let quad = terrain::Quad {
            v1: terrain::QuadVertex::new(
                verticies[0][0] / 16.0,
                verticies[0][1] / 16.0,
                verticies[0][2] / 16.0,
                uv[0] + texture[0] as f32,
                uv[1] + texture[1] as f32,
            ),
            v2: terrain::QuadVertex::new(
                verticies[1][0] / 16.0,
                verticies[1][1] / 16.0,
                verticies[1][2] / 16.0,
                uv[2] + texture[0] as f32,
                uv[1] + texture[1] as f32,
            ),
            v3: terrain::QuadVertex::new(
                verticies[2][0] / 16.0,
                verticies[2][1] / 16.0,
                verticies[2][2] / 16.0,
                uv[2] + texture[0] as f32,
                uv[3] + texture[1] as f32,
            ),
            v4: terrain::QuadVertex::new(
                verticies[3][0] / 16.0,
                verticies[3][1] / 16.0,
                verticies[3][2] / 16.0,
                uv[0] + texture[0] as f32,
                uv[3] + texture[1] as f32,
            ),
            tint,
        };

        quads.push(quad);
        model_quads.push(ModelQuad {
            tintindex: face.tintindex,
            cullface: face.cullface.clone(),
            verticies: [
                Vector3::from_row_slice(&verticies[0]),
                Vector3::from_row_slice(&verticies[1]),
                Vector3::from_row_slice(&verticies[2]),
                Vector3::from_row_slice(&verticies[3]),
            ],
        });
    }
}

// JSON deserialization structures

#[derive(Debug, Deserialize)]
pub struct ModelJson {
    pub parent: Option<String>,
    #[serde(default)]
    pub elements: Vec<ElementJson>,
    #[serde(default)]
    pub textures: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct ElementJson {
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub faces: HashMap<FaceIdJson, FaceJson>,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum FaceIdJson {
    North,
    South,
    Down,
    Up,
    East,
    West,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum Cullface {
    Down,
    Up,
    North,
    South,
    East,
    West,
    #[default]
    None,
}

impl Cullface {
    pub fn rev(&self) -> Cullface {
        match self {
            Cullface::East => Cullface::West,
            Cullface::West => Cullface::East,
            Cullface::Up => Cullface::Down,
            Cullface::Down => Cullface::Up,
            Cullface::South => Cullface::North,
            Cullface::North => Cullface::South,
            _ => Cullface::None,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct FaceJson {
    pub texture: String,
    #[serde(default)]
    pub cullface: Cullface,
    pub uv: Option<[f32; 4]>,
    pub tintindex: Option<u32>,
}
