use crate::{error::Error, fetchers, git::{resolve_git_treeish, GitRange, GitReference}, install::{InstallContext, IntoResolutionResult, ResolutionResult}, primitives::{Ident, Locator, Reference}};

pub async fn resolve_descriptor(context: &InstallContext<'_>, ident: Ident, source: &GitRange) -> Result<ResolutionResult, Error> {
    let commit = resolve_git_treeish(source).await?;

    let git_reference = GitReference {
        repo: source.repo.clone(),
        commit: commit.clone(),
        prepare_params: source.prepare_params.clone(),
    };

    let locator
        = Locator::new(ident, Reference::Git(git_reference.clone()));

    let fetch_result
        = fetchers::git::fetch_locator(context, &locator, &git_reference).await?;

    Ok(fetch_result.into_resolution_result(context))
}
