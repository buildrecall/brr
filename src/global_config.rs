use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fmt::Debug,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct RepoConfig {
    pub id: uuid::Uuid,
    pub name: String,
    pub path: PathBuf,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ConnectionConfig {
    pub access_token: Option<String>,
    pub host: Option<String>,
}

// What's stored in their home directory
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct GlobalConfig {
    pub connection: Option<ConnectionConfig>,

    // To prevent "values emitted after tables, this repos needs"
    // to happen after everything else.
    pub repos: Option<Vec<RepoConfig>>,
}

impl GlobalConfig {
    pub fn host(&self) -> Option<String> {
        self.connection.clone()?.host
    }

    pub fn access_token(&self) -> Option<String> {
        self.connection.clone()?.access_token
    }
}

fn ensure_global_config_file(dir: PathBuf) -> Result<()> {
    fs::create_dir_all(dir.clone()).context(format!("Failed to create dir {:?}", dir.clone()))?;
    let filepath = dir.join("config");

    let _ = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(filepath.clone())
        .context(format!(
            "Failed to create and open config file {:?}",
            filepath
        ))?;

    Ok(())
}

pub fn get_global_config_dir() -> Result<PathBuf> {
    // In the future, we could use a $BUILD_RECALL_ACCESS_TOKEN env instead of a config file if
    // this becomes a problem
    let home = dirs::home_dir().ok_or(anyhow::anyhow!(
        "Build Recall creates a config file for you in your $HOME directory, but it
can't the environment variable $HOME (aka: '~'). If you're using a system that
doesn't have a $HOME for development, please reach out to us and we can add a
workaround for you.",
    ))?;

    let dir = Path::new(&home).join(".buildrecall");

    Ok(dir)
}

pub fn read_global_config(dir: PathBuf) -> Result<GlobalConfig> {
    ensure_global_config_file(dir.clone())?;

    fs::create_dir_all(dir.clone())?;
    let filepath = dir.join("config");
    let f = fs::read_to_string(filepath.clone())
        .context(format!("Can't read path {:?}", filepath))
        .unwrap();
    let config: GlobalConfig = toml::from_str(f.as_str()).unwrap();

    Ok(config)
}

pub fn overwrite_global_config(
    dir: PathBuf,
    f: impl FnOnce(GlobalConfig) -> GlobalConfig,
) -> Result<()> {
    let current = read_global_config(dir.clone())?;
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
    let filepath = dir.join("config");

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(filepath.clone())
        .context(format!("Failed to open config file {:?}", filepath))?;

    file.write_all(t.as_bytes())
        .context(format!("Failed to write to config file {:?}", filepath))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use anyhow::Context;
    use tempdir::TempDir;

    use crate::global_config::{
        overwrite_global_config, read_global_config, ConnectionConfig, GlobalConfig,
    };

    #[test]
    fn test_read_creates_file_if_not_exist() {
        let tmp = TempDir::new(".buildrecall")
            .context("Can't create a tmp dir")
            .unwrap();
        let dir = tmp.into_path();
        let _ = read_global_config(dir.clone())
            .context("Can't read global config")
            .unwrap();

        assert!(Path::new(&dir).join("config").metadata().is_ok());
    }

    #[test]
    fn test_ensure_can_write_to_config() {
        let tmp = TempDir::new(".buildrecall")
            .context("Can't create a tmp dir")
            .unwrap();
        let dir = tmp.into_path();

        let _ = overwrite_global_config(dir.clone(), |c| GlobalConfig {
            connection: Some(ConnectionConfig {
                access_token: Some("i-am-test".to_string()),
                host: c.host(),
            }),
            repos: c.repos,
        });

        let written_config = read_global_config(dir.clone())
            .context("Can't read global config")
            .unwrap();

        assert_eq!(written_config.access_token(), Some("i-am-test".to_string()));
    }
}
