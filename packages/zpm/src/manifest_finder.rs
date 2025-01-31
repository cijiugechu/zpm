use std::{collections::{BTreeMap, BTreeSet}, io, time::UNIX_EPOCH};

use arca::Path;
use bincode::{Decode, Encode};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use crate::error::Error;

#[derive(Default, Debug, Encode, Decode)]
pub struct SaveState {
    cache: BTreeMap<Path, u128>,
    manifests: BTreeSet<Path>,
    roots: Vec<Path>,
}

impl SaveState {
    pub fn new(roots: Vec<Path>) -> Self {
        Self {
            cache: BTreeMap::from_iter(roots.iter().map(|root| (root.clone(), 0))),
            manifests: BTreeSet::new(),
            roots,
        }
    }
}

#[derive(Debug)]
pub enum PollResult {
    Changed,
    NotFound,
}

pub trait ManifestFinder {
    fn rsync(&mut self) -> Result<Vec<&Path>, Error>;
}

/**
 * The CachedManifestFinder struct is meant to very quickly locate all the
 * manifests in a given directory, no matter how deep the directory structure
 * is, by caching the mtime of each directory it checks.
 * 
 * This strategy is similar to how `git status` works; subsequent invocations
 * only need to compare the cached mtime for each directory with the current
 * mtime to figure out whether they need perform the costly readdir syscall.
 */
#[derive(Default, Debug)]
pub struct CachedManifestFinder {
    root_path: Path,
    save_state_path: Path,
    save_state: SaveState,
}

impl CachedManifestFinder {
    pub fn new(root_path: Path) -> Result<Self, Error> {
        let save_state_path = root_path
            .with_join_str(".yarn/ignore/manifests");

        let roots = vec![
            Path::new(),
        ];

        // We tolerate any errors; worst case, we'll just re-scan the entire
        // directory to rebuild the cache.
        let mut save_state = save_state_path
            .fs_read()
            .ok()
            .and_then(|save_data| bincode::decode_from_slice::<SaveState, _>(save_data.as_slice(), bincode::config::standard()).ok())
            .map(|(save_state, _)| save_state)
            .unwrap_or_default();

        if save_state.roots != roots {
            save_state = SaveState::new(roots);
        }

        Ok(Self {
            root_path,
            save_state_path,
            save_state,
        })
    }

    fn save(&self) -> Result<(), Error> {
        let data = bincode::encode_to_vec(
            &self.save_state,
            bincode::config::standard(),
        )?;

        self.save_state_path
            .fs_create_parent()?
            .fs_write(&data)?;

        Ok(())
    }


    fn refresh_directory(&mut self, rel_path: &Path, current_time: u128) -> Result<(), Error> {
        self.save_state.cache.insert(rel_path.clone(), current_time);

        let abs_path = self.root_path
            .with_join(rel_path);

        let directory_entries = abs_path.fs_read_dir()?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;

        let has_package_json = directory_entries
            .iter()
            .any(|entry| entry.file_name() == "package.json");

        if has_package_json {
            self.save_state.manifests.insert(rel_path.clone());
        } else {
            self.save_state.manifests.remove(&rel_path);
        }

        let directory_entries_and_types = directory_entries
            .into_iter()
            .map(|entry| entry.file_type().map(|file_type| (entry, file_type.is_dir())))
            .collect::<Result<Vec<_>, _>>()?;

        let new_directories = directory_entries_and_types
            .into_iter()
            .filter_map(|(entry, is_dir)| is_dir.then_some(entry))
            .collect::<Vec<_>>();

        for directory in new_directories {
            let entry_rel_path = rel_path
                .with_join_str(directory.file_name().to_str().unwrap());

            self.refresh_directory(&entry_rel_path, current_time)?;
        }

        Ok(())
    }
}

impl ManifestFinder for CachedManifestFinder {
    fn rsync(&mut self) -> Result<Vec<&Path>, Error> {
        let current_time = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_nanos() as u128;

        let cache_check = self.save_state.cache.par_iter().map(|(rel_path, stored_mtime)| -> Result<Option<(Path, PollResult, Option<u128>)>, Error> {
            let abs_path = self.root_path
                .with_join(&rel_path);

            let metadata = match abs_path.fs_metadata() {
                Ok(metadata) => metadata,
                Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Some((rel_path.clone(), PollResult::NotFound, None))),
                Err(e) => return Err(e.into()),
            };

            let mtime = metadata.modified()?
                .duration_since(UNIX_EPOCH)?
                .as_nanos() as u128;

            let status = match mtime <= *stored_mtime {
                true => return Ok(None),
                false => PollResult::Changed,
            };

            Ok(Some((rel_path.clone(), status, Some(mtime))))
        }).collect::<Result<Vec<_>, Error>>()?;

        let mut has_changed = false;

        for check_entry in cache_check {
            if let Some((rel_path, poll_result, _)) = check_entry {
                has_changed = true;

                match poll_result {
                    PollResult::Changed => {
                        self.refresh_directory(&rel_path, current_time)?;
                    },

                    PollResult::NotFound => {
                        self.save_state.cache.remove(&rel_path);
                        self.save_state.manifests.remove(&rel_path);
                    },
                }
            }
        }

        if has_changed {
            self.save()?;
        }

        let manifests = self.save_state.manifests.iter()
            .collect::<Vec<_>>();

        Ok(manifests)
    }
}
