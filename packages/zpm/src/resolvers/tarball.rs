use std::sync::Arc;

use crate::{error::Error, fetchers, install::{InstallContext, InstallOpResult, IntoResolutionResult, ResolutionResult}, primitives::{Ident, Locator, Reference}};

pub async fn resolve_descriptor(context: &InstallContext<'_>, ident: Ident, path: &str, parent: &Option<Locator>, dependencies: Vec<InstallOpResult>) -> Result<ResolutionResult, Error> {
    let locator = Locator::new_bound(ident, Reference::Tarball(path.to_string()), parent.clone().map(Arc::new));

    let fetch_result
        = fetchers::tarball::fetch_locator(context, &locator, path, dependencies).await?;

    Ok(fetch_result.into_resolution_result(context))
}
