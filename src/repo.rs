use std::cell::RefCell;
use anni_repo::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use anni_repo::RepositoryManager;
use rand::Rng;
use rand::rngs::ThreadRng;

pub struct RepoManager {
    albums: HashMap<String, Album>,
}

impl RepoManager {
    pub fn new<P: AsRef<Path>>(root: P) -> Self {
        let manager = RepositoryManager::new(root).expect("Invalid Anni Metadata Repository");

        let mut albums = HashMap::new();
        for catalog in manager.catalogs().unwrap() {
            let album = manager.load_album(&catalog).unwrap();
            albums.insert(album.album_id().to_string(), album);
        }

        Self { albums }
    }

    pub fn load_album(&self, album_id: &str) -> Option<&Album> {
        self.albums.get(album_id)
    }

    pub fn filter_tracks<'repo, 'album>(&'repo self, albums: &'album HashSet<String>) -> TrackList<'repo, 'album> {
        let mut result = Vec::new();
        for album_id in albums.iter() {
            if let Some(album) = self.load_album(album_id) {
                for (disc_id, disc) in album.discs().iter().enumerate() {
                    let disc_id = disc_id + 1;
                    for (track_id, track) in disc.tracks().iter().enumerate() {
                        if let TrackType::Normal = track.track_type() {
                            result.push(TrackRef {
                                album_id,
                                disc_id,
                                track_id: track_id + 1,
                                album,
                                track,
                            });
                        }
                    }
                }
            }
        }

        TrackList::new(result)
    }
}

pub struct TrackRef<'repo, 'album> {
    pub album_id: &'album str,
    pub disc_id: usize,
    pub track_id: usize,
    pub album: &'repo Album,
    pub track: &'repo Track,
}

pub struct TrackList<'r, 'a> {
    rng: RefCell<ThreadRng>,
    inner: Vec<TrackRef<'r, 'a>>,
}

impl<'r, 'a> TrackList<'r, 'a> {
    fn new(tracks: Vec<TrackRef<'r, 'a>>) -> Self {
        Self {
            rng: RefCell::new(Default::default()),
            inner: tracks,
        }
    }

    pub fn random(&self) -> &TrackRef<'r, 'a> {
        let mut rng = self.rng.borrow_mut();
        let n = rng.gen_range(0..self.inner.len());
        self.inner.get(n).unwrap()
    }
}