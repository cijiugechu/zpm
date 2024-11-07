use std::sync::Arc;

use crate::{error::Error, formats::zip::ZipSupport, install::{InstallContext, InstallOpResult, IntoResolutionResult, ResolutionResult}, manifest::parse_manifest, primitives::{Ident, Locator, Reference}, resolvers::Resolution};

pub fn resolve_descriptor(ctx: &InstallContext, ident: &Ident, path: &str, parent: &Option<Locator>, dependencies: Vec<InstallOpResult>) -> Result<ResolutionResult, Error> {
    let parent_data = dependencies[0].as_fetched();

    let parent = parent.as_ref()
        .expect("The parent locator is required for resolving a portal package");

    let package_directory = parent_data.package_data
        .context_directory()
        .with_join_str(path);

    let manifest_path = package_directory
        .with_join_str("package.json");
    let manifest_text = manifest_path
        .fs_read_text_with_zip()?;
    let manifest
        = parse_manifest(manifest_text)?;

    let locator = Locator::new_bound(ident.clone(), Reference::Portal(path.to_string()), Some(Arc::new(parent.clone())));
    let resolution = Resolution::from_remote_manifest(locator, manifest.remote);

    Ok(resolution.into_resolution_result(ctx))
}
