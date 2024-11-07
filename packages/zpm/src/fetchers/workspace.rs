use crate::{error::Error, install::{FetchResult, InstallContext}, primitives::Ident};

use super::PackageData;

pub fn fetch_locator(context: &InstallContext<'_>, ident: &Ident) -> Result<FetchResult, Error> {
    let project = context.project
        .expect("The project is required for fetching a workspace package");

    let workspace = project.workspaces
        .get(ident)
        .ok_or_else(|| Error::WorkspaceNotFound(ident.clone()))?;

    Ok(FetchResult::new(PackageData::Local {
        package_directory: workspace.path.clone(),
        discard_from_lookup: false,
    }))
}
