use std::{collections::HashMap, path::PathBuf};

use anyhow::{anyhow, Context, Result};
use clap::Clap;

use crate::{
    api::{ApiClient, BuildRecall},
    config_global::read_global_config,
    config_local::{overwrite_local_config, read_local_config, EnvValue, LocalConfig, SecretEnv},
};

/// Creates a new secret
/// If the secret already exists, creates a new version
#[derive(Clap, Debug)]
pub struct Set {
    #[clap()]
    name: String,

    #[clap()]
    value: String,
}

#[derive(Clap, Debug)]
pub enum SecretsSubCommand {
    #[clap()]
    Set(Set),
}

pub async fn run_secrets(
    set: SecretsSubCommand,
    global_config_dir: PathBuf,
    local_config_dir: PathBuf,
) -> Result<()> {
    match set {
        SecretsSubCommand::Set(s) => {
            let local = read_local_config(local_config_dir.clone())
                .context("Failed to read buildrecall.toml")?;
            let global = read_global_config(global_config_dir)?;
            let client = ApiClient::new(global.clone());

            let project_slug = local.project().name.ok_or(anyhow!(
                "Missing a project.name parameter in the buildrecall.toml"
            ))?;

            let new_secret = client
                .set_secret(project_slug, s.name, s.value)
                .await
                .context("Failed to set secret")?;

            overwrite_local_config(local_config_dir, |f| {
                let env = f.env();
                let mut map: HashMap<String, EnvValue> = HashMap::new();
                let new_secret_env = SecretEnv {
                    secret: new_secret.slug,
                    version: new_secret.version,
                };

                for key in env.keys() {
                    let val = match env.get(key).unwrap() {
                        crate::config_local::EnvValue::AsSecret(curr) => {
                            // If it's this secret, let's bump the version
                            if curr.secret.eq(key) {
                                EnvValue::AsSecret(new_secret_env.clone())
                            } else {
                                EnvValue::AsSecret(curr.clone())
                            }
                        }
                        crate::config_local::EnvValue::AsString(s) => {
                            EnvValue::AsString(s.to_owned())
                        }
                    };

                    map.insert(key.to_string(), val.to_owned());
                }

                print!("{:?}", map.clone());

                LocalConfig {
                    env: Some(map),
                    jobs: f.jobs,
                    project: f.project,
                }
            })?;

            Ok(())
        }
    }
}