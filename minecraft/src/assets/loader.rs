use std::path::PathBuf;

use log::info;

#[derive(Debug, Clone)]
pub struct AssetsLoader {
    paths: Vec<PathBuf>,
}

impl AssetsLoader {
    pub fn new(mut paths: Vec<PathBuf>) -> Self {
        paths.reverse();
        Self { paths }
    }

    pub fn load(&self, asset: &str) -> Option<Vec<u8>> {
        for path in self.paths.iter() {
            let rel_path = path.join(asset);

            if std::fs::exists(&rel_path).ok().is_none_or(|v| !v) {
                continue;
            }

            return std::fs::read(rel_path).ok();
        }

        None
    }

    pub fn path(&self, asset: &str) -> Option<PathBuf> {
        for path in self.paths.iter() {
            let rel_path = path.join(asset);

            if std::fs::exists(&rel_path).ok().is_none_or(|v| !v) {
                continue;
            }

            return Some(rel_path);
        }

        None
    }
}
