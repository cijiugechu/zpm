use crate::{error::Error, fetchers, install::{InstallContext, IntoResolutionResult, ResolutionResult}, primitives::{Ident, Locator, Reference}};

pub async fn resolve_descriptor(context: &InstallContext<'_>, ident: Ident, url: &str) -> Result<ResolutionResult, Error> {
    let locator = Locator::new(ident.clone(), Reference::Url(url.to_string()));

    let fetch_result
        = fetchers::url::fetch_locator(context, &locator, url).await?;

    Ok(fetch_result.into_resolution_result(context))
}
