use std::cell::RefCell;
use anni_repo::{Album, RepositoryManager};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use anni_repo::album::{Track, TrackType};
use rand::Rng;
use rand::rngs::ThreadRng;

pub struct RepoManager {
    albums: HashMap<String, Album>,
    discs: HashMap<String, Album>,
}

impl RepoManager {
    pub fn new<P: AsRef<Path>>(root: P) -> Self {
        let manager = RepositoryManager::new(root).expect("Invalid Anni Metadata Repository");

        let mut albums = HashMap::new();
        let mut discs = HashMap::new();
        for catalog in manager.catalogs().unwrap() {
            let album = manager.load_album(&catalog).unwrap();
            if album.discs().len() == 1 {
                albums.insert(album.catalog().to_string(), album);
            } else {
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
            }
        }

        Self { albums, discs }
    }

    pub fn load_album(&self, catalog: &str) -> Option<&Album> {
        self.discs.get(catalog).map(|a| Some(a)).unwrap_or(self.albums.get(catalog))
    }

    pub fn filter_tracks<'repo, 'catalog>(&'repo self, albums: &'catalog HashSet<String>) -> TrackList<'repo, 'catalog> {
        let mut result = Vec::new();
        for catalog in albums.iter() {
            if let Some(album) = self.load_album(catalog) {
                for (id, track) in album.discs()[0].tracks().iter().enumerate() {
                    if let TrackType::Normal = track.track_type() {
                        result.push(TrackRef {
                            catalog,
                            track_id: id + 1,
                            album,
                            track,
                        });
                    }
                }
            }
        }

        TrackList::new(result)
    }
}

pub struct TrackRef<'repo, 'catalog> {
    pub catalog: &'catalog str,
    pub track_id: usize,
    pub album: &'repo Album,
    pub track: &'repo Track,
}

pub struct TrackList<'r, 'c> {
    rng: RefCell<ThreadRng>,
    inner: Vec<TrackRef<'r, 'c>>,
}

impl<'r, 'c> TrackList<'r, 'c> {
    fn new(tracks: Vec<TrackRef<'r, 'c>>) -> Self {
        Self {
            rng: RefCell::new(Default::default()),
            inner: tracks,
        }
    }

    pub fn random(&self) -> &TrackRef<'r, 'c> {
        let mut rng = self.rng.borrow_mut();
        let n = rng.gen_range(0..self.inner.len());
        self.inner.get(n).unwrap()
    }
}