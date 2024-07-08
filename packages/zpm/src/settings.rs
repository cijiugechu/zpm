use arca::Path;
use serde::Deserialize;
use zpm_macros::yarn_config;
use crate::config::{BoolField, EnumField, GlobField, PathField, StringField, VecField};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PnpFallbackMode {
    None,
    DependenciesOnly,
    All,
}

#[yarn_config]
pub struct UserConfig {
}

#[yarn_config]
pub struct ProjectConfig {
    #[default(false)]
    pub enable_global_cache: BoolField,

    #[default("https://registry.npmjs.org".to_string())]
    pub npm_registry_server: StringField,

    #[default(crate::path::home(&Path::from(".yarn/zpm")))]
    pub global_folder: PathField,

    #[default(PnpFallbackMode::All)]
    pub pnp_fallback_mode: EnumField<PnpFallbackMode>,

    #[default(true)]
    pub pnp_enable_inlining: BoolField,

    #[default(vec![])]
    pub pnp_ignore_patterns: VecField<GlobField>,

    #[default("#!/usr/bin/env node".to_string())]
    pub pnp_shebang: StringField,
}
