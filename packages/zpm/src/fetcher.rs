use std::{collections::HashSet, fmt::{self, Display, Formatter}, io::{Cursor, Read}, sync::Arc};

use arca::Path;
use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::{config::registry_url_for, error::Error, hash::Sha256, http::http_client, install::InstallContext, manifest::Manifest, primitives::{Ident, Locator, Reference}, resolver::Resolution, semver, zip::first_entry_from_zip};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum PackageData {
    Local {
        path: Path,
        discard_from_lookup: bool,
    },

    Zip {
        path: Path,
        data: Vec<u8>,
        checksum: Sha256,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum PackageLinking {
    Hard,
    Soft,
}

impl Display for PackageLinking {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            PackageLinking::Hard => write!(f, "hard"),
            PackageLinking::Soft => write!(f, "soft"),
        }
    }
}

impl PackageData {
    pub fn path(&self) -> &Path {
        match self {
            PackageData::Local {path, ..} => path,
            PackageData::Zip {path, ..} => path,
        }
    }

    pub fn source_dir(&self, locator: &Locator) -> Path {
        match self {
            PackageData::Local {..} => Path::from("."),
            PackageData::Zip {..} => Path::from(format!("node_modules/{}", locator.ident.as_str())),
        }
    }

    pub fn checksum(&self) -> Option<Sha256> {
        match self {
            PackageData::Local {..} => None,
            PackageData::Zip {checksum, ..} => Some(checksum.clone()),
        }
    }

    pub fn link_type(&self) -> PackageLinking {
        match self {
            PackageData::Local {..} => PackageLinking::Soft,
            PackageData::Zip {..} => PackageLinking::Hard,
        }
    }

    pub fn read_text(&self, p: &Path) -> Result<String, Error> {
        match self {
            PackageData::Local {path, ..} => {
                let path = path
                    .with_join(p);

                std::fs::read_to_string(path.to_path_buf())
                    .map_err(Arc::new)
                    .map_err(Error::IoError)
            },

            PackageData::Zip {data, ..} => {
                let reader = Cursor::new(data);
                let mut zip = zip::read::ZipArchive::new(reader)
                    .unwrap();

                let mut file_entry = zip.by_name(&p.to_string())
                    .expect("Failed to find the requested file");

                let mut text = String::new();
                file_entry.read_to_string(&mut text).unwrap();

                Ok(text)
            },
        }
    }
}

fn convert_tar_gz_to_zip(ident: &Ident, tar_gz_data: Bytes) -> Result<Vec<u8>, Error> {
    let mut decompressed = vec![];

    flate2::read::GzDecoder::new(Cursor::new(tar_gz_data))
        .read_to_end(&mut decompressed)
        .map_err(Arc::new)
        .map_err(Error::IoError)?;

    let entries = crate::zip::entries_from_tar(&decompressed)?;
    let entries = crate::zip::strip_first_segment(entries);

    let manifest_entry = entries.iter().find(|entry| entry.name == "package.json")
        .ok_or(Error::MissingPackageManifest)?;

    let manifest: Manifest = serde_json::from_slice(&manifest_entry.data)
        .map_err(Arc::new)
        .map_err(Error::InvalidJsonData)?;

    let entries = crate::zip::normalize_entries(entries);
    let entries = crate::zip::prefix_entries(entries, format!("node_modules/{}", ident.as_str()));

    Ok(crate::zip::craft_zip(&entries))
}

fn convert_folder_to_zip(ident: &Ident, folder_path: &Path) -> Result<Vec<u8>, Error> {
    let entries = crate::zip::entries_from_folder(folder_path.to_path_buf())?;

    let manifest_entry = entries.iter().find(|entry| entry.name == "package.json")
        .ok_or(Error::MissingPackageManifest)?;

    let manifest: Manifest = serde_json::from_slice(&manifest_entry.data)
        .map_err(Arc::new)
        .map_err(Error::InvalidJsonData)?;

    let entries = crate::zip::normalize_entries(entries);
    let entries = crate::zip::prefix_entries(entries, format!("node_modules/{}", ident.as_str()));

    Ok(crate::zip::craft_zip(&entries))
}

pub async fn fetch<'a>(context: InstallContext<'a>, locator: &Locator, parent_data: Option<PackageData>) -> Result<PackageData, Error> {
    match &locator.reference {
        Reference::Link(path)
            => fetch_link(path, &locator.parent, parent_data),

        Reference::Portal(path)
            => fetch_portal(path, &locator.parent, parent_data),

        Reference::Tarball(path)
            => Ok(fetch_tarball_with_manifest(context, &locator, path, &locator.parent, parent_data).await?.1),

        Reference::Folder(path)
            => Ok(fetch_folder_with_manifest(context, &locator, path, &locator.parent, parent_data).await?.1),

        Reference::Semver(version)
            => fetch_semver(context, &locator, &locator.ident, &version).await,

        Reference::SemverAlias(ident, version)
            => fetch_semver(context, &locator, &ident, &version).await,

        Reference::Workspace(ident)
            => fetch_workspace(context, &ident),

        _ => Err(Error::Unsupported),
    }
}

pub fn fetch_link(path: &str, parent: &Option<Arc<Locator>>, parent_data: Option<PackageData>) -> Result<PackageData, Error> {
    let parent = parent.as_ref()
        .expect("The parent locator is required for resolving a linked package");
    let parent_data = parent_data
        .expect("The parent data is required for retrieving the path of a linked package");

    let link_path = parent_data.path()
        .with_join(&parent_data.source_dir(parent))
        .with_join_str(&path);

    Ok(PackageData::Local {
        path: link_path,
        discard_from_lookup: true,
    })
}

pub fn fetch_portal(path: &str, parent: &Option<Arc<Locator>>, parent_data: Option<PackageData>) -> Result<PackageData, Error> {
    let parent = parent.as_ref()
        .expect("The parent locator is required for resolving a portal package");
    let parent_data = parent_data
        .expect("The parent data is required for retrieving the path of a portal package");

    let portal_path = parent_data.path()
        .with_join(&parent_data.source_dir(parent))
        .with_join_str(&path);

    Ok(PackageData::Local {
        path: portal_path,
        discard_from_lookup: false,
    })
}

pub async fn fetch_tarball_with_manifest<'a>(context: InstallContext<'a>, locator: &Locator, path: &str, parent: &Option<Arc<Locator>>, parent_data: Option<PackageData>) -> Result<(Resolution, PackageData), Error> {
    let parent = parent.as_ref()
        .expect("The parent locator is required for resolving a tarball package");
    let parent_data = parent_data
        .expect("The parent data is required for retrieving the path of a tarball package");

    let tarball_path = parent_data.path()
        .with_join(&parent_data.source_dir(parent))
        .with_join_str(&path);

    let (path, data, checksum) = context.package_cache.unwrap().upsert_blob(locator.clone(), &".zip", || async {
        let archive = std::fs::read(tarball_path.to_path_buf())
            .map_err(Arc::new)?;

        convert_tar_gz_to_zip(&locator.ident, Bytes::from(archive))
    }).await?;

    let first_entry = first_entry_from_zip(&data);
    let manifest = first_entry
        .and_then(|entry|
            serde_json::from_slice::<Manifest>(&entry.data)
                .map_err(Arc::new)
                .map_err(Error::InvalidJsonData)
        )?;

    let resolution = Resolution {
        version: manifest.version,
        locator: locator.clone(),
        dependencies: manifest.dependencies.unwrap_or_default(),
        peer_dependencies: manifest.peer_dependencies.unwrap_or_default(),
        optional_dependencies: HashSet::new(),
    };

    Ok((resolution, PackageData::Zip {
        path,
        data,
        checksum,
    }))
}

pub async fn fetch_folder_with_manifest<'a>(context: InstallContext<'a>, locator: &Locator, path: &str, parent: &Option<Arc<Locator>>, parent_data: Option<PackageData>) -> Result<(Resolution, PackageData), Error> {
    let parent = parent.as_ref()
        .expect("The parent locator is required for resolving a tarball package");
    let parent_data = parent_data
        .expect("The parent data is required for retrieving the path of a tarball package");

    let folder_path = parent_data.path()
        .with_join(&parent_data.source_dir(parent))
        .with_join_str(&path);

    let (path, data, checksum) = context.package_cache.unwrap().upsert_blob(locator.clone(), &".zip", || async {
        convert_folder_to_zip(&locator.ident, &folder_path)
    }).await?;

    let first_entry = first_entry_from_zip(&data);
    let manifest = first_entry
        .and_then(|entry|
            serde_json::from_slice::<Manifest>(&entry.data)
                .map_err(Arc::new)
                .map_err(Error::InvalidJsonData)
        )?;

    let resolution = Resolution {
        version: manifest.version,
        locator: locator.clone(),
        dependencies: manifest.dependencies.unwrap_or_default(),
        peer_dependencies: manifest.peer_dependencies.unwrap_or_default(),
        optional_dependencies: HashSet::new(),
    };

    Ok((resolution, PackageData::Zip {
        path,
        data,
        checksum,
    }))
}

pub async fn fetch_semver<'a>(context: InstallContext<'a>, locator: &Locator, ident: &Ident, version: &semver::Version) -> Result<PackageData, Error> {
    let (path, data, checksum) = context.package_cache.unwrap().upsert_blob(locator.clone(), &".zip", || async {
        let client = http_client()?;
        let url = format!("{}/{}/-/{}-{}.tgz", registry_url_for(ident), ident, ident.name(), version.to_string());

        let response = client.get(url.clone()).send().await
            .map_err(|err| Error::RemoteRegistryError(Arc::new(err)))?;

        let archive = response.bytes().await
            .map_err(|err| Error::RemoteRegistryError(Arc::new(err)))?;

        convert_tar_gz_to_zip(ident, archive)
    }).await?;

    Ok(PackageData::Zip {
        path,
        data,
        checksum,
    })
}

pub fn fetch_workspace(context: InstallContext, ident: &Ident) -> Result<PackageData, Error> {
    let project = context.project
        .expect("The project is required for fetching a workspace package");

    let workspace = project.workspaces
        .get(ident)
        .ok_or(Error::WorkspaceNotFound(ident.clone()))?;

    Ok(PackageData::Local {
        path: workspace.path.clone(),
        discard_from_lookup: false,
    })
}
