use std::{collections::HashMap, fmt, marker::PhantomData, str::FromStr, sync::{Arc, LazyLock}};

use regex::Regex;
use serde::{de::{self, DeserializeSeed, IgnoredAny, Visitor}, Deserialize, Deserializer};

use crate::{error::Error, http::http_client, install::{InstallContext, IntoResolutionResult, ResolutionResult}, manifest::RemoteManifest, primitives::{Descriptor, Ident, Locator, Range, Reference}, resolvers::Resolution, semver};

static NODE_GYP_IDENT: LazyLock<Ident> = LazyLock::new(|| Ident::from_str("node-gyp").unwrap());
static NODE_GYP_MATCH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(node-gyp|prebuild-install)\b").unwrap());

pub async fn resolve_semver_descriptor(context: &InstallContext<'_>, ident: &Ident, range: &semver::Range) -> Result<ResolutionResult, Error> {
    pub struct FindField<'a, T> {
        field: &'a str,
        nested: T,
    }
    
    impl<'de, T> Visitor<'de> for FindField<'_, T> where T: DeserializeSeed<'de> + Clone {
        type Value = T::Value;
    
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            write!(formatter, "a map with a matching field")
        }
    
        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error> where A: de::MapAccess<'de> {
            let mut selected = None;
    
            while let Some(key) = map.next_key::<String>()? {
                if key == self.field {
                    selected = Some(map.next_value_seed(self.nested.clone())?);
                } else {
                    let _ = map.next_value::<IgnoredAny>();
                }
            }
    
            selected
                .ok_or(de::Error::missing_field(""))
        }
    }
    
    #[derive(Clone)]
    pub struct FindVersion<T> {
        range: semver::Range,
        phantom: PhantomData<T>,
    }
    
    impl<'de, T> DeserializeSeed<'de> for FindVersion<T> where T: Deserialize<'de> {
        type Value = (semver::Version, T);
    
        fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error> where D: Deserializer<'de> {
            deserializer.deserialize_map(self)
        }
    }
    
    impl<'de, T> Visitor<'de> for FindVersion<T> where T: Deserialize<'de> {
        type Value = (semver::Version, T);
    
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            write!(formatter, "a map with a matching version")
        }
    
        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error> where A: de::MapAccess<'de> {
            let mut selected = None;
    
            while let Some(key) = map.next_key::<String>()? {
                let version = semver::Version::from_str(key.as_str()).unwrap();

                if self.range.check(&version) && selected.as_ref().map(|(current_version, _)| *current_version < version).unwrap_or(true) {
                    selected = Some((version, map.next_value::<serde_json::Value>()?));
                } else {
                    map.next_value::<IgnoredAny>()?;
                }
            }
    
            Ok(selected.map(|(version, version_payload)| {
                (version, T::deserialize(version_payload).unwrap())
            }).unwrap())
        }
    }

    let project = context.project
        .expect("The project is required for resolving a workspace package");

    let client = http_client()?;

    let registry_url = project.config.registry_url_for(ident);
    let url = format!("{}/{}", registry_url, ident);

    let response = client.get(url.clone()).send().await
        .map_err(|err| Error::RemoteRegistryError(Arc::new(err)))?;

    if response.status().as_u16() == 404 {
        return Err(Error::PackageNotFound(ident.clone(), url));
    }
 
    let registry_text = response.text().await
        .map_err(|err| Error::RemoteRegistryError(Arc::new(err)))?;

    let mut deserializer
        = serde_json::Deserializer::from_str(registry_text.as_str());

    #[derive(Clone, Deserialize)]
    struct RemoteManifestWithScripts {
        #[serde(flatten)]
        remote: RemoteManifest,

        #[serde(default)]
        #[serde(skip_serializing_if = "HashMap::is_empty")]
        scripts: HashMap<String, String>,
    }

    let manifest_result = deserializer.deserialize_map(FindField {
        field: "versions",
        nested: FindVersion {
            range: range.clone(),
            phantom: PhantomData::<RemoteManifestWithScripts>,
        },
    });

    let (version, mut manifest) = manifest_result
        .map_err(|_| Error::NoCandidatesFound(Range::Semver(range.clone())))?;

    // Manually add node-gyp dependency if there is a script using it and not already set
    // This is because the npm registry will automatically add a `node-gyp rebuild` install script
    // in the metadata if there is not already an install script and a binding.gyp file exists.
    // Also, node-gyp is not always set as a dependency in packages, so it will also be added if used in scripts.
    //
    if !manifest.remote.dependencies.contains_key(&NODE_GYP_IDENT) && !manifest.remote.peer_dependencies.contains_key(&NODE_GYP_IDENT) {
        for script in manifest.scripts.values() {
            if NODE_GYP_MATCH.is_match(script.as_str()) {
                manifest.remote.dependencies.insert(NODE_GYP_IDENT.clone(), Descriptor::new_semver(NODE_GYP_IDENT.clone(), "*").unwrap());
                break;
            }
        }
    }

    let dist_manifest = manifest.remote.dist
        .as_ref()
        .expect("Expected the registry to return a 'dist' field amongst the manifest data");

    let expected_registry_url
        = project.config.registry_url_for_package_data(ident, &version);

    let reference = match expected_registry_url == dist_manifest.tarball {
        true => Reference::Semver(version),
        false => Reference::Url(dist_manifest.tarball.clone()),
    };

    let locator = Locator::new(ident.clone(), reference);
    let resolution = Resolution::from_remote_manifest(locator, manifest.remote);

    Ok(resolution.into_resolution_result(context))
}

pub async fn resolve_tag_descriptor(context: &InstallContext<'_>, ident: Ident, tag: &str) -> Result<ResolutionResult, Error> {
    let project = context.project
        .expect("The project is required for resolving a workspace package");

    let client = http_client()?;

    let registry_url = project.config.registry_url_for(&ident);
    let url = format!("{}/{}", registry_url, ident);

    let response = client.get(url.clone()).send().await
        .map_err(|err| Error::RemoteRegistryError(Arc::new(err)))?;

    let registry_text = response.text().await
        .map_err(|err| Error::RemoteRegistryError(Arc::new(err)))?;

    #[derive(Deserialize)]
    struct RegistryMetadata {
        #[serde(rename(deserialize = "dist-tags"))]
        dist_tags: HashMap<String, semver::Version>,
        versions: HashMap<semver::Version, RemoteManifest>,
    }

    let mut registry_data: RegistryMetadata = serde_json::from_str(registry_text.as_str())
        .map_err(Arc::new)?;

    let version = registry_data.dist_tags.get(tag)
        .ok_or_else(|| Error::MissingSemverTag(tag.to_string()))?;

    let manifest = registry_data.versions.remove(version).unwrap();

    let dist_manifest = manifest.dist
        .as_ref()
        .expect("Expected the registry to return a 'dist' field amongst the manifest data");

    let expected_registry_url
        = project.config.registry_url_for_package_data(&ident, &version);

    let reference = match expected_registry_url == dist_manifest.tarball {
        true => Reference::SemverAlias(ident.clone(), version.clone()),
        false => Reference::Url(dist_manifest.tarball.clone()),
    };

    let locator = Locator::new(ident.clone(), reference);
    let resolution = Resolution::from_remote_manifest(locator, manifest);

    Ok(resolution.into_resolution_result(context))
}
