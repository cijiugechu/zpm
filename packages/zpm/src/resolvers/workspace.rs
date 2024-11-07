use arca::Path;

use crate::{error::Error, install::{InstallContext, IntoResolutionResult, ResolutionResult}, primitives::{Ident, Locator, Reference}, resolvers::Resolution};

pub fn resolve_name_descriptor(context: &InstallContext<'_>, ident: Ident) -> Result<ResolutionResult, Error> {
    let project = context.project
        .expect("The project is required for resolving a workspace package");

    match project.workspaces.get(&ident) {
        Some(workspace) => {
            let manifest = workspace.manifest.clone();

            let locator = Locator::new(ident.clone(), Reference::Workspace(workspace.name.clone()));
            let mut resolution = Resolution::from_remote_manifest(locator, manifest.remote);

            resolution.dependencies.extend(manifest.dev_dependencies);

            Ok(resolution.into_resolution_result(context))
        }

        None => Err(Error::WorkspaceNotFound(ident)),
    }
}

pub fn resolve_path_descriptor(context: &InstallContext<'_>, path: &str) -> Result<ResolutionResult, Error> {
    let project = context.project
        .expect("The project is required for resolving a workspace package");

    if let Some(ident) = project.workspaces_by_rel_path.get(&Path::from(path)) {
        resolve_name_descriptor(context, ident.clone())
    } else {
        Err(Error::WorkspacePathNotFound())
    }
}
