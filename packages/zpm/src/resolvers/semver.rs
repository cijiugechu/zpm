use crate::{error::Error, install::{InstallContext, ResolutionResult}, primitives::Ident, semver};

use super::{npm, workspace};

pub async fn resolve_descriptor(context: &InstallContext<'_>, ident: &Ident, range: &semver::Range) -> Result<ResolutionResult, Error> {
    let project = context.project
        .expect("The project is required for resolving a workspace package");

    if project.config.project.enable_transparent_workspaces.value {
        if let Ok(workspace) = workspace::resolve_name_descriptor(context, ident.clone()) {
            if range.check(&workspace.resolution.version) {
                return Ok(workspace);
            }
        }
    }

    npm::resolve_semver_descriptor(context, ident, range).await
}

