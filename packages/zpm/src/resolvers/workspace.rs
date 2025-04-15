use crate::{error::Error, install::{InstallContext, IntoResolutionResult, ResolutionResult}, primitives::{range, reference, Descriptor, Locator}, resolvers::Resolution};

pub fn resolve_name_descriptor(context: &InstallContext<'_>, descriptor: &Descriptor, params: &range::WorkspaceIdentRange) -> Result<ResolutionResult, Error> {
    let project = context.project
        .expect("The project is required for resolving a workspace package");

    let manifest = project.workspace_by_ident(&params.ident)?
        .manifest
        .clone();

    let reference = reference::WorkspaceReference {
        ident: params.ident.clone(),
    };

    let locator = descriptor.resolve_with(reference.into());
    let mut resolution = Resolution::from_remote_manifest(locator, manifest.remote);

    resolution.dependencies.extend(manifest.dev_dependencies);

    Ok(resolution.into_resolution_result(context))
}

pub fn resolve_path_descriptor(context: &InstallContext<'_>, descriptor: &Descriptor, params: &range::WorkspacePathRange) -> Result<ResolutionResult, Error> {
    let project = context.project
        .expect("The project is required for resolving a workspace package");

    let workspace = project.workspace_by_rel_path(&params.path)?;

    resolve_name_descriptor(context, descriptor, &range::WorkspaceIdentRange {ident: workspace.name.clone()})
}

pub fn resolve_locator(context: &InstallContext<'_>, locator: &Locator, params: &reference::WorkspaceReference) -> Result<ResolutionResult, Error> {
    let project = context.project
        .expect("The project is required for resolving a workspace package");

    let manifest = project
        .workspace_by_ident(&params.ident)?
        .manifest
        .clone();

    let mut resolution = Resolution::from_remote_manifest(locator.clone(), manifest.remote);

    resolution.dependencies.extend(manifest.dev_dependencies);

    Ok(resolution.into_resolution_result(context))
}
