use std::collections::BTreeMap;

use clipanion::cli;
use wax::{Glob, Program};

use crate::{error::Error, primitives::Ident, project::{Project, Workspace}};

#[cli::command]
#[cli::path("remove")]
pub struct Remove {
    #[cli::option("-A,--all", default = false)]
    all: bool,

    identifiers: Vec<Ident>,
}

impl Remove {
    #[tokio::main()]
    pub async fn execute(&self) -> Result<(), Error> {
        let mut project
            = Project::new(None).await?;

        let ident_globs = self.identifiers.iter()
            .map(|ident| Glob::new(ident.as_str()).unwrap())
            .collect::<Vec<_>>();

        if self.all {
            for workspace in project.workspaces.iter_mut() {
                self.remove_dependencies_from_manifest(workspace, &ident_globs)?;
            }
        } else {
            let active_workspace
                = project.active_workspace_mut()?;

            self.remove_dependencies_from_manifest(active_workspace, &ident_globs)?;
        }

        project.run_install().await?;

        Ok(())
    }

    fn remove_dependencies_from_manifest(&self, workspace: &mut Workspace, ident_globs: &[Glob]) -> Result<(), Error> {
        self.remove_dependencies_from_set(&mut workspace.manifest.remote.dependencies, ident_globs);
        self.remove_dependencies_from_set(&mut workspace.manifest.remote.optional_dependencies, ident_globs);
        self.remove_dependencies_from_set(&mut workspace.manifest.remote.peer_dependencies, ident_globs);
        self.remove_dependencies_from_set(&mut workspace.manifest.dev_dependencies, ident_globs);

        workspace.write_manifest()?;

        Ok(())
    }

    fn remove_dependencies_from_set<T>(&self, set: &mut BTreeMap<Ident, T>, ident_globs: &[Glob]) {
        for ident in set.keys().cloned().collect::<Vec<_>>() {
            for glob in ident_globs.iter() {
                if glob.is_match(ident.as_str()) {
                    set.remove(&ident);
                }
            }
        }
    }
}
