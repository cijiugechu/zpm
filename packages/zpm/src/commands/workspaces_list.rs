use clipanion::cli;

use crate::{error::Error, primitives::Ident, project};

#[cli::command]
#[cli::path("workspaces", "list")]
#[cli::category("Workspace commands")]
#[cli::description("List the workspaces in the project")]
pub struct WorkspacesList {
    #[cli::option("--json", default = false)]
    json: bool,
}

impl WorkspacesList {
    #[tokio::main()]
    pub async fn execute(&self) -> Result<(), Error> {
        let project
            = project::Project::new(None).await?;

        for workspace in &project.workspaces {
            let workspace_path_str
                = workspace.rel_path.to_string();

            let workspace_printed_path = match workspace_path_str.is_empty() {
                true => ".",
                false => workspace_path_str.as_str(),
            };

            if self.json {
                #[derive(serde::Serialize)]
                struct Payload<'a> {
                    location: &'a str,
                    name: &'a Ident,
                }

                let payload = Payload {
                    location: workspace_printed_path,
                    name: &workspace.name,
                };

                println!("{}", sonic_rs::to_string(&payload)?);
            } else {
                println!("{}", workspace_printed_path);
            }
        }

        Ok(())
    }
}
