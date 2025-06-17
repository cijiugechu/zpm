use std::{clone, collections::BTreeMap, fmt::{self, Display, Formatter}, sync::LazyLock};

use zpm_utils::Path;
use bincode::{Decode, Encode};
use colored::Colorize;
use fancy_regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use zpm_utils::{impl_serialization_traits, FromFileString, ToFileString, ToHumanString};

use crate::{error::Error, github, prepare::PrepareParams, script::ScriptEnvironment};

static NEW_STYLE_GIT_SELECTOR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-z]+=").unwrap());

static GH_URL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:github:|(?:https):\/\/github\.com\/|git(?:\+ssh)?:\/\/(?:git@)?github\.com\/|git@github\.com:)?(?!\.{1,2}\/)([a-zA-Z0-9._-]+)\/(?!\.{1,2}(?:#|$))([a-zA-Z0-9._-]+?)(?:\.git)?(#.*)?$").unwrap());
static GH_TARBALL_URL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^https?:\/\/github\.com\/(?!\.{1,2}\/)([a-zA-Z0-9._-]+)\/(?!\.{1,2}(?:#|$))([a-zA-Z0-9._-]+?)\/tarball\/(.+)?$").unwrap());

static GH_URL_SET: LazyLock<Vec<Regex>> = LazyLock::new(|| vec![
    Regex::new(r"^ssh:").unwrap(),
    Regex::new(r"^git(?:\+[^:]+)?:").unwrap(),
  
    // `git+` is optional, `.git` is required
    Regex::new(r"^(?:git\+)?https?:[^#]+\/[^#]+(?:\.git)(?:#.*)?$").unwrap(),
  
    Regex::new(r"^git@[^#]+\/[^#]+\.git(?:#.*)?$").unwrap(),
    // Also match git@github.com:user/repo format (with colon)
    Regex::new(r"^git@github\.com:[^/]+/[^/]+(?:\.git)?(?:#.*)?$").unwrap(),
  
    Regex::new(r"^(?:github:|https:\/\/github\.com\/)?(?!\.{1,2}\/)([a-zA-Z._0-9-]+)\/(?!\.{1,2}(?:#|$))([a-zA-Z._0-9-]+?)(?:\.git)?(?:#.*)?$").unwrap(),
    // GitHub `/tarball/` URLs
    Regex::new(r"^https?:\/\/github\.com\/(?!\.{1,2}\/)([a-zA-Z0-9._-]+)\/(?!\.{1,2}(?:#|$))([a-zA-Z0-9._-]+?)\/tarball\/(.+)?$").unwrap(),
]);

pub fn is_git_url<P: AsRef<str>>(url: P) -> bool {
    GH_URL_SET.iter().any(|r| r.is_match(url.as_ref()).unwrap())
}

pub fn normalize_git_url<P: AsRef<str>>(url: P) -> String {
    let mut normalized = url.as_ref().to_string();

    if normalized.starts_with("git+https:") {
        normalized = normalized[4..].to_string();
    }

    normalized = GH_URL.replace(&normalized, "https://github.com/$1/$2.git$3").to_string();
    normalized = GH_TARBALL_URL.replace(&normalized, "https://github.com/$1/$2.git#$3").to_string();

    normalized
}

#[derive(Clone, Debug, Decode, Deserialize, Encode, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub enum GitSource {
    GitHub { owner: String, repository: String },
    Url(String),
}

impl GitSource {
    /// Parse a git URL into a GitSource
    pub fn from_url(url: &str) -> Self {
        // Normalize the URL first to handle various GitHub URL formats
        let normalized = normalize_git_url(url);
        
        // Check if it's a GitHub URL
        if let Ok(Some(captures)) = GH_URL.captures(&normalized) {
            if let (Some(owner), Some(repo)) = (captures.get(1), captures.get(2)) {
                return GitSource::GitHub {
                    owner: owner.as_str().to_string(),
                    repository: repo.as_str().to_string(),
                };
            }
        }
        
        // Check GitHub tarball URLs (on the original URL, not normalized)
        if let Ok(Some(captures)) = GH_TARBALL_URL.captures(url) {
            if let (Some(owner), Some(repo)) = (captures.get(1), captures.get(2)) {
                return GitSource::GitHub {
                    owner: owner.as_str().to_string(),
                    repository: repo.as_str().to_string(),
                };
            }
        }
        
        // Otherwise, treat it as a generic URL
        GitSource::Url(url.to_string())
    }
    
    /// Convert GitSource back to a URL string
    pub fn to_url(&self) -> String {
        match self {
            GitSource::GitHub { owner, repository } => {
                format!("https://github.com/{}/{}.git", owner, repository)
            }
            GitSource::Url(url) => url.clone(),
        }
    }
}

impl Display for GitSource {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.to_url())
    }
}

#[derive(Clone, Debug, Decode, Deserialize, Encode, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub enum GitTreeish {
    AnythingGoes(String),
    Head(String),
    Commit(String),
    Semver(zpm_semver::Range),
    Tag(String),
}

impl Display for GitTreeish {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            GitTreeish::AnythingGoes(treeish) => write!(f, "{}", treeish),
            GitTreeish::Head(head) => write!(f, "head={}", head),
            GitTreeish::Commit(commit) => write!(f, "commit={}", commit),
            GitTreeish::Semver(range) => write!(f, "semver={}", range),
            GitTreeish::Tag(tag) => write!(f, "tag={}", tag),
        }
    }
}

#[derive(Clone, Debug, Decode, Encode, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GitRange {
    pub repo: GitSource,
    pub treeish: GitTreeish,
    pub prepare_params: PrepareParams,
}

impl FromFileString for GitRange {
    type Error = Error;

    fn from_file_string(src: &str) -> Result<Self, Self::Error> {
        // TODO: I have the feeling we should do the other way around: first normalize, then validate.
        // Otherwise I'm concerned we'd forget to normalize some patterns.

        if !is_git_url(src) {
            return Err(Error::InvalidGitUrl(src.to_string()));
        }

        let normalized = normalize_git_url(src);
        extract_git_range(normalized)
    }
}

impl ToFileString for GitRange {
    fn to_file_string(&self) -> String {
        let mut params = vec![];

        params.push(match &self.treeish {
            GitTreeish::AnythingGoes(treeish) => treeish.to_string(),
            GitTreeish::Head(head) => format!("head={}", head),
            GitTreeish::Commit(commit) => format!("commit={}", commit),
            GitTreeish::Semver(range) => format!("semver={}", range),
            GitTreeish::Tag(tag) => format!("tag={}", tag),
        });

        if let Some(cwd) = &self.prepare_params.cwd {
            params.push(format!("cwd={}", urlencoding::encode(cwd)));
        }

        if let Some(workspace) = &self.prepare_params.workspace {
            params.push(format!("workspace={}", urlencoding::encode(workspace)));
        }

        format!("{}#{}", self.repo.to_url(), params.join("&"))
    }
}

impl ToHumanString for GitRange {
    fn to_print_string(&self) -> String {
        self.to_file_string().truecolor(135, 175, 255).to_string()
    }
}

impl_serialization_traits!(GitRange);

#[derive(Clone, Debug, Decode, Encode, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GitReference {
    pub repo: GitSource,
    pub commit: String,
    pub prepare_params: PrepareParams,
}

impl FromFileString for GitReference {
    type Error = Error;

    fn from_file_string(src: &str) -> Result<Self, Self::Error> {
        let mut parts = src.splitn(2, '#');

        let repo = parts.next().unwrap().to_string();
        let qs = parts.next().unwrap();

        let mut commit = None;
        let mut prepare_params = PrepareParams::default();

        for pair in qs.split('&') {
            if let Some(eq_index) = pair.find('=') {
                let (key, value) = pair.split_at(eq_index);
                let value = urlencoding::decode(&value[1..]).unwrap();

                match key {
                    "commit" =>
                        commit = Some(value.to_string()),

                    "cwd" =>
                        prepare_params.cwd = Some(value.to_string()),

                    "workspace" =>
                        prepare_params.workspace = Some(value.to_string()),

                    _ => {},
                };
            }
        }

        let commit = commit
            .expect("Expected a commit to always be present in a git reference");

        Ok(GitReference {
            repo: GitSource::from_url(&repo),
            commit,
            prepare_params,
        })
    }
}

impl ToFileString for GitReference {
    fn to_file_string(&self) -> String {
        let mut params = vec![
            format!("commit={}", urlencoding::encode(&self.commit)),
        ];

        if let Some(cwd) = &self.prepare_params.cwd {
            params.push(format!("cwd={}", urlencoding::encode(cwd)));
        }

        if let Some(workspace) = &self.prepare_params.workspace {
            params.push(format!("workspace={}", urlencoding::encode(workspace)));
        }

        format!("{}#{}", self.repo.to_url(), params.join("&"))
    }
}

impl ToHumanString for GitReference {
    fn to_print_string(&self) -> String {
        self.to_file_string().truecolor(135, 175, 255).to_string()
    }
}

impl_serialization_traits!(GitReference);

pub fn extract_git_range<P: AsRef<str>>(url: P) -> Result<GitRange, Error> {
    let url = url.as_ref();

    let hash_index = url.find('#');
    if hash_index.is_none() {
        return Ok(GitRange {
            repo: GitSource::from_url(url),
            treeish: GitTreeish::Head("HEAD".to_string()),
            prepare_params: PrepareParams::default(),
        });
    }

    let (repo, subsequent) = url.split_at(hash_index.unwrap());
    let subsequent = &subsequent[1..];

    // New-style: "#commit=abcdef&workspace=foobar"
    if NEW_STYLE_GIT_SELECTOR.is_match(subsequent).unwrap() {
        let mut treeish = GitTreeish::Head("HEAD".to_string());
        let mut prepare_params = PrepareParams::default();

        for pair in subsequent.split('&') {
            if let Some(eq_index) = pair.find('=') {
                let (key, value) = pair.split_at(eq_index);
                let value = urlencoding::decode(&value[1..]).unwrap();

                match key {
                    "head" =>
                        treeish = GitTreeish::Head(value.to_string()),

                    "commit" =>
                        treeish = GitTreeish::Commit(value.to_string()),

                    "semver" =>
                        treeish = GitTreeish::Semver(zpm_semver::Range::from_file_string(value.as_ref())?),

                    "tag" =>
                        treeish = GitTreeish::Tag(value.to_string()),

                    "cwd" =>
                        prepare_params.cwd = Some(value.to_string()),

                    "workspace" =>
                        prepare_params.workspace = Some(value.to_string()),

                    _ => {},
                }
            }
        }

        return Ok(GitRange {
            repo: GitSource::from_url(repo),
            treeish,
            prepare_params,
        });
    }

    // Old-style: "#commit:abcdef"
    let treeish = if let Some(colon_index) = subsequent.find(':') {
        let (kind, subsequent) = subsequent.split_at(colon_index);
        let subsequent = &subsequent[1..];

        match kind {
            "head" => GitTreeish::Head(subsequent.to_string()),
            "commit" => GitTreeish::Commit(subsequent.to_string()),
            "semver" => GitTreeish::Semver(zpm_semver::Range::from_file_string(subsequent)?),
            "tag" => GitTreeish::Tag(subsequent.to_string()),
            _ => GitTreeish::Commit(subsequent.to_string()),
        }
    } else {
        GitTreeish::AnythingGoes(subsequent.to_string())
    };

    Ok(GitRange {
        repo: GitSource::from_url(repo),
        treeish,
        prepare_params: PrepareParams::default(),
    })
}

async fn ls_remote(repo: &str) -> Result<BTreeMap<String, String>, Error> {
    let output = tokio::process::Command::new("git")
        .envs(make_git_env())
        .arg("ls-remote")
        .arg(repo)
        .output()
        .await?;

    if !output.status.success() {
        return Err(Error::GitError);
    }

    let output = String::from_utf8(output.stdout).unwrap();
    let mut refs = BTreeMap::new();

    for line in output.lines() {
        let mut parts = line.split_whitespace();
        let hash = parts.next().unwrap();
        let name = parts.next().unwrap();

        refs.insert(name.to_string(), hash.to_string());
    }

    Ok(refs)
}

fn tolerate_non_git_errors<T>(result: Result<T, Error>) -> Result<Result<T, Error>, Error> {
    if let Err(Error::GitError) = result {
        Err(Error::GitError)
    } else {
        Ok(result)
    }
}

pub async fn resolve_git_treeish(git_range: &GitRange) -> Result<String, Error> {
    match &git_range.treeish {
        GitTreeish::AnythingGoes(treeish) => {
            if let Ok(result) = tolerate_non_git_errors(resolve_git_treeish_stricter(&git_range.repo.to_url(), GitTreeish::Commit(treeish.clone())).await)? {
                Ok(result)
            } else if let Ok(result) = tolerate_non_git_errors(resolve_git_treeish_stricter(&git_range.repo.to_url(), GitTreeish::Tag(treeish.clone())).await)? {
                Ok(result)
            } else if let Ok(result) = tolerate_non_git_errors(resolve_git_treeish_stricter(&git_range.repo.to_url(), GitTreeish::Head(treeish.clone())).await)? {
                Ok(result)
            } else {
                Err(Error::InvalidGitSpecifier)
            }
        }

        _ => resolve_git_treeish_stricter(&git_range.repo.to_url(), git_range.treeish.clone()).await
    }
}

async fn resolve_git_treeish_stricter(repo: &str, treeish: GitTreeish) -> Result<String, Error> {
    let refs = ls_remote(repo).await?;

    match treeish {
        GitTreeish::AnythingGoes(_) => {
            unreachable!();
        },

        GitTreeish::Head(head) => {
            let ref_name = if head == "HEAD" {
                "HEAD".to_string()
            } else {
                format!("refs/heads/{}", head)
            };

            let head = refs.get(&ref_name)
                .ok_or(Error::InvalidGitBranch(head))?;

            Ok(head.to_string())
        }

        GitTreeish::Commit(commit) => {
            if commit.len() == 40 && commit.chars().all(|c| c.is_ascii_hexdigit()) {
                Ok(commit)
            } else {
                Err(Error::InvalidGitCommit(commit))
            }
        }

        GitTreeish::Semver(tag) => {
            let mut candidates: Vec<(String, zpm_semver::Version)> = refs.into_iter()
                .filter(|(k, _)| k.starts_with("refs/tags/") && !k.ends_with("^{}"))
                .filter_map(|(k, _)| zpm_semver::Version::from_file_string(&k[10..]).ok().map(|v| (k, v)))
                .filter(|(_, v)| tag.check(v))
                .collect();

            candidates.sort_by(|(_, v1), (_, v2)| {
                v2.cmp(v1)
            });

            if let Some((k, _)) = candidates.first() {
                Ok(k.to_string())
            } else {
                Err(Error::NoCandidatesFound(tag.to_string()))
            }
        }

        GitTreeish::Tag(tag) => {
            let ref_name = format!("refs/tags/{}", tag);

            let head = refs.get(&ref_name)
                .ok_or(Error::InvalidGitBranch(tag))?;

            Ok(head.to_string())
        }
    }
}

fn make_git_env() -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();

    if let Err(std::env::VarError::NotPresent) = std::env::var("GIT_SSH_COMMAND") {
        let ssh = std::env::var("GIT_SSH").unwrap_or("ssh".to_string());
        let ssh_command = format!("{} -o BatchMode=yes", ssh);

        env.insert("GIT_SSH_COMMAND".to_string(), ssh_command);
    }

    env
}

pub async fn clone_repository(source: &GitSource, commit: &str) -> Result<Path, Error> {
    let clone_dir
        = Path::temp_dir()?;

    if download_into(&source, commit, &clone_dir).await?.is_some() {
        return Ok(clone_dir);
    }

    git_clone_into(source, commit, &clone_dir).await?;
    Ok(clone_dir)
}

async fn download_into(source: &GitSource, commit: &str, download_dir: &Path) -> Result<Option<()>, Error> {
    if github::download_into(source, commit, download_dir).await?.is_some() {
        return Ok(Some(()));
    }

    Ok(None)
}

async fn git_clone_into(source: &GitSource, commit: &str, clone_dir: &Path) -> Result<(), Error> {
    let clone_url = match source {
        GitSource::GitHub {owner, repository} => {
            let is_auth_ok = false;

            if is_auth_ok {
                format!("git@github.com:{owner}/{repository}.git")
            } else {
                format!("https://github.com/{owner}/{repository}.git")
            }
        },

        GitSource::Url(url) => {
            url.clone()
        },
    };

    ScriptEnvironment::new()?
        .with_env(make_git_env())
        .run_exec("git", &["clone", "-c", "core.autocrlf=false", &clone_url, clone_dir.as_str()]).await
        .ok()?;

    ScriptEnvironment::new()?
        .with_cwd(clone_dir.clone())
        .with_env(make_git_env())
        .run_exec("git", &["checkout", commit]).await
        .ok()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_source_from_github_urls() {
        // Test various GitHub URL formats
        let test_cases = vec![
            ("https://github.com/rust-lang/cargo.git", "rust-lang", "cargo"),
            ("git@github.com:rust-lang/cargo.git", "rust-lang", "cargo"),
            ("github:rust-lang/cargo", "rust-lang", "cargo"),
            ("https://github.com/rust-lang/cargo", "rust-lang", "cargo"),
        ];

        for (url, expected_owner, expected_repo) in test_cases {
            match GitSource::from_url(url) {
                GitSource::GitHub { owner, repository } => {
                    assert_eq!(owner, expected_owner, "Owner mismatch for URL: {}", url);
                    assert_eq!(repository, expected_repo, "Repository mismatch for URL: {}", url);
                }
                _ => panic!("Expected GitHub variant for URL: {}", url),
            }
        }
    }

    #[test]
    fn test_git_source_from_non_github_urls() {
        // Test non-GitHub URLs
        let test_cases = vec![
            "https://gitlab.com/user/repo.git",
            "git@bitbucket.org:user/repo.git",
            "https://example.com/git/repo.git",
        ];

        for url in test_cases {
            match GitSource::from_url(url) {
                GitSource::Url(parsed_url) => {
                    assert_eq!(parsed_url, url, "URL mismatch");
                }
                _ => panic!("Expected Url variant for URL: {}", url),
            }
        }
    }

    #[test]
    fn test_git_source_to_url() {
        // Test GitHub variant
        let github_source = GitSource::GitHub {
            owner: "rust-lang".to_string(),
            repository: "cargo".to_string(),
        };
        assert_eq!(github_source.to_url(), "https://github.com/rust-lang/cargo.git");

        // Test Url variant
        let url_source = GitSource::Url("https://example.com/repo.git".to_string());
        assert_eq!(url_source.to_url(), "https://example.com/repo.git");
    }
}
