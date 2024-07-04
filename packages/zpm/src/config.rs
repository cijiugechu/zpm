use arca::{Path, ToArcaPath};
use serde::{Deserialize, Deserializer};

use crate::{error::Error, primitives::Ident, settings::{ProjectConfig, UserConfig}};

#[derive(Debug, Default, Clone)]
pub enum SettingSource {
    #[default]
    Default,
    User,
    Project,
    Env,
}

#[derive(Debug, Default, Clone)]
pub struct Setting<T> {
    pub value: T,
    pub source: SettingSource,
}

impl<'de, T> Deserialize<'de> for Setting<T> where T: Deserialize<'de> {
    fn deserialize<D>(deserializer: D) -> Result<Setting<T>, D::Error> where D: Deserializer<'de> {
        let value = T::deserialize(deserializer)?;

        Ok(Setting {
            value,
            source: SettingSource::Default,
        })
    }
}

pub trait HydrateSetting<T> {
    fn hydrate_setting_from_env(raw: &str) -> Result<T, Error>;
}

impl HydrateSetting<String> for String {
    fn hydrate_setting_from_env(raw: &str) -> Result<String, Error> {
        Ok(raw.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct Glob {
    pub pattern: String,
}

impl Glob {
    pub fn to_regex_string(&self) -> String {
        wax::Glob::new(&self.pattern)
            .unwrap()
            .to_regex()
            .to_string()
    }
}

impl<'t> HydrateSetting<Glob> for Glob {
    fn hydrate_setting_from_env(raw: &str) -> Result<Glob, Error> {
        Ok(Glob { pattern: raw.to_string() })
    }
}

impl<'de> Deserialize<'de> for Glob {
    fn deserialize<D>(deserializer: D) -> Result<Glob, D::Error> where D: Deserializer<'de> {
        Ok(Glob { pattern: String::deserialize(deserializer)? })
    }
}

impl HydrateSetting<bool> for bool {
    fn hydrate_setting_from_env(raw: &str) -> Result<bool, Error> {
        match raw {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => panic!("Invalid boolean value: {}", raw),
        }
    }
}

pub struct Config {
    pub user: UserConfig,
    pub project: ProjectConfig,
}

impl Config {
    fn import_config<'a, T>(path: Option<Path>) -> T where T: for<'de> Deserialize<'de> {
        let content = path
            .and_then(|path| path.fs_read_text().ok())
            .unwrap_or_default();

        serde_yaml::from_str::<T>(&content)
            .unwrap()
    }

    pub fn new(cwd: Option<Path>) -> Self {
        let user_yarnrc_path = std::env::home_dir()
            .map(|dir| dir.to_arca().with_join_str(".yarnrc.yml"));

        let project_yarnrc_path = cwd
            .map(|cwd| cwd.with_join_str(".yarnrc.yml"));

        Config {
            user: Config::import_config::<UserConfig>(user_yarnrc_path),
            project: Config::import_config::<ProjectConfig>(project_yarnrc_path),
        }
    }

    pub fn registry_url_for(&self, _ident: &Ident) -> String {
        self.project.npm_registry_server.value.clone()
    }
}
