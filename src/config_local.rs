use anyhow::{anyhow, Context, Result};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt::Debug,
    fs::{self, File, OpenOptions},
    io::Write,
    path::PathBuf,
};

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ProjectConfig {
    pub name: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct JobConfig {
    pub run: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, EnvValue>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct SecretEnv {
    pub secret: String,
    pub version: i32,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum EnvValue {
    AsSecret(SecretEnv),
    AsString(String),
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Container {
    /// Container image to run the build in
    pub image: String,
    #[serde(default)]
    /// Directory absolute paths to persist between builds
    pub persist: Vec<String>,
}

// What's stored in their repo directory
#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct LocalConfig {
    pub project: Option<ProjectConfig>,
    #[serde(default)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub jobs: HashMap<String, JobConfig>,
    #[serde(default)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub containers: HashMap<String, Container>,
}

impl LocalConfig {
    pub fn jobs(&self) -> Vec<(String, JobConfig)> {
        self.jobs
            .keys()
            .map(|k| (k.clone(), self.jobs.get(k).unwrap().clone()))
            .collect_vec()
    }

    pub fn project(&self) -> ProjectConfig {
        match self.project.clone() {
            Some(p) => p,
            None => ProjectConfig { name: None },
        }
    }
}

pub const LOCAL_CONFIG_NAME: &str = "buildrecall.toml";

fn ensure_local_config_file(dir: PathBuf) -> Result<File> {
    fs::create_dir_all(dir.clone()).context(format!("Failed to create dir {:?}", dir.clone()))?;
    let filepath = dir.join(LOCAL_CONFIG_NAME);

    let f = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(filepath.clone())
        .context(format!(
            "Failed to create and open config file {:?}",
            filepath
        ))?;

    Ok(f)
}

pub fn read_local_config(dir: PathBuf) -> Result<LocalConfig> {
    ensure_local_config_file(dir.clone())?;

    fs::create_dir_all(dir.clone())?;
    let filepath = dir.join(LOCAL_CONFIG_NAME);
    let f = fs::read_to_string(filepath.clone())
        .context(format!("Can't read path {:?}", filepath))
        .unwrap();
    let config: LocalConfig = toml::from_str(f.as_str()).context("Failed to parse toml")?;

    Ok(config)
}

pub fn overwrite_local_config(
    dir: PathBuf,
    f: impl FnOnce(LocalConfig) -> LocalConfig,
) -> Result<()> {
    let current = read_local_config(dir.clone())?;
    let next_config = f(current);

    let t = match toml::to_string_pretty(&next_config) {
        Ok(str) => str,
        Err(e) => {
            return Err(anyhow!(
                "Failed to serialize the global config to TOML. Got error: '{}'",
                e.to_string()
            ))
        }
    };

    fs::create_dir_all(dir.clone())?;
    let filepath = dir.join(LOCAL_CONFIG_NAME);

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(filepath.clone())
        .context(format!("Failed to open config file {:?}", filepath.clone()))?;

    file.write_all(t.as_bytes())
        .context(format!("Failed to write to config file {:?}", filepath))?;

    Ok(())
}
