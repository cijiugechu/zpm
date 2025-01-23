use arca::Path;
use clipanion::cli;

use crate::{error::Error, primitives::Ident, project};

#[cli::command]
#[cli::path("workspaces", "list")]
pub struct WorkspacesList {
    #[cli::option("--json", default = true)]
    json: bool,
}

impl WorkspacesList {
    #[tokio::main()]
    pub async fn execute(&self) -> Result<(), Error> {
        let project
            = project::Project::new(None).await?;

        for workspace in &project.workspaces {
            if self.json {
                #[derive(serde::Serialize)]
                struct Payload<'a> {
                    location: &'a Path,
                    name: &'a Ident,
                }

                let payload = Payload {
                    location: &workspace.rel_path,
                    name: &workspace.name,
                };

                println!("{}", sonic_rs::to_string(&payload)?);
            } else {
                println!("{}", workspace.rel_path);
            }
        }

        Ok(())
    }
}
