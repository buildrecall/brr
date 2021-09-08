use std::{env, path::PathBuf};

use anyhow::{anyhow, Result};

use crate::{
    api::{ApiClient, BuildRecall},
    global_config::{
        overwrite_global_config, read_global_config, ConnectionConfig, GlobalConfig, RepoConfig,
    },
};

pub struct AttachArguments {
    pub slug: String,
}

pub async fn run_attach(global_config_dir: PathBuf, args: AttachArguments) -> Result<()> {
    let global_config = read_global_config(global_config_dir.clone())?;
    let client = ApiClient::new(global_config.clone());

    if global_config.clone().access_token().is_none() {
        return Err(anyhow!(
            "You're not logged in. You can login by going to https://buildrecall.com/setup"
        ));
    }

    let path = env::current_dir()?;
    let pieces = path
        .components()
        .map(|comp| comp.as_os_str().to_str().unwrap_or("").to_string())
        .collect::<Vec<_>>();
    let folder = pieces[pieces.len() - 1].clone();

    // check if global config already has this path.
    // In which case do nothing

    let existing = global_config
        .clone()
        .repos
        .unwrap_or(vec![])
        .iter()
        .find(|r| r.path == path)
        .is_some();

    if existing {
        return Err(anyhow!(
            "There's already a build farm attached to this folder. Perhaps you meant to detach it?"
        ));
    }

    client.create_project(args.slug.clone()).await?;

    overwrite_global_config(global_config_dir, move |c| {
        let mut repos: Vec<RepoConfig> = vec![RepoConfig {
            path: path,
            name: args.slug.clone(),
            id: uuid::Uuid::new_v4(),
        }];
        repos.extend(c.repos.unwrap_or(vec![]));

        GlobalConfig {
            connection: c.connection,
            repos: Some(repos),
        }
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_adds_a() {
        assert_eq!(2 + 2, 4);
    }
}
