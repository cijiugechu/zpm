use structs::{ConstraintsPackage, ConstraintsWorkspace};

use crate::{error::Error, install::InstallState, primitives::Reference, project::{Project, Workspace}, resolvers::Resolution};

pub mod apply;
pub mod structs;

pub fn to_constraints_workspace<'a>(workspace: &'a Workspace) -> Result<ConstraintsWorkspace, Error> {
    Ok(ConstraintsWorkspace {
        cwd: workspace.rel_path.clone(),
        ident: workspace.name.clone(),
    })
}

pub fn to_constraints_package<'a>(project: &'a Project, install_state: &'a InstallState, resolution: &'a Resolution) -> ConstraintsPackage<'a> {
    let dependencies = resolution.dependencies.iter()
        .map(|(ident, descriptor)| {
            (ident, install_state.resolution_tree.descriptor_to_locator.get(descriptor).unwrap())
        }).collect::<Vec<_>>();

    let workspace = if let Reference::Workspace(params) = &resolution.locator.reference {
        Some(project.workspace_by_ident(&params.ident).unwrap().rel_path.clone())
    } else {
        None
    };

    ConstraintsPackage {
        locator: resolution.locator.clone(),
        workspace,
        ident: resolution.locator.ident.clone(),
        version: resolution.version.clone(),
        dependencies,
        peer_dependencies: vec![],
    }
}
