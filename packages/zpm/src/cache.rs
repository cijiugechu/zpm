use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path as StdPath, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(target_os = "linux")]
use std::os::fd::{AsFd, AsRawFd};
use itertools::Itertools;
use zpm_formats::{iter_ext::IterExt, zip::ToZip, Entry};
use zpm_macro_enum::zpm_enum;
use zpm_primitives::Locator;
use zpm_utils::{Hash64, Path};
use futures::Future;

#[cfg(target_os = "linux")]
use rustix::{fs::{AtFlags, Mode, OFlags}, io::Errno};

use crate::report::current_report;
use crate::{
    error::Error,
};

pub const CACHE_VERSION: usize = 1;
static CACHE_TMP_SEQ: AtomicU64 = AtomicU64::new(0);

#[zpm_enum]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[derive_variants(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CacheEntry {
    #[no_pattern]
    Info {
        path: Path,
        checksum: Option<Hash64>,
    },

    #[no_pattern]
    Data {
        info: InfoCacheEntry,
        data: Vec<u8>,
    },
}

impl CacheEntry {
    pub fn into_info(self) -> InfoCacheEntry {
        match self {
            CacheEntry::Info(params) => params,
            CacheEntry::Data(params) => params.info,
        }
    }
}

pub struct CachePacker {
    compression_algorithm: Option<zpm_formats::CompressionAlgorithm>,
}

impl CachePacker {
    pub fn pack<'a>(&self, entries: Vec<Entry<'a>>) -> Result<Vec<u8>, Error> {
        let archive = entries
            .into_iter()
            .update_crc32()
            .compress(self.compression_algorithm)
            .collect::<Vec<_>>()
            .to_zip();

        Ok(archive)
    }
}

pub struct CompositeCache {
    pub compression_algorithm: Option<zpm_formats::CompressionAlgorithm>,

    pub global_cache: Option<DiskCache>,
    pub local_cache: Option<DiskCache>,
}

impl CompositeCache {
    pub fn new(compression_algorithm: Option<zpm_formats::CompressionAlgorithm>, global_cache: Option<DiskCache>, local_cache: Option<DiskCache>) -> Self {
        CompositeCache {
            compression_algorithm,
            global_cache,
            local_cache,
        }
    }

    pub fn packer(&self) -> CachePacker {
        CachePacker {
            compression_algorithm: self.compression_algorithm,
        }
    }

    pub fn key_path(&self, key: &Locator, ext: &str) -> Path {
        if let Some(ref cache) = self.local_cache {
            return cache.key_path(key, ext);
        }

        if let Some(ref cache) = self.global_cache {
            return cache.key_path(key, ext);
        }

        panic!("Expected at least one cache to be set");
    }

    pub fn cache_entry(&self, key: Locator, ext: &str) -> Result<InfoCacheEntry, Error> {
        if let Some(ref cache) = self.local_cache {
            return cache.cache_entry(key, ext);
        }

        if let Some(ref cache) = self.global_cache {
            return cache.cache_entry(key, ext);
        }

        panic!("Expected at least one cache to be set");
    }

    pub fn check_cache_entry(&self, key: Locator, ext: &str) -> Result<Option<InfoCacheEntry>, Error> {
        if let Some(ref cache) = self.local_cache {
            return cache.check_cache_entry(key, ext);
        }

        if let Some(ref cache) = self.global_cache {
            return cache.check_cache_entry(key, ext);
        }

        panic!("Expected at least one cache to be set");
    }

    async fn load<R, F>(func: F) -> Result<Vec<u8>, Error>
    where
        R: Future<Output = Result<Vec<u8>, Error>>,
        F: FnOnce() -> R,
    {
        current_report().await.as_ref().map(|report| {
            report.counters.fetch_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        });

        let res
            = func().await;

        if let Ok(data) = res.as_ref() {
            current_report().await.as_ref().map(|report| {
                report.counters.fetch_size.fetch_add(data.len() as u32, std::sync::atomic::Ordering::Relaxed);
            });
        }

        res
    }

    pub async fn ensure_blob<R, F>(&self, key: Locator, ext: &str, func: F) -> Result<CacheEntry, Error>
    where
        R: Future<Output = Result<Vec<u8>, Error>>,
        F: FnOnce() -> R,
    {
        if let Some(ref cache) = self.local_cache {
            return cache.ensure_blob(key.clone(), ext, || async {
                if let Some(ref cache) = self.global_cache {
                    Ok(cache.upsert_blob(key, ext, || Self::load(func)).await?.data)
                } else {
                    Self::load(func).await
                }
            }).await;
        }

        if let Some(ref cache) = self.global_cache {
            return cache.ensure_blob(key, ext, || Self::load(func)).await;
        }

        panic!("Expected at least one cache to be set");
    }

    pub async fn upsert_blob<R, F>(&self, key: Locator, ext: &str, func: F) -> Result<DataCacheEntry, Error>
    where
        R: Future<Output = Result<Vec<u8>, Error>>,
        F: FnOnce() -> R,
    {
        if let Some(ref cache) = self.local_cache {
            return cache.upsert_blob(key.clone(), ext, || async {
                if let Some(ref cache) = self.global_cache {
                    Ok(cache.upsert_blob(key, ext, || Self::load(func)).await?.data)
                } else {
                    Self::load(func).await
                }
            }).await;
        }

        if let Some(ref cache) = self.global_cache {
            return cache.upsert_blob(key, ext, || Self::load(func)).await;
        }

        panic!("Expected at least one cache to be set");
    }

    pub async fn clean(&self) -> Result<usize, Error> {
        if let Some(ref cache) = self.local_cache {
            return cache.clean().await;
        }

        Ok(0)
    }
}

pub struct DiskCache {
    cache_path: Path,
    name_suffix: String,
    immutable: bool,
    accessed_files: Arc<Mutex<HashSet<String>>>,
}

impl DiskCache {
    pub fn new(cache_path: Path, name_suffix: String, immutable: bool) -> Self {
        DiskCache {
            cache_path,
            name_suffix,
            immutable,
            accessed_files: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn key_path(&self, locator: &Locator, ext: &str) -> Path {
        let key_name
            = format!("{}-{}{}{}", locator.slug(), CACHE_VERSION, self.name_suffix, ext);

        let key_path = self.cache_path
            .with_join_str(&key_name);

        if let Ok(mut accessed) = self.accessed_files.lock() {
            accessed.insert(key_name);
        }

        key_path
    }

    pub fn cache_entry(&self, key: Locator, ext: &str) -> Result<InfoCacheEntry, Error> {
        let key_path
            = self.key_path(&key, ext);

        Ok(InfoCacheEntry {
            path: key_path,
            checksum: None,
        })
    }

    pub fn check_cache_entry(&self, key: Locator, ext: &str) -> Result<Option<InfoCacheEntry>, Error> {
        let key_path
            = self.key_path(&key, ext);

        Ok(key_path.if_exists().map(|path| {
            InfoCacheEntry {
                path,
                checksum: None,
            }
        }))
    }

    pub async fn ensure_blob<R, F>(&self, key: Locator, ext: &str, func: F) -> Result<CacheEntry, Error>
    where
        R: Future<Output = Result<Vec<u8>, Error>>,
        F: FnOnce() -> R,
    {
        let key_path
            = self.key_path(&key, ext);
        let key_path_buf
            = key_path.to_path_buf();

        let exists
            = tokio::fs::try_exists(key_path_buf.clone()).await?;

        Ok(match exists {
            true => {
                InfoCacheEntry {
                    path: key_path,
                    checksum: None,
                }.into()
            },

            false => {
                if self.immutable {
                    return Err(Error::ImmutableCache(key));
                }

                let data
                    = self.fetch_and_store_blob::<R, F>(key_path_buf, func).await?;

                tokio::task::spawn_blocking(move || {
                    let checksum
                        = Hash64::from_data(&data);

                    InfoCacheEntry {
                        path: key_path,
                        checksum: Some(checksum),
                    }.into()
                }).await.unwrap()
            },
        })
    }

    pub async fn upsert_blob<R, F>(&self, key: Locator, ext: &str, func: F) -> Result<DataCacheEntry, Error>
    where
        R: Future<Output = Result<Vec<u8>, Error>>,
        F: FnOnce() -> R,
    {
        let key_path
            = self.key_path(&key, ext);
        let key_path_buf
            = key_path.to_path_buf();

        let read
            = tokio::fs::read(key_path_buf.clone()).await;

        Ok(match read {
            Ok(data) => {
                DataCacheEntry {
                    info: InfoCacheEntry {
                        path: key_path,
                        checksum: None,
                    },
                    data,
                }
            },

            Err(err) => {
                if err.kind() != std::io::ErrorKind::NotFound {
                    return Err(err)?;
                }

                if self.immutable {
                    return Err(Error::ImmutableCache(key));
                }

                let data
                    = self.fetch_and_store_blob::<R, F>(key_path_buf, func).await?;

                tokio::task::spawn(async move {
                    let checksum
                        = Hash64::from_data(&data);

                    DataCacheEntry {
                        info: InfoCacheEntry {
                            path: key_path,
                            checksum: Some(checksum),
                        },
                        data,
                    }
                }).await.unwrap()
            },
        })
    }

    async fn fetch_and_store_blob<R, F>(&self, key_path: PathBuf, func: F) -> Result<Vec<u8>, Error>
    where
        R: Future<Output = Result<Vec<u8>, Error>>,
        F: FnOnce() -> R,
    {
        let data
            = func().await?;

        self.write_blob_atomically(&key_path, &data)?;

        Ok(data)
    }

    fn write_blob_atomically(&self, key_path: &StdPath, data: &[u8]) -> Result<(), Error> {
        #[cfg(target_os = "linux")]
        {
            if self.try_write_blob_with_otmpfile_linux(key_path, data)? {
                return Ok(());
            }
        }

        self.write_blob_with_named_temp(key_path, data)
    }

    fn write_blob_with_named_temp(&self, key_path: &StdPath, data: &[u8]) -> Result<(), Error> {
        key_path.parent()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "cache path has no parent"))?;

        let mut tmp_path = None;
        for _ in 0..32 {
            let candidate
                = Self::build_temp_path(key_path)?;

            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&candidate)
            {
                Ok(mut file) => {
                    file.write_all(data)?;
                    tmp_path = Some(candidate);
                    break;
                }

                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                }

                Err(err) => {
                    return Err(err.into());
                },
            }
        }

        let tmp_path
            = tmp_path.ok_or_else(|| std::io::Error::other("failed to allocate temporary cache file path"))?;

        let mut renamed = false;
        let rename_result = std::fs::rename(&tmp_path, key_path);
        match rename_result {
            Ok(_) => {
                renamed = true;
            }

            Err(err) if Self::is_rename_conflict(&err) => {
                let _ = std::fs::remove_file(key_path);
                match std::fs::rename(&tmp_path, key_path) {
                    Ok(_) => {
                        renamed = true;
                    }

                    Err(second_err) => {
                        if !(Self::is_rename_conflict(&second_err) && key_path.exists()) {
                            let _ = std::fs::remove_file(&tmp_path);
                            return Err(second_err.into());
                        }
                    },
                }
            }

            Err(err) => {
                let _ = std::fs::remove_file(&tmp_path);
                return Err(err.into());
            },
        }

        if !renamed {
            let _ = std::fs::remove_file(&tmp_path);
        }

        // We may race with concurrent writers that already produced a valid cache file.
        if !key_path.exists() {
            return Err(std::io::Error::new(std::io::ErrorKind::NotFound, format!(
                "cache write completed without creating destination: {}",
                key_path.display()
            )).into());
        }

        Ok(())
    }

    fn build_temp_path(key_path: &StdPath) -> Result<PathBuf, Error> {
        let parent
            = key_path.parent()
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "cache path has no parent"))?;

        let file_name
            = key_path.file_name()
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "cache path has no file name"))?
                .to_string_lossy();

        let seq
            = CACHE_TMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let nanos
            = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
        let pid
            = std::process::id();

        Ok(parent.join(format!(".{}.{}.{}.{}.tmp", file_name, pid, nanos, seq)))
    }

    fn is_rename_conflict(error: &std::io::Error) -> bool {
        matches!(
            error.kind(),
            std::io::ErrorKind::AlreadyExists | std::io::ErrorKind::DirectoryNotEmpty
        )
    }

    #[cfg(target_os = "linux")]
    fn try_write_blob_with_otmpfile_linux(&self, key_path: &StdPath, data: &[u8]) -> Result<bool, Error> {
        let parent
            = key_path.parent()
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "cache path has no parent"))?;
        let file_name
            = key_path.file_name()
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "cache path has no file name"))?
                .to_str()
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "cache file name is not valid UTF-8"))?;

        let dir
            = std::fs::File::open(parent)?;

        let tmp_fd = match rustix::fs::openat(
            dir.as_fd(),
            ".",
            OFlags::WRONLY | OFlags::TMPFILE,
            Mode::from_raw_mode(0o664),
        ) {
            Ok(fd) => fd,

            Err(errno) if Self::is_tmpfile_unsupported_errno(errno) => {
                return Ok(false);
            }

            Err(errno) => {
                return Err(Self::errno_to_io(errno).into());
            },
        };

        let mut tmp_file
            = std::fs::File::from(tmp_fd);
        tmp_file.write_all(data)?;

        if let Err(_first_err) = Self::link_tmpfile_into_path(&tmp_file, &dir, file_name) {
            let _ = rustix::fs::unlinkat(dir.as_fd(), file_name, AtFlags::empty());
            if let Err(second_err) = Self::link_tmpfile_into_path(&tmp_file, &dir, file_name) {
                return Err(Self::errno_to_io(second_err).into());
            }
        }

        Ok(true)
    }

    #[cfg(target_os = "linux")]
    fn link_tmpfile_into_path(
        tmp_file: &std::fs::File,
        dir: &std::fs::File,
        file_name: &str,
    ) -> Result<(), Errno> {
        match rustix::fs::linkat(
            tmp_file.as_fd(),
            "",
            dir.as_fd(),
            file_name,
            AtFlags::EMPTY_PATH,
        ) {
            Ok(_) => Ok(()),

            Err(errno) if Self::should_try_procfs_link(errno) => {
                let proc_path
                    = format!("/proc/self/fd/{}", tmp_file.as_raw_fd());

                rustix::fs::linkat(
                    rustix::fs::CWD,
                    proc_path.as_str(),
                    dir.as_fd(),
                    file_name,
                    AtFlags::SYMLINK_FOLLOW,
                )
            }

            Err(errno) => Err(errno),
        }
    }

    #[cfg(target_os = "linux")]
    fn should_try_procfs_link(errno: Errno) -> bool {
        matches!(
            errno,
            Errno::PERM | Errno::INVAL | Errno::OPNOTSUPP | Errno::NOENT | Errno::ISDIR
        )
    }

    #[cfg(target_os = "linux")]
    fn is_tmpfile_unsupported_errno(errno: Errno) -> bool {
        matches!(
            errno,
            Errno::INVAL | Errno::OPNOTSUPP | Errno::NOSYS | Errno::PERM
        )
    }

    #[cfg(target_os = "linux")]
    fn errno_to_io(errno: Errno) -> std::io::Error {
        std::io::Error::from_raw_os_error(errno.raw_os_error())
    }

    pub async fn clean(&self) -> Result<usize, Error> {
        let accessed_files
            = self.accessed_files.lock()
                .map_err(|_| Error::Unsupported)?;

        let cache_entries = self.cache_path
            .fs_read_dir()?
            .collect::<Result<Vec<_>, _>>()?;

        let extraneous_cache_files = cache_entries
            .iter()
            .filter(|entry| entry.file_type().unwrap().is_file())
            .map(|entry| entry.file_name().to_os_string().into_string().unwrap())
            .filter(|file| !accessed_files.contains(file))
            .collect_vec();

        let extraneous_count
            = extraneous_cache_files.len();

        if extraneous_count > 0 && self.immutable {
            return Err(Error::ImmutableCacheCleanup(Path::try_from(extraneous_cache_files[0].clone()).unwrap()));
        }

        for file in extraneous_cache_files {
            self.cache_path
                .with_join_str(&file)
                .fs_rm_file()?;
        }

        Ok(extraneous_count)
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, sync::{Arc, atomic::Ordering}, time::{SystemTime, UNIX_EPOCH}};

    use super::DiskCache;
    use zpm_utils::Path;

    fn temp_cache_dir(label: &str) -> std::path::PathBuf {
        let seq
            = super::CACHE_TMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let nanos
            = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();

        let path = std::env::temp_dir()
            .join(format!("zpm-cache-test-{}-{}-{}-{}", label, std::process::id(), nanos, seq));

        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn test_cache(path: &std::path::Path) -> DiskCache {
        let cache_path
            = Path::try_from(path.to_path_buf()).unwrap();

        DiskCache::new(cache_path, "".to_string(), false)
    }

    #[test]
    fn writes_new_file_atomically() {
        let dir
            = temp_cache_dir("new");
        let key_path
            = dir.join("entry.zip");
        let cache
            = test_cache(&dir);

        cache.write_blob_atomically(&key_path, b"hello").unwrap();

        let content
            = std::fs::read(&key_path).unwrap();
        assert_eq!(content, b"hello");

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn replaces_existing_file() {
        let dir
            = temp_cache_dir("replace");
        let key_path
            = dir.join("entry.zip");
        let cache
            = test_cache(&dir);

        std::fs::write(&key_path, b"before").unwrap();
        cache.write_blob_atomically(&key_path, b"after").unwrap();

        let content
            = std::fs::read(&key_path).unwrap();
        assert_eq!(content, b"after");

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn concurrent_writers_produce_valid_file() {
        let dir
            = temp_cache_dir("concurrency");
        let key_path
            = dir.join("entry.zip");

        let cache
            = Arc::new(test_cache(&dir));

        let payloads = (0..12)
            .map(|idx| vec![idx as u8; 512 + idx as usize * 13])
            .collect::<Vec<_>>();

        let mut handles
            = vec![];

        for payload in payloads.clone() {
            let cache
                = Arc::clone(&cache);
            let key_path
                = key_path.clone();

            handles.push(std::thread::spawn(move || {
                cache.write_blob_atomically(&key_path, &payload).unwrap();
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let expected
            = payloads.into_iter()
                .collect::<HashSet<_>>();
        let content
            = std::fs::read(&key_path).unwrap();

        assert!(expected.contains(&content));
        assert!(content.len() >= 512);

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_tmpfile_path_smoke() {
        let dir
            = temp_cache_dir("linux");
        let key_path
            = dir.join("entry.zip");
        let cache
            = test_cache(&dir);

        cache.write_blob_atomically(&key_path, b"linux").unwrap();

        let content
            = std::fs::read(&key_path).unwrap();
        assert_eq!(content, b"linux");

        std::fs::remove_dir_all(dir).unwrap();
    }
}
