use std::sync::Once;

use crate::Result;
use hyper::{header::UPGRADE, Body, Client};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::*;

fn init() -> Result<()> {
    tracing_subscriber::fmt::init();
    init_git_transport();

    Ok(())
}

const RECALL_GIT_SCHEME_HTTP: &str = "recall+git";
const RECALL_GIT_SCHEME_HTTPS: &str = "recalls+git";

fn init_git_transport() {
    static INIT: Once = Once::new();

    INIT.call_once(move || unsafe {
        git2::transport::register(RECALL_GIT_SCHEME_HTTP, |remote| {
            git2::transport::Transport::smart(remote, false, RecallGitTransport)
        })
        .unwrap();
    });
}

struct RecallGitTransport;

impl git2::transport::SmartSubtransport for RecallGitTransport {
    fn action(
        &self,
        url: &str,
        action: git2::transport::Service,
    ) -> Result<Box<dyn git2::transport::SmartSubtransportStream>, git2::Error> {
        use git2::{
            transport::SmartSubtransport, transport::Transport, Error, ErrorClass, ErrorCode,
        };

        trace!("creating transport for url {}", url);

        let mut url = url::Url::parse(url)
            .map_err(|e| Error::new(ErrorCode::Invalid, ErrorClass::Config, e.to_string()))?;

        url.set_scheme("http").map_err(|_| {
            Error::new(
                ErrorCode::Invalid,
                ErrorClass::Config,
                "failed to set url scheme",
            )
        })?;

        let handle = tokio::runtime::Handle::current();
        let conn = handle
            .block_on(git_conn(url))
            .map_err(|e| Error::new(ErrorCode::GenericError, ErrorClass::Http, e.to_string()))?;

        Ok(Box::new(conn))
    }

    fn close(&self) -> Result<(), git2::Error> {
        Ok(())
    }
}
struct RecallGitConn(hyper::upgrade::Upgraded);

async fn git_conn(url: url::Url) -> Result<RecallGitConn> {
    //  https://github.com/hyperium/hyper/blob/master/examples/upgrades.rs
    let upgrade_req = hyper::Request::builder()
        .uri(url.to_string())
        .header(UPGRADE, "recall-git")
        .body(Body::empty())?;

    let res = Client::new().request(upgrade_req).await?;

    let conn = hyper::upgrade::on(res).await?;

    Ok(RecallGitConn(conn))
}

async fn send_latest_commit() {}

fn send_diff() {}

impl std::io::Read for RecallGitConn {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let handle = tokio::runtime::Handle::current();
        let result = handle.block_on(async { self.0.read(buf).await });
        result
    }
}

impl std::io::Write for RecallGitConn {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let handle = tokio::runtime::Handle::current();
        handle.block_on(self.0.write(buf))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let handle = tokio::runtime::Handle::current();
        handle.block_on(self.0.flush())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[tokio::test]
    async fn test_git_upgrade() -> Result<()> {
        tracing_subscriber::fmt::init();
        trace!("initing git transport");
        init_git_transport();
        trace!("init git transport");

        let handle = tokio::runtime::Handle::current();
        handle
            .spawn_blocking(|| -> Result<_> {
                let repo = TempGitRepo::init()?;
                trace!("temp git repo ready");
                let mut remote = repo.remote_anonymous("recall+git://localhost:7890")?;
                // let mut remote = repo.remote("recall", "recall+git://localhost:7890")?;
                Ok(remote.push(&["head"], None)?)
            })
            .await??;
        trace!("here");

        Ok(())
    }

    struct TempGitRepo {
        path: std::path::PathBuf,
        repo: git2::Repository,
    }

    impl std::ops::Deref for TempGitRepo {
        type Target = git2::Repository;

        fn deref(&self) -> &Self::Target {
            &self.repo
        }
    }

    impl TempGitRepo {
        pub fn init() -> Result<Self> {
            let rand_path: [u8; 16] = rand::random();
            let rand_path =
                std::env::temp_dir().join(base64::encode_config(rand_path, base64::URL_SAFE));
            trace!("creating temp git repo {:?}", rand_path);
            Ok(Self {
                path: rand_path.clone(),
                repo: git2::Repository::init(rand_path)?,
            })
        }
    }

    impl Drop for TempGitRepo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}
