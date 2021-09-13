use anyhow::{anyhow, Context, Result};
use crypto::digest::Digest;
use crypto::sha3::Sha3;
use std::fs;
use std::path::PathBuf;

/// Computes a sha-3 hash of the files in sorted order
/// hash = sha3(file_path_relative_to_root + file_contents)
pub async fn repo_hash(root: &PathBuf, paths: Vec<PathBuf>) -> Result<String> {
    let mut sorted = paths.clone();
    sorted.sort_by(|a, b| b.cmp(a));

    // Start hash
    let mut hasher = Sha3::sha3_256();

    for path in sorted {
        /*
        Potentially could be a bug here: Executables on windows vs unix.
        - On Windows, every file is executable
        - On Unix, only files where metadata.mode() & 0o111 != 0

        If I make a file executable in git on unix, it will store that, so when I git pull, any file someone else marked as exec is now exec.

        This might lead to problems where, someone creates a file on a windows computer, that's supposed to be executable, but the unix build farm server doesn't know it's supposed to be exec
        */

        let cloned_path = path.clone();
        let result = cloned_path
            .strip_prefix(root)
            .context(format!("{:?} is not a child of {:?}", path, root))?;
        let as_str = result
            .to_str()
            .ok_or(anyhow!("Failed to cconvert {:?} to string", result))?;

        hasher.input_str(format!("{}{}", as_str, fs::read_to_string(path)?.as_str()).as_str());
    }

    Ok("".to_string())
}

#[cfg(test)]
mod tests {
    use anyhow::Context;
    use itertools::Itertools;
    use std::fs::File;

    use std::io::Write;
    use std::path::Path;
    use std::vec;
    use tempdir::TempDir;

    use crate::hash::repo_hash;

    #[tokio::test]
    async fn test_with_different_permutations() {
        let tmp = TempDir::new(".hash")
            .context("Can't create a tmp dir")
            .unwrap();

        let a = tmp.path().join("alpha.flargle");
        let mut afile = File::create(a.clone()).unwrap();
        afile.write_all(b"Some *() text here; () => {}").unwrap();

        let b = tmp.path().join("231asb21.json");
        let mut bfile = File::create(b.clone()).unwrap();
        bfile.write_all(b"ablawefome text here; () => {}").unwrap();

        let c = tmp.path().join("zed.bargle");
        let mut cfile = File::create(c.clone()).unwrap();
        cfile.write_all(b"ac ahhhhh here; () => {}").unwrap();

        let d = tmp.path().join("derga.azzz");
        let mut afile = File::create(d.clone()).unwrap();
        afile.write_all(b"aaaa text here; () => {}").unwrap();

        let e = tmp.path().join("derga");
        let mut efile = File::create(e.clone()).unwrap();
        efile.write_all(b"Some text here; () => {}").unwrap();

        let f = tmp.path().join("smarshmellow");
        let mut ffile = File::create(f.clone()).unwrap();
        ffile.write_all(b"Some text here; () => {}").unwrap();

        let files = vec![a, b, c, d, e, f];

        let paths = files
            .iter()
            .permutations(files.len())
            .unique()
            .collect::<Vec<_>>();

        let mut hashes: Vec<String> = vec![];
        for p in paths {
            let hash = repo_hash(
                &Path::new(tmp.as_ref()).to_path_buf(),
                p.clone().iter().cloned().map(|e| e.clone()).collect_vec(),
            )
            .await
            .unwrap();
            hashes.push(hash);
        }

        assert!(hashes.iter().all_equal());
    }
}
