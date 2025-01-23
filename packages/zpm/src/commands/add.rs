use clipanion::cli;

use crate::{error::Error, primitives::{descriptor::LooseDescriptor, Reference}, project};

#[cli::command]
#[cli::path("add")]
pub struct Add {
    descriptors: Vec<LooseDescriptor>,
}

impl Add {
    #[tokio::main()]
    pub async fn execute(&self) -> Result<(), Error> {
        let mut project
            = project::Project::new(None).await?;

        project
            .import_install_state()?;

        let active_package
            = project.active_package()?;

        if let Reference::Workspace(params) = &active_package.reference {
            let workspace_idx = project.workspaces_by_ident.get_mut(&params.ident)
                .expect("Expected the workspace to exist in the project instance");

            let workspace
                = &mut project.workspaces[*workspace_idx];

            for loose_descriptor in &self.descriptors {
                workspace.manifest.remote.dependencies.insert(loose_descriptor.descriptor.ident.clone(), loose_descriptor.descriptor.clone());
            }

            workspace.write_manifest()?;

            project.run_install().await?;
        }


        Ok(())
    }
}
