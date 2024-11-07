use std::collections::{HashMap, HashSet};

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::{error::Error, install::{normalize_resolutions, InstallContext, InstallOpResult, IntoResolutionResult, ResolutionResult}, manifest::RemoteManifest, primitives::{descriptor::{descriptor_map_deserializer, descriptor_map_serializer}, Descriptor, Ident, Locator, PeerRange, Range}, system};

mod folder;
mod git;
mod link;
mod patch;
mod portal;
mod npm;
mod semver;
mod tarball;
mod url;
mod workspace;

/**
 * Contains the information we keep in the lockfile for a given package.
 */
#[derive(Clone, Debug, Deserialize, Decode, Encode, Serialize, PartialEq, Eq)]
pub struct Resolution {
    #[serde(rename = "resolution")]
    pub locator: Locator,
    pub version: crate::semver::Version,

    #[serde(flatten)]
    pub requirements: system::Requirements,

    #[serde(default)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    #[serde(serialize_with = "descriptor_map_serializer")]
    #[serde(deserialize_with = "descriptor_map_deserializer")]
    pub dependencies: HashMap<Ident, Descriptor>,

    #[serde(default)]
    #[serde(rename = "peerDependencies")]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub peer_dependencies: HashMap<Ident, PeerRange>,

    #[serde(default)]
    #[serde(rename = "optionalDependencies")]
    #[serde(skip_serializing_if = "HashSet::is_empty")]
    pub optional_dependencies: HashSet<Ident>,

    #[serde(default)]
    #[serde(skip_serializing_if = "HashSet::is_empty")]
    pub missing_peer_dependencies: HashSet<Ident>,
}

impl Resolution {
    pub fn from_remote_manifest(locator: Locator, manifest: RemoteManifest) -> Resolution {
        let optional_dependencies
            = HashSet::from_iter(manifest.optional_dependencies.keys().cloned());

        let mut dependencies
            = manifest.dependencies;

        dependencies
            .extend(manifest.optional_dependencies);

        Resolution {
            locator,
            version: manifest.version,
            dependencies,
            peer_dependencies: manifest.peer_dependencies,
            optional_dependencies,
            missing_peer_dependencies: HashSet::new(),
            requirements: manifest.requirements,
        }
    }
}

impl IntoResolutionResult for Resolution {
    fn into_resolution_result(mut self, context: &InstallContext<'_>) -> ResolutionResult {
        let original_resolution = self.clone();

        let (dependencies, peer_dependencies)
            = normalize_resolutions(context, &self);

        self.dependencies = dependencies;
        self.peer_dependencies = peer_dependencies;

        ResolutionResult {
            resolution: self,
            original_resolution,
            package_data: None,
        }
    }
}

pub async fn resolve(context: InstallContext<'_>, descriptor: Descriptor, dependencies: Vec<InstallOpResult>) -> Result<ResolutionResult, Error> {
    resolve_direct(&context, descriptor, dependencies).await
}

async fn resolve_direct(context: &InstallContext<'_>, descriptor: Descriptor, dependencies: Vec<InstallOpResult>) -> Result<ResolutionResult, Error> {
    match &descriptor.range {
        Range::SemverOrWorkspace(range)
            => semver::resolve_descriptor(context, &descriptor.ident, range).await,

        Range::Git(range)
            => git::resolve_descriptor(context, descriptor.ident, range).await,

        Range::Semver(range)
            => npm::resolve_semver_descriptor(context, &descriptor.ident, range).await,

        Range::SemverAlias(ident, range)
            => npm::resolve_semver_descriptor(context, ident, range).await,

        Range::Link(path)
            => link::resolve_descriptor(context, &descriptor.ident, path, &descriptor.parent),

        Range::Url(url)
            => url::resolve_descriptor(context, descriptor.ident, url).await,

        Range::Patch(_, file)
            => patch::resolve_descriptor(context, descriptor.ident, file, &descriptor.parent, dependencies).await,

        Range::Tarball(path)
            => tarball::resolve_descriptor(context, descriptor.ident, path, &descriptor.parent, dependencies).await,

        Range::Folder(path)
            => folder::resolve_descriptor(context, descriptor.ident, path, &descriptor.parent, dependencies).await,

        Range::Portal(path)
            => portal::resolve_descriptor(context, &descriptor.ident, path, &descriptor.parent, dependencies),

        Range::SemverTag(tag)
            => npm::resolve_tag_descriptor(context, descriptor.ident, tag).await,

        Range::WorkspaceMagic(_)
            => workspace::resolve_name_descriptor(context, descriptor.ident),

        Range::WorkspaceSemver(_)
            => workspace::resolve_name_descriptor(context, descriptor.ident),

        Range::WorkspacePath(path)
            => workspace::resolve_path_descriptor(context, path),

        _ => Err(Error::Unsupported),
    }
}
