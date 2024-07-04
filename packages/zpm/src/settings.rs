use zpm_macros::yarn_config;
use crate::config;
use crate::config::HydrateSetting;

#[yarn_config]
pub struct UserConfig {
}

#[yarn_config]
pub struct ProjectConfig {
    #[default("https://registry.npmjs.org".to_string())]
    pub npm_registry_server: String,

    #[default(true)]
    pub pnp_enable_inlining: bool,

    #[array]
    pub pnp_ignore_patterns: config::Glob,

    #[default("#!/usr/bin/env node".to_string())]
    pub pnp_shebang: String,
}
