use eframe::egui;
use rayon::slice::ParallelSliceMut;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct Image {
    pub size: [usize; 2],
    #[serde(skip)]
    pub texture_id: Option<egui::TextureHandle>,
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct ImageStack<P: AsRef<Path>> {
    pub homedir: Option<P>,
    pub pos: usize,
    #[serde(skip)]
    pub stacks: Option<Arc<[PathBuf]>>,
}

impl<P: AsRef<Path>> ImageStack<P> {
    pub fn set_homedir(&mut self, homedir: P) -> bool {
        self.homedir = Some(homedir);
        self.glob()
    }
    fn glob(&mut self) -> bool {
        self.homedir
            .as_ref()
            .and_then(|homedir| {
                let pattern = homedir.as_ref().join("*.jpg").display().to_string();

                glob::glob(&pattern)
                    .map(|paths| {
                        let mut paths: Vec<PathBuf> = paths.filter_map(|p| p.ok()).collect();
                        paths.par_sort_unstable();
                        self.stacks.replace(paths.into());
                    })
                    .ok()
            })
            .is_some()
    }
    pub fn len(&self) -> usize {
        self.stacks
            .as_ref()
            .map(|stacks| stacks.len())
            .unwrap_or(0)
    }

    pub fn max_slice(&self) -> usize {
        self.len().saturating_sub(1)
    }
    pub fn get_stacks(&self) -> Option<Arc<[PathBuf]>> {
        self.stacks.as_ref().map(Arc::clone)
    }

    pub fn get_current_images(&self, step: usize) -> (Option<&PathBuf>, Option<&PathBuf>) {
        self.stacks
            .as_ref()
            .map(|stacks| (stacks.get(self.pos - step), stacks.get(self.pos)))
            .unwrap_or_default()
    }
}
