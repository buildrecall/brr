use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

const BUILD_RECALL_HOST: &str = "https://buildrecall.com";

use crate::global_config::{self, GlobalConfig};

#[derive(Serialize, Deserialize)]
pub struct LoginRequestBody {
    pub single_use_token: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LoginRequestResponseBody {
    // The "real" access token that's saved on the user's
    // computer and used to auth
    pub access_token: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Project {
    pub id: uuid::Uuid,

    // Aka the "name", what's used in the command line arg.
    // This can be changed by a user
    pub slug: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CreateProjectBody {
    pub slug: String,
}

#[async_trait]
pub trait BuildRecall {
    async fn login(&self, body: LoginRequestBody) -> Result<LoginRequestResponseBody>;
    async fn list_projects(&self) -> Result<Vec<Project>>;
    async fn create_project(&self, slug: String) -> Result<Project>;
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ApiClient {
    global_config: GlobalConfig,
}

impl ApiClient {
    pub fn new(global_config: GlobalConfig) -> ApiClient {
        ApiClient { global_config }
    }

    pub fn get_host(&self) -> String {
        self.global_config
            .clone()
            .host()
            .unwrap_or(BUILD_RECALL_HOST.into())
    }
}

#[async_trait]
impl BuildRecall for ApiClient {
    async fn login(&self, body: LoginRequestBody) -> Result<LoginRequestResponseBody> {
        let client = reqwest::Client::new();

        let resp = client
            .post(format!("{}/v1/cli/login", self.get_host()))
            .json(&body)
            .send()
            .await
            .context("Failed to connect to Build Recall. Perhaps your internet is down, or Build Recall is having an outage.")?;

        if resp.status() == 401 {
            return Err(anyhow!(
                    "The token '{}' is expired, invalid, or has already been used to login.\nYou can get a new one at https://buildrecall.com/setup",
                    body.single_use_token
                ));
        }
        if !resp.status().is_success() {
            return Err(anyhow!("Failed to login to Build Recall. Got a status code '{}' for 'POST {}/v1/cli/login'. Build Recall may be having an outage, or you may need to try running this command again.", resp.status(), self.get_host()));
        }
        let result = resp.json::<LoginRequestResponseBody>()
                .await
                .context("Failed to login to Build Recall. The response unexpectedly did not return a JSON body. This is almost certainly a bug in Build Recall. :(")?;

        Ok(result)
    }

    async fn list_projects(&self) -> Result<Vec<Project>> {
        let client = reqwest::Client::new();
        let tok = self.global_config.clone().access_token().ok_or(
                anyhow!("Can't find an 'access_token' in your global config file (which typically lives at ~/.buildrecall/config).")
            )?;

        let resp = client
            .get(format!("{}/v1/cli/projects",  self.get_host()))
            .bearer_auth(tok)
            .send()
            .await
            .context("Failed to connect to Build Recall. Perhaps your internet is down, or Build Recall is having an outage.")?;

        if resp.status() == 401 {
            return Err(anyhow!(
                    "Something is wrong with your access token, perhaps you've been logged out by the server? You can login again at https://buildrecall.com/setup",
                    ));
        }
        if !resp.status().is_success() {
            return Err(anyhow!("Failed to list your projects. Got a status code '{}' for 'GET {}/v1/cli/projects'. Build Recall may be having an outage, or you may need to try running this command again.", resp.status(), self.get_host()));
        }

        let projects = resp.json::<Vec<Project>>()
        .await
        .context(format!("Failed to list projects. The response to {} unexpectedly did not return a JSON body. This is almost certainly a bug in Build Recall. :(", "/v1/cli/projects"))?;

        Ok(projects)
    }

    async fn create_project(&self, slug: String) -> Result<Project> {
        let client = reqwest::Client::new();
        let tok = self.global_config.clone().access_token().ok_or(
                anyhow!("Can't find an 'access_token' in your global config file (which typically lives at ~/.buildrecall/config).")
        )?;

        let resp = client
            .post(format!("{}/v1/cli/projects",  self.get_host()))
            .json(&CreateProjectBody{
                slug: slug
            })
            .bearer_auth(tok)
            .send()
            .await
            .context("Failed to connect to Build Recall. Perhaps your internet is down, or Build Recall is having an outage.")?;

        if resp.status() == 401 {
            return Err(anyhow!(
                "Something is wrong with your access token, perhaps you've been logged out by the server? You can login again at https://buildrecall.com/setup"
            ));
        }
        if !resp.status().is_success() {
            return Err(anyhow!("Failed to create this project. Got a status code '{}' for 'POST {}/v1/cli/projects'. Build Recall may be having an outage, or you may need to try running this command again.", resp.status(), self.get_host()));
        }

        let result = resp.json::<Project>()
                .await
                .context("Failed to create this project. The response unexpectedly did not return a JSON body. This is almost certainly a bug in Build Recall. :(")?;

        Ok(result)
    }
}
