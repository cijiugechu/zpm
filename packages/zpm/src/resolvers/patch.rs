use std::sync::Arc;

use crate::{error::Error, fetchers, install::{InstallContext, InstallOpResult, IntoResolutionResult, ResolutionResult}, primitives::{Ident, Locator, Reference}, serialize::UrlEncoded};

pub async fn resolve_descriptor(context: &InstallContext<'_>, ident: Ident, path: &str, parent: &Option<Locator>, mut dependencies: Vec<InstallOpResult>) -> Result<ResolutionResult, Error> {
    let inner_locator
        = dependencies[1].as_resolved().resolution.locator.clone();

    let locator
        = Locator::new_bound(ident, Reference::Patch(Box::new(UrlEncoded::new(inner_locator)), path.to_string()), parent.clone().map(Arc::new));

    // We need to remove the "resolve" operation where we resolved the
    // descriptor into a locator before passing it to fetch
    dependencies.remove(1);

    let fetch_result
        = fetchers::patch::fetch_locator(context, &locator, path, dependencies).await?;

    Ok(fetch_result.into_resolution_result(context))
}
