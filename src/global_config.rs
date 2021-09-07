use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Deserialize, Serialize)]
pub struct RepoConfig {
    path: String,
}

// What's stored in their home directory
#[derive(Deserialize, Serialize)]
pub struct GlobalConfig {
    pub access_token: Option<String>,
    pub repos: Option<Vec<RepoConfig>>,
}

fn get_global_dir() -> Result<PathBuf> {
    // In the future, we could use a $BUILD_RECALL_ACCESS_TOKEN env instead of a config file if
    // this becomes a problem
    let home = std::env::var("HOME")
        .context("Build Recall creates a config file for you in your $HOME directory, but it can't the environment variable $HOME (aka: '~'). If you're using a system that doesn't have a $HOME for development, please reach out to us and we can add a workaround for you.")?;

    let dir = Path::new(&home).join(".buildrecall");

    Ok(dir)
}

pub fn read_global_config() -> Result<GlobalConfig> {
    let dir = get_global_dir()?;
    fs::create_dir_all(dir.clone())?;
    let filepath = dir.join("config");
    let f = fs::read_to_string(filepath).unwrap();
    let config: GlobalConfig = toml::from_str(f.as_str()).unwrap();

    Ok(config)
}

pub fn overwrite_global_config(f: impl FnOnce(GlobalConfig) -> GlobalConfig) -> Result<()> {
    let current = read_global_config()?;
    let next_config = f(current);

    let t = toml::to_string_pretty(&next_config).unwrap();

    let dir = get_global_dir()?;
    fs::create_dir_all(dir.clone())?;

    let filepath = dir.join("config");

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(filepath.clone())
        .context(format!("Failed to open config file {:?}", filepath))?;

    file.write_all(t.as_bytes())
        .context(format!("Failed to write to config file {:?}", filepath))
}
