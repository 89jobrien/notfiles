use std::path::PathBuf;

use crate::config::{ForgeConfig, ForgeSecretRef, RepoSpec};
use crate::error::NotforgeError;

/// Authentication material resolved for forge API requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForgeAuth {
    /// Token-based Gitea API authentication.
    Token { token: String },
    /// Basic authentication fallback for bootstrap flows.
    Basic { username: String, password: String },
}

/// Remote repository metadata returned by a forge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteRepository {
    /// Repository owner namespace.
    pub owner: String,
    /// Repository name.
    pub name: String,
    /// HTTP clone URL.
    pub clone_url: String,
    /// SSH clone URL.
    pub ssh_url: String,
}

/// Forge version metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeVersion {
    /// Server version string.
    pub version: String,
}

/// Local git repository path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalRepository {
    /// Filesystem path to the local repository.
    pub path: PathBuf,
}

/// Result of ensuring a forge backend is available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleStatus {
    /// The forge was already reachable.
    AlreadyAvailable,
    /// The forge was started by this operation.
    Started,
    /// The forge was verified without needing a start operation.
    Verified,
}

/// Result of setting a local git remote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteStatus {
    /// The remote was added.
    Added,
    /// The remote existed and was updated.
    Updated,
    /// The remote already matched the desired URL.
    Unchanged,
}

/// Result of pushing a local branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushStatus {
    /// The branch was pushed.
    Pushed,
    /// The branch already matched the remote.
    AlreadyUpToDate,
}

/// Port for forge API operations.
pub trait ForgeApi {
    /// Return server version metadata.
    fn version(&self) -> Result<ForgeVersion, NotforgeError>;

    /// Look up a repository by owner and name.
    fn repo(&self, owner: &str, name: &str) -> Result<Option<RemoteRepository>, NotforgeError>;

    /// Create a repository from a spec and resolved auth.
    fn create_repo(
        &self,
        spec: &RepoSpec,
        auth: &ForgeAuth,
    ) -> Result<RemoteRepository, NotforgeError>;
}

/// Port for local or remote forge lifecycle operations.
pub trait ForgeLifecycle {
    /// Ensure the configured forge backend is available.
    fn ensure_available(&self, config: &ForgeConfig) -> Result<LifecycleStatus, NotforgeError>;
}

/// Port for git remote and push operations.
pub trait GitRemoteManager {
    /// Return the URL for a local remote if it exists.
    fn remote_url(
        &self,
        repo: &LocalRepository,
        remote_name: &str,
    ) -> Result<Option<String>, NotforgeError>;

    /// Add or update a local git remote.
    fn set_remote(
        &self,
        repo: &LocalRepository,
        remote_name: &str,
        url: &str,
    ) -> Result<RemoteStatus, NotforgeError>;

    /// Push a branch to a local remote.
    fn push(
        &self,
        repo: &LocalRepository,
        remote_name: &str,
        branch: &str,
        set_upstream: bool,
    ) -> Result<PushStatus, NotforgeError>;
}

/// Port for resolving forge secrets.
pub trait SecretResolverPort {
    /// Resolve a secret reference into an in-memory secret value.
    fn resolve(&self, secret: &ForgeSecretRef) -> Result<String, NotforgeError>;
}
