use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zpm_utils::Path;

use crate::error::Error;
use crate::project::Project;

const SCHEMA_VERSION: u32 = 1;
const MANIFEST_CACHE_DIR: &str = "manifest";

#[derive(Debug, Clone)]
pub struct ManifestCacheEntry {
    pub body: Vec<u8>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Debug)]
pub struct ManifestCache {
    root: Path,
    enable_write: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheMeta {
    version: u32,
    etag: Option<String>,
    last_modified: Option<String>,
}

impl ManifestCache {
    pub fn new(project: &Project) -> Result<Self, Error> {
        let root = project.preferred_cache_path()
            .with_join_str(MANIFEST_CACHE_DIR);

        let enable_write = !project.config.settings.enable_immutable_cache.value;

        if enable_write {
            root.fs_create_dir_all()?;
        }

        Ok(Self { root, enable_write })
    }

    pub fn cache_key(registry: &str, path: &str) -> String {
        format!("{}{}", registry, path)
    }

    pub fn get(&self, key: &str) -> Result<Option<ManifestCacheEntry>, Error> {
        let (body_path, meta_path) = self.paths_for_key(key);

        if !body_path.fs_exists() || !meta_path.fs_exists() {
            return Ok(None);
        }

        let meta_text = match meta_path.fs_read_text() {
            Ok(text) => text,
            Err(_) => return Ok(None),
        };

        let meta: CacheMeta = match serde_json::from_str(&meta_text) {
            Ok(meta) => meta,
            Err(_) => return Ok(None),
        };

        if meta.version != SCHEMA_VERSION {
            return Ok(None);
        }

        let body = match body_path.fs_read() {
            Ok(body) => body,
            Err(_) => return Ok(None),
        };

        Ok(Some(ManifestCacheEntry {
            body,
            etag: meta.etag,
            last_modified: meta.last_modified,
        }))
    }

    pub fn put(&self, key: &str, entry: &ManifestCacheEntry) -> Result<(), Error> {
        if !self.enable_write {
            return Ok(());
        }

        let (body_path, meta_path) = self.paths_for_key(key);
        let hash = hash_key(key);

        let meta = CacheMeta {
            version: SCHEMA_VERSION,
            etag: entry.etag.clone(),
            last_modified: entry.last_modified.clone(),
        };

        let meta_text = serde_json::to_string(&meta)
            .map_err(|err| Error::SerializationError(err.to_string()))?;

        let tmp_body = self.root.with_join_str(format!(".{}.body.tmp-{}", hash, rand::random::<u64>()));
        let tmp_meta = self.root.with_join_str(format!(".{}.meta.tmp-{}", hash, rand::random::<u64>()));

        tmp_body.fs_write(&entry.body)?;
        tmp_body.fs_rename(&body_path)?;

        tmp_meta.fs_write_text(meta_text)?;
        tmp_meta.fs_rename(&meta_path)?;

        Ok(())
    }

    fn paths_for_key(&self, key: &str) -> (Path, Path) {
        let hash = hash_key(key);
        let body_path = self.root.with_join_str(format!("{}.json", hash));
        let meta_path = self.root.with_join_str(format!("{}.meta.json", hash));
        (body_path, meta_path)
    }
}

fn hash_key(key: &str) -> String {
    hex::encode(Sha256::digest(key.as_bytes()))
}
