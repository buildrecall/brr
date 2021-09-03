use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
};

// What's stored in their home directory
#[derive(Deserialize, Serialize)]
pub struct GlobalConfig {
    pub access_token: Option<String>,
}

pub fn read_global_config() -> Result<GlobalConfig> {
    fs::create_dir_all("~/.buildrecall")?;
    let f = fs::read_to_string("~/.buildrecall/config").unwrap();
    let config: GlobalConfig = toml::from_str(f.as_str()).unwrap();

    Ok(config)
}

pub fn overwrite_global_config(c: GlobalConfig) -> Result<()> {
    let t = toml::to_string_pretty(&c).unwrap();

    fs::create_dir_all("~/.buildrecall")?;

    println!("Creating");

    let filepath = "~/.buildrecall/config";

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(filepath)
        .context(format!("Failed to open config file {}", filepath))?;

    file.write_all(t.as_bytes())
        .context(format!("Failed to write to config file {}", filepath))
}
