use std::borrow::Cow;
use std::cell::RefCell;
use anni_repo::prelude::*;
use std::collections::HashSet;
use std::path::Path;
use anni_repo::{OwnedRepositoryManager, RepositoryManager};
use rand::Rng;
use rand::rngs::ThreadRng;

pub struct RepoManager {
    manager: OwnedRepositoryManager,
}

impl RepoManager {
    pub fn new<P: AsRef<Path>>(root: P) -> Self {
        let manager = RepositoryManager::new(root).expect("Invalid Anni Metadata Repository");
        let manager = manager.into_owned_manager().unwrap();

        Self { manager }
    }

    pub fn load_album(&self, album_id: &str) -> Option<&Album> {
        self.manager.albums().get(album_id)
    }

    pub fn filter_tracks<'repo, 'album>(&'repo self, albums: &'album HashSet<Cow<str>>) -> TrackList<'repo, 'album> {
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