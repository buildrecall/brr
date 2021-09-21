use anyhow::{anyhow, Context, Result};
use std::{env, path::PathBuf};

use crate::{
    api::{ApiClient, BuildRecall, Project},
    git,
    global_config::{overwrite_global_config, read_global_config, GlobalConfig, RepoConfig},
};

pub async fn preattach_to_repo(global_config_dir: PathBuf, slug: String) -> Result<()> {
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

    let projects = client.list_projects().await?;

    let existing = global_config.repo_config_of_pathbuf(path.clone())?;
    if existing.is_some() {
        return Ok(()); // skip if we already have something in our config
    }

    let maybe_project = projects.iter().find(|p| p.slug == slug.clone());
    let project: Result<Project> = match maybe_project {
        Some(p) => Ok(p.clone()),
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

pub async fn run_pull(global_config_dir: PathBuf, slug: String) -> Result<()> {
    let config = read_global_config(global_config_dir.clone())?;
    let g = git::RecallGit::new(global_config_dir.clone())?;

    // Send hash to build farm to request build

    Ok(())
}
