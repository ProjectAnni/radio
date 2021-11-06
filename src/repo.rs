use anni_repo::{Album, RepositoryManager};
use std::collections::HashMap;
use std::path::Path;

pub struct RepoManager {
    albums: HashMap<String, Album>,
    discs: HashMap<String, Album>,
    /// one album catalog -> multi disc catalog map
    multi_map: HashMap<String, Vec<String>>,
}

impl RepoManager {
    pub fn new<P: AsRef<Path>>(root: P) -> Self {
        let manager = RepositoryManager::new(root).expect("Invalid Anni Metadata Repository");

        let mut albums = HashMap::new();
        let mut discs = HashMap::new();
        let mut multi_map = HashMap::new();
        for catalog in manager.catalogs().unwrap() {
            let album = manager.load_album(&catalog).unwrap();
            if album.discs().len() == 1 {
                albums.insert(album.catalog().to_string(), album);
            } else {
                let album_catalog = album.catalog().to_string();
                let release_date = album.release_date().clone();
                let mut disc_catalogs = Vec::new();
                for (i, disc) in album.into_discs().into_iter().enumerate() {
                    let title = disc.title().to_string();
                    disc_catalogs.push(disc.catalog().to_string());
                    discs.insert(
                        disc.catalog().to_string(),
                        disc.into_album(
                            format!("{} [Disc {}]", title, i + 1),
                            release_date.clone(),
                        ),
                    );
                }
                multi_map.insert(album_catalog, disc_catalogs);
            }
        }

        Self { albums, discs, multi_map }
    }

    pub fn load_album(&self, catalog: &str) -> Option<&Album> {
        self.discs.get(catalog).map(|a| Some(a)).unwrap_or(self.albums.get(catalog))
    }

    pub fn load_albums(&self, catalog: &str) -> Vec<&Album> {
        if self.multi_map.contains_key(catalog) {
            self.multi_map[catalog].iter().filter_map(|c| self.load_album(c)).collect()
        } else {
            self.load_album(catalog).map(|a| vec![a]).unwrap_or_default()
        }
    }
}