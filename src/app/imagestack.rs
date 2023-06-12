use std::path::{Path, PathBuf};

use eframe::egui;
use rayon::slice::ParallelSliceMut;

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct Image {
    pub size: [usize; 2],
    #[serde(skip)]
    pub texture_id: Option<egui::TextureHandle>,
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct ImageStack<P: AsRef<Path>> {
    pub homedir: Option<P>,
    #[serde(skip)]
    pub stacks: Vec<PathBuf>,
    pub pos: usize,
}

impl<P: AsRef<Path>> ImageStack<P> {
    pub fn set_homedir(&mut self, homedir: P) -> bool {
        self.homedir = Some(homedir);
        self.stacks.clear();
        self.glob()
    }
    fn glob(&mut self) -> bool {
        match &self.homedir {
            None => false,
            Some(homedir) => {
                let pattern = homedir.as_ref().join("*.jpg").display().to_string();
                match glob::glob(&pattern).ok() {
                    None => false,
                    Some(paths) => {
                        self.stacks.extend(paths.filter_map(|p| p.ok()));
                        self.stacks.par_sort_unstable();
                        true
                    }
                }
            }
        }
    }
    pub fn len(&self) -> usize {
        self.stacks.len()
    }

    pub fn max_slice(&self) -> usize {
        self.stacks.len().saturating_sub(1)
    }

    pub fn get_current_images(&self) -> (Option<&PathBuf>, Option<&PathBuf>) {
        (self.stacks.get(self.pos - 1), self.stacks.get(self.pos))
    }
}
