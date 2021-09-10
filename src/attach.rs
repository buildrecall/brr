use std::{env, path::PathBuf};

use anyhow::{anyhow, Result};
use dialoguer::Confirm;

use crate::{
    api::{ApiClient, BuildRecall},
    global_config::{
        overwrite_global_config, read_global_config, ConnectionConfig, GlobalConfig, RepoConfig,
    },
};

pub struct AttachArguments {
    pub slug: Option<String>,
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

    let slug = args.slug.unwrap_or(folder);

    // check if global config already has this path.
    // In which case do nothing
    let empty = vec![];
    let configs = global_config.clone().repos.unwrap_or(empty);
    let existing = configs.iter().find(|r| r.path == path);

    let projects = client.list_projects().await?;

    match existing {
        Some(c) => {
            if projects.iter().find(|p| p.slug == c.name).is_none() {
                return Err(anyhow!("This folder is attached to a build farm that no longer exists.\n\nIt might have been deleted by someone else, or it might exist on a different account.\nIf you want to recreate it, detach and then re-attach this folder:\n\n\tbrr detach\n\tbrr attach {}\n", c.name));
            }

            return Err(anyhow!(
            "This folder is already attached to a build farm named '{}'. Perhaps you meant to detach it, like this:\n\n\tbrr detach\n", 
            c.name
        ));
        }
        None => {}
    }

    // Check if there's already a project
    let proj = projects.iter().find(|p| p.slug == slug.clone());
    if proj.is_some() {
        if !Confirm::new()
            .with_prompt(format!(
                "A project named '{}' already exists\nLink this project?",
                slug.clone()
            ))
            .default(true)
            .interact()?
        {
            return Ok(());
        }
    }

    client.create_project(slug.clone()).await?;

    overwrite_global_config(global_config_dir, move |c| {
        let mut repos: Vec<RepoConfig> = vec![RepoConfig {
            path: path,
            name: slug.clone(),
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
