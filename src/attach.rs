use std::{env, path::PathBuf};

use anyhow::{anyhow, Context, Result};
use dialoguer::Confirm;

use crate::{
    api::{ApiClient, BuildRecall, Project},
    git,
    global_config::{
        overwrite_global_config, read_global_config, ConnectionConfig, GlobalConfig, RepoConfig,
    },
};

pub struct AttachArguments {
    pub slug: Option<String>,
}

pub async fn run_attach(global_config_dir: PathBuf, args: AttachArguments) -> Result<()> {
    let global_config = read_global_config(global_config_dir.clone()).context(format!(
        "Failed to read the global config file: {:?}",
        global_config_dir
    ))?;
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
    let maybe_project = projects.iter().find(|p| p.slug == slug.clone());
    let project: Result<Project> = match maybe_project {
        Some(p) => {
            if !Confirm::new()
                .with_prompt(format!(
                    "A project named '{}' already exists\nLink this project?",
                    slug.clone()
                ))
                .default(true)
                .interact()?
            {
                return Ok(());
            } else {
                Ok(p.clone())
            }
        }
        None => {
            let p = client
                .create_project(slug.clone())
                .await
                .context("Failed to create the project in Buildrecall")?
                .clone();
            Ok(p)
        }
    };

    // create the project in the local config
    let project_config_id = project?.clone().id.clone();
    overwrite_global_config(global_config_dir.clone(), move |c| {
        let mut repos: Vec<RepoConfig> = vec![RepoConfig {
            path: path,
            name: slug.clone(),
            id: project_config_id,
        }];
        repos.extend(c.repos.unwrap_or(vec![]));

        GlobalConfig {
            connection: c.connection,
            repos: Some(repos),
        }
    })
    .context("Failed to store this project in the global config file")?;

    // create a .git folder for brr to use that doesn't mess with the user's git.
    let g = git::RecallGit::new(global_config_dir.clone())?;
    g.create_shadow_git_folder(project_config_id)
        .context(format!("Failed to create a shadow git folder (used to sync files without messing with your own git setup) in {:?}/{}", global_config_dir, ".gits"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_adds_a() {
        assert_eq!(2 + 2, 4);
    }
}
