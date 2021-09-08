use std::{env, path::PathBuf};

use anyhow::Result;

use crate::{
    api::{ApiClient, BuildRecall},
    global_config::{overwrite_global_config, read_global_config, GlobalConfig, RepoConfig},
};

pub struct AttachArguments {
    pub slug: String,
}

pub async fn run_attach(global_config_dir: PathBuf, args: AttachArguments) -> Result<()> {
    let path = env::current_dir()?;
    let pieces = path
        .components()
        .map(|comp| comp.as_os_str().to_str().unwrap_or("").to_string())
        .collect::<Vec<_>>();
    let folder = pieces[pieces.len() - 1].clone();

    let global_config = read_global_config(global_config_dir.clone())?;
    let client = ApiClient::new(global_config);

    // check if global config already has this

    client.create_project(args.slug.clone()).await?;

    overwrite_global_config(global_config_dir, move |c| {
        let mut repos: Vec<RepoConfig> = vec![RepoConfig {
            path: path,
            name: args.slug.clone(),
            id: uuid::Uuid::new_v4(),
        }];
        repos.extend(c.repos.unwrap_or(vec![]));

        GlobalConfig {
            access_token: c.access_token,
            repos: Some(repos),
            host: c.host,
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
