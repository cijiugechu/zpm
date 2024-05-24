use std::ffi::OsString;

use clap::{Parser, Subcommand};
use tokio::process::Command;

use crate::{error::Error, install::{InstallContext, InstallManager}, linker, project};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Install {},

    #[command(external_subcommand)]
    External(Vec<String>),
}

pub async fn run_cli() -> Result<(), Error> {
    let cli = Cli::parse();

    match &cli.command {
        None | Some(Commands::Install {}) => {
            let project
                = project::Project::new(None)?;

            let package_cache
                = project.package_cache();

            let install_context = InstallContext::default()
                .with_package_cache(Some(&package_cache))
                .with_project(Some(&project));

            let install = InstallManager::default()
                .with_context(install_context)
                .with_lockfile(project.lockfile()?)
                .with_roots_iter(project.workspaces.values().map(|w| w.descriptor()))
                .run().await?;

            project
                .write_lockfile(&install.lockfile)?;

            linker::link_project(&project, &install)
                .await?;
        }

        Some(Commands::External(args)) => {
            let project
                = project::Project::new(None)?;

            match args[0].as_str() {
                "node" => {
                    let mut command = Command::new("node");

                    command.args(&args[1..]);

                    let mut node_options = std::env::var("NODE_OPTIONS")
                        .unwrap_or_else(|_| "".to_string());

                    if let Some(pnp_path) = project.pnp_path().if_exists() {
                        node_options = format!("{} --require {}", node_options, pnp_path.to_string());
                    }

                    if let Some(pnp_loader_path) = project.pnp_loader_path().if_exists() {
                        node_options = format!("{} --experimental-loader {}", node_options, pnp_loader_path.to_string());
                    }

                    if node_options.len() > 0 {
                        command.env("NODE_OPTIONS", node_options);
                    }
        
                    command.status().await.unwrap();
                },

                _ => {
                    panic!("Unknown external subcommand: {}", args[0]);
                }
            };
        }
    }

    Ok(())
}
