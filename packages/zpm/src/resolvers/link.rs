use std::{collections::{HashMap, HashSet}, sync::Arc};

use crate::{error::Error, install::{InstallContext, IntoResolutionResult, ResolutionResult}, primitives::{Ident, Locator, Reference}, resolvers::Resolution, semver, system};

pub fn resolve_descriptor(ctx: &InstallContext<'_>, ident: &Ident, path: &str, parent: &Option<Locator>) -> Result<ResolutionResult, Error> {
    let resolution = Resolution {
        version: semver::Version::new(),
        locator: Locator::new_bound(ident.clone(), Reference::Link(path.to_string()), parent.clone().map(Arc::new)),
        dependencies: HashMap::new(),
        peer_dependencies: HashMap::new(),
        optional_dependencies: HashSet::new(),
        missing_peer_dependencies: HashSet::new(),
        requirements: system::Requirements::default(),
    };

    Ok(resolution.into_resolution_result(ctx))
}
