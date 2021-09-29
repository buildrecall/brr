use anyhow::{anyhow, Context, Result};
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
    pub name: Option<String>,
    pub run: Option<String>,
    pub artifacts: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
}

// What's stored in their repo directory
#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct LocalConfig {
    pub project: Option<ProjectConfig>,
    pub jobs: Option<Vec<JobConfig>>,
}

impl LocalConfig {
    pub fn jobs(&self) -> Vec<JobConfig> {
        self.jobs.clone().unwrap_or(vec![])
    }

    pub fn project(&self) -> ProjectConfig {
        match self.project.clone() {
            Some(p) => p,
            None => ProjectConfig { name: None },
        }
    }
}

const LOCAL_CONFIG_NAME: &str = "buildrecall.toml";

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
    let config: LocalConfig = toml::from_str(f.as_str()).unwrap();

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
