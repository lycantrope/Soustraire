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
    #[serde(skip)]
    pub stacks: Arc<Vec<PathBuf>>,
    pub pos: usize,
}

impl<P: AsRef<Path>> ImageStack<P> {
    pub fn set_homedir(&mut self, homedir: P) -> bool {
        self.homedir = Some(homedir);
        self.stacks = Arc::new(Vec::new());
        self.glob()
    }
    fn glob(&mut self) -> bool {
        self.homedir
            .as_ref()
            .and_then(|homedir| {
                let pattern = homedir.as_ref().join("*.jpg").display().to_string();
                glob::glob(&pattern)
                    .map(|paths| {
                        let stacks =
                            Arc::get_mut(&mut self.stacks).expect("fail to retrieve mutable stack");
                        stacks.extend(paths.filter_map(|p| p.ok()));
                        stacks.par_sort_unstable();
                    })
                    .ok()
            })
            .is_some()
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
