use anni_repo::{Album, RepositoryManager};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use anni_repo::album::Track;
use rand::Rng;

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

    pub fn random_track<'repo, 'catalog>(&'repo self, albums: &'catalog HashSet<String>) -> TrackRef<'repo, 'catalog> {
        loop {
            let mut rng = rand::thread_rng();
            let pos = rng.gen_range(0..albums.len());
            if let Some(catalog) = albums.iter().nth(pos) {
                if let Some(album) = self.load_album(catalog) {
                    let tracks = album.discs()[0].tracks();
                    let track_id = rng.gen_range(0..tracks.len());
                    let ref track = tracks[track_id];
                    let track_id = track_id + 1;
                    use anni_repo::album::TrackType;
                    match track.track_type() {
                        TrackType::Normal => {
                            return TrackRef {
                                catalog,
                                track_id,
                                album,
                                track,
                            };
                        }
                        _ => continue,
                    }
                }
            }
        }
    }
}

pub struct TrackRef<'repo, 'catalog> {
    pub catalog: &'catalog str,
    pub track_id: usize,
    pub album: &'repo Album,
    pub track: &'repo Track,
}