use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use zpm_utils::{ColoredJsonValue, JsonPath, Path};

use crate::{manifest::Manifest, primitives::{Ident, Locator}};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Caller {
    pub file: Option<String>,
    pub method_name: String,
    pub arguments: Vec<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
#[serde(rename_all_fields = "camelCase")]
pub enum WorkspaceError {
    MissingField {
        field_path: JsonPath,
        expected: ColoredJsonValue,
    },

    ExtraneousField {
        field_path: JsonPath,
        current_value: ColoredJsonValue,
    },

    InvalidField {
        field_path: JsonPath,
        expected: ColoredJsonValue,
        current_value: ColoredJsonValue,
    },

    ConflictingValues {
        field_path: JsonPath,
        values: Vec<(ColoredJsonValue, Vec<Caller>)>,
    },

    UserError {
        message: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
#[serde(rename_all_fields = "camelCase")]
pub enum WorkspaceOperation {
    Set {
        path: Vec<String>,
        value: serde_json::Value,
    },
    Unset {
        path: Vec<String>,
    },
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ConstraintsOutput {
    #[serde_as(as = "Vec<(_, _)>")]
    pub all_workspace_operations: BTreeMap<Path, Vec<WorkspaceOperation>>,
    #[serde_as(as = "Vec<(_, _)>")]
    pub all_workspace_errors: BTreeMap<Path, Vec<WorkspaceError>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ConstraintsContext<'a> {
    pub workspaces: Vec<ConstraintsWorkspace>,
    pub packages: Vec<ConstraintsPackage<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConstraintsWorkspace {
    pub cwd: Path,
    pub ident: Ident,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConstraintsPackage<'a> {
    pub locator: Locator,
    pub workspace: Option<Path>,
    pub ident: Ident,
    pub version: zpm_semver::Version,
    pub dependencies: Vec<(&'a Ident, &'a Locator)>,
    pub peer_dependencies: Vec<(&'a Ident, &'a Locator)>,
}
