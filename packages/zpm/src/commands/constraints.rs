use std::{collections::BTreeMap, fs::Permissions, os::unix::fs::PermissionsExt};

use clipanion::cli;
use colored::Colorize;
use convert_case::{Case, Casing};
use serde::{Deserialize, Serialize};
use zpm_utils::{Path, ToHumanString, FromFileString};
use zpm_parsers::{JsonFormatter, JsonValue, JsonPath};

use crate::{constraints::{structs::{ConstraintsContext, WorkspaceError, ConstraintsOutput, WorkspaceOperation}, to_constraints_package, to_constraints_workspace}, error::Error, install::InstallState, manifest::Manifest, primitives::{Ident, Locator, Reference}, project::{Project, Workspace}, resolvers::Resolution, script::ScriptEnvironment, settings::ProjectConfigType, ui::tree};

#[cli::command]
#[cli::path("constraints")]
#[cli::category("Dependency management")]
pub struct Constraints {
    #[cli::option("-f,--fix", default = false)]
    fix: bool,
}

impl Constraints {
    #[tokio::main]
    pub async fn execute(&self) -> Result<(), Error> {
        let mut project
            = Project::new(None).await?;

        let max_loops = if self.fix {
            10
        } else {
            1
        };

        for loop_idx in 1..=max_loops {
            project
                .import_install_state()?
                .lazy_install().await?;

            let install_state
                = project.install_state.as_ref().unwrap();

            let constraints_workspaces
                = project.workspaces.iter()
                    .map(|workspace| to_constraints_workspace(workspace))
                    .collect::<Result<Vec<_>, _>>()?;

            let constraints_packages
                = install_state.normalized_resolutions.iter()
                    .map(|(_, resolution)| to_constraints_package(&project, install_state, resolution))
                    .collect::<Vec<_>>();

            let constraints_context = ConstraintsContext {
                workspaces: constraints_workspaces,
                packages: constraints_packages,
            };

            let config_path = project.project_cwd
                .with_join_str("yarn.config.cjs");

            let script
                = generate_constraints_adapter(&config_path, &constraints_context, self.fix);

            let output = ScriptEnvironment::new()?
                .with_project(&project)
                .with_stdin(Some(script))
                .run_exec("node", &vec!["-"])
                .await
                .ok()?;

            let stdout
                = &output.output()
                    .stdout;

            let output
                = serde_json::from_slice::<ConstraintsOutput>(&stdout).unwrap();

            for (workspace_rel_path, operations) in &output.all_workspace_operations {
                // Read the current manifest
                let manifest_path = project.project_cwd
                    .with_join(workspace_rel_path)
                    .with_join_str("package.json");
                
                let manifest_content = manifest_path
                    .fs_read_text_prealloc()?;

                let mut formatter
                    = JsonFormatter::from(&manifest_content).unwrap();
                
                // Apply each operation
                for operation in operations {
                    match operation {
                        WorkspaceOperation::Set { path, value } => {
                            formatter.set(&path.clone().into(), value.clone().into()).unwrap();
                        },

                        WorkspaceOperation::Unset { path } => {
                            formatter.remove(&path.clone().into()).unwrap();
                        },
                    }
                }
                
                // Write the formatted result back
                let updated_content
                    = formatter.to_string();

                manifest_path
                    .fs_change(&updated_content, Permissions::from_mode(0o644))?;
            }

            let should_break = false
                || output.all_workspace_operations.is_empty()
                || output.all_workspace_errors.is_empty()
                || loop_idx == max_loops;

            if should_break {
                if !output.all_workspace_errors.is_empty() {
                    display_report(&project, &output)?;
                }

                break;
            }
        }

        Ok(())
    }
}

fn generate_constraints_adapter(config_path: &Path, context: &ConstraintsContext, fix: bool) -> String {
    vec![
        "\"use strict\";\n",
        "\n",
        "const CONFIG_PATH =\n",
        &sonic_rs::to_string(&config_path).unwrap(), ";\n",
        "const SERIALIZED_CONTEXT =\n",
        &sonic_rs::to_string(&sonic_rs::to_string_pretty(&context).unwrap()).unwrap(), ";\n",
        &format!("const FIX = {};\n", fix),
        "\n",
        std::include_str!("constraints.tpl.js"),
    ].join("")
}

fn display_report(project: &Project, output: &ConstraintsOutput) -> Result<(), Error> {
    let mut root = tree::Node {
        label: ".".to_string(),
        children: vec![],
    };

    let cog
        = "⚙".truecolor(130, 130, 130).to_string();

    for (workspace_rel_path, errors) in &output.all_workspace_errors {
        let workspace
            = project.workspace_by_rel_path(&workspace_rel_path)?;

        let mut workspace_node = tree::Node {
            label: workspace.name.to_print_string(),
            children: vec![],
        };

        for error in errors {
            match error {
                WorkspaceError::MissingField { field_path, expected } => {
                    workspace_node.children.push(tree::Node {
                        label: format!("{cog} Missing field at {}; expected {}", field_path, expected),
                        children: vec![],
                    });
                },

                WorkspaceError::ExtraneousField { field_path, current_value } => {
                    workspace_node.children.push(tree::Node {
                        label: format!("{cog} Extraneous field at {}; current value {}", field_path, current_value),
                        children: vec![],
                    });
                },

                WorkspaceError::InvalidField { field_path, expected, current_value } => {
                    workspace_node.children.push(tree::Node {
                        label: format!("{cog} Invalid field at {}; expected {}, but got {}", field_path, expected, current_value),
                        children: vec![],
                    });
                },

                WorkspaceError::ConflictingValues { field_path, values } => {
                    let options = values.iter()
                        .flat_map(|(value, callers)| callers.iter().map(|caller| format!("{} as {:?}", value.to_print_string(), caller)))
                        .map(|option| tree::Node {label: option, children: vec![]})
                        .collect::<Vec<_>>();

                    workspace_node.children.push(tree::Node {
                        label: format!("Conflicting values at {}; expected values are:", field_path),
                        children: options,
                    });
                },

                WorkspaceError::UserError { message } => {
                    workspace_node.children.push(tree::Node {
                        label: message.to_string(),
                        children: vec![],
                    });
                },
            }
        }

        root.children.push(workspace_node);
    }

    println!("{}", root.to_string());

    Ok(())
}
