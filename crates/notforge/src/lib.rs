//! Forge lifecycle and repository provisioning for notfiles.

pub mod config;
pub mod error;
pub mod git;
pub mod gitea;
pub mod lifecycle;
pub mod ports;

use config::{ForgeConfig, RepoSpec};
use error::NotforgeError;
use ports::{
    ForgeApi, ForgeAuth, ForgeLifecycle, GitRemoteManager, LifecycleStatus, LocalRepository,
    RemoteRepository, RemoteStatus, SecretResolverPort,
};

/// Current `notforge` crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Resolve `config`'s authentication material via `secrets`, preferring a
/// token over basic-auth username/password when both are configured.
pub fn resolve_auth(
    config: &config::ForgeAuthConfig,
    secrets: &dyn SecretResolverPort,
) -> Result<ForgeAuth, NotforgeError> {
    if let Some(token_ref) = &config.token {
        let token = secrets.resolve(token_ref)?;
        return Ok(ForgeAuth::Token { token });
    }
    if let (Some(username_ref), Some(password_ref)) = (&config.username, &config.password) {
        let username = secrets.resolve(username_ref)?;
        let password = secrets.resolve(password_ref)?;
        return Ok(ForgeAuth::Basic { username, password });
    }
    Err(NotforgeError::Secret(
        "no forge auth configured: set [gitea.auth.token] or both [gitea.auth.username] and [gitea.auth.password]".to_string(),
    ))
}

/// Ensure the forge backend described by `config` is available.
pub fn ensure_forge(
    config: &ForgeConfig,
    lifecycle: &dyn ForgeLifecycle,
) -> Result<LifecycleStatus, NotforgeError> {
    lifecycle.ensure_available(config)
}

/// Ensure `spec` exists on the forge, creating it if absent.
pub fn ensure_repository(
    api: &dyn ForgeApi,
    auth: &ForgeAuth,
    spec: &RepoSpec,
) -> Result<RemoteRepository, NotforgeError> {
    match api.repo(&spec.owner, &spec.name)? {
        Some(repo) => Ok(repo),
        None => api.create_repo(spec, auth),
    }
}

/// Derive the SSH clone URL for `repo` under `config`'s host and port.
pub fn ssh_remote_url(config: &config::GiteaConfig, repo: &RepoSpec) -> String {
    format!(
        "ssh://git@{}:{}/{}/{}.git",
        config.ssh_host, config.ssh_port, repo.owner, repo.name
    )
}

/// Ensure `local`'s git remote named `repo.remote_name` points at `remote_url`.
pub fn ensure_git_remote(
    manager: &dyn GitRemoteManager,
    local: &LocalRepository,
    repo: &RepoSpec,
    remote_url: &str,
) -> Result<RemoteStatus, NotforgeError> {
    manager.set_remote(local, &repo.remote_name, remote_url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ForgeAuthConfig, ForgeSecretRef, GiteaConfig, GiteaMode};
    use crate::ports::{ForgeVersion, PushStatus};
    use std::cell::Cell;

    #[test]
    fn exposes_package_version() {
        assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));
    }

    struct FakeSecrets;
    impl SecretResolverPort for FakeSecrets {
        fn resolve(&self, secret: &ForgeSecretRef) -> Result<String, NotforgeError> {
            match secret {
                ForgeSecretRef::Env { key } => Ok(format!("resolved-{key}")),
                ForgeSecretRef::Op { uri } => Ok(format!("resolved-{uri}")),
            }
        }
    }

    #[test]
    fn resolve_auth_prefers_token_over_basic() {
        let config = ForgeAuthConfig {
            token: Some(ForgeSecretRef::Env {
                key: "GITEA_TOKEN".to_string(),
            }),
            username: Some(ForgeSecretRef::Env {
                key: "GITEA_USER".to_string(),
            }),
            password: Some(ForgeSecretRef::Env {
                key: "GITEA_PASS".to_string(),
            }),
        };

        let auth = resolve_auth(&config, &FakeSecrets).unwrap();

        assert_eq!(
            auth,
            ForgeAuth::Token {
                token: "resolved-GITEA_TOKEN".to_string()
            }
        );
    }

    #[test]
    fn resolve_auth_falls_back_to_basic() {
        let config = ForgeAuthConfig {
            token: None,
            username: Some(ForgeSecretRef::Env {
                key: "GITEA_USER".to_string(),
            }),
            password: Some(ForgeSecretRef::Env {
                key: "GITEA_PASS".to_string(),
            }),
        };

        let auth = resolve_auth(&config, &FakeSecrets).unwrap();

        assert_eq!(
            auth,
            ForgeAuth::Basic {
                username: "resolved-GITEA_USER".to_string(),
                password: "resolved-GITEA_PASS".to_string(),
            }
        );
    }

    #[test]
    fn resolve_auth_errors_when_nothing_configured() {
        let config = ForgeAuthConfig::default();
        assert!(resolve_auth(&config, &FakeSecrets).is_err());
    }

    struct FakeForgeApi {
        existing: Option<RemoteRepository>,
        create_called: Cell<bool>,
    }

    impl ForgeApi for FakeForgeApi {
        fn version(&self) -> Result<ForgeVersion, NotforgeError> {
            Ok(ForgeVersion {
                version: "1.0.0".to_string(),
            })
        }

        fn repo(
            &self,
            _owner: &str,
            _name: &str,
        ) -> Result<Option<RemoteRepository>, NotforgeError> {
            Ok(self.existing.clone())
        }

        fn create_repo(
            &self,
            spec: &RepoSpec,
            _auth: &ForgeAuth,
        ) -> Result<RemoteRepository, NotforgeError> {
            self.create_called.set(true);
            Ok(RemoteRepository {
                owner: spec.owner.clone(),
                name: spec.name.clone(),
                clone_url: format!("http://gitea.local/{}/{}.git", spec.owner, spec.name),
                ssh_url: format!("ssh://git@gitea.local/{}/{}.git", spec.owner, spec.name),
            })
        }
    }

    fn repo_spec() -> RepoSpec {
        RepoSpec {
            owner: "joe".to_string(),
            name: "notfiles-config".to_string(),
            private: true,
            description: None,
            remote_name: "gitea".to_string(),
        }
    }

    #[test]
    fn ensure_repository_creates_when_absent() {
        let api = FakeForgeApi {
            existing: None,
            create_called: Cell::new(false),
        };
        let auth = ForgeAuth::Token {
            token: "t".to_string(),
        };

        let repo = ensure_repository(&api, &auth, &repo_spec()).unwrap();

        assert!(api.create_called.get());
        assert_eq!(repo.name, "notfiles-config");
    }

    #[test]
    fn ensure_repository_is_idempotent_when_present() {
        let existing = RemoteRepository {
            owner: "joe".to_string(),
            name: "notfiles-config".to_string(),
            clone_url: "http://gitea.local/joe/notfiles-config.git".to_string(),
            ssh_url: "ssh://git@gitea.local/joe/notfiles-config.git".to_string(),
        };
        let api = FakeForgeApi {
            existing: Some(existing.clone()),
            create_called: Cell::new(false),
        };
        let auth = ForgeAuth::Token {
            token: "t".to_string(),
        };

        let repo = ensure_repository(&api, &auth, &repo_spec()).unwrap();

        assert!(!api.create_called.get());
        assert_eq!(repo, existing);
    }

    #[test]
    fn ssh_remote_url_formats_host_port_owner_name() {
        let config = GiteaConfig {
            base_url: "http://gitea.local:3000".to_string(),
            owner: "joe".to_string(),
            ssh_host: "gitea.local".to_string(),
            ssh_port: 2222,
            mode: GiteaMode::Existing,
            auth: ForgeAuthConfig::default(),
        };

        assert_eq!(
            ssh_remote_url(&config, &repo_spec()),
            "ssh://git@gitea.local:2222/joe/notfiles-config.git"
        );
    }

    struct FakeGitRemoteManager {
        result: RemoteStatus,
        called_with: Cell<Option<()>>,
    }

    impl GitRemoteManager for FakeGitRemoteManager {
        fn remote_url(
            &self,
            _repo: &LocalRepository,
            _remote_name: &str,
        ) -> Result<Option<String>, NotforgeError> {
            Ok(None)
        }

        fn set_remote(
            &self,
            _repo: &LocalRepository,
            _remote_name: &str,
            _url: &str,
        ) -> Result<RemoteStatus, NotforgeError> {
            self.called_with.set(Some(()));
            Ok(self.result)
        }

        fn push(
            &self,
            _repo: &LocalRepository,
            _remote_name: &str,
            _branch: &str,
            _set_upstream: bool,
        ) -> Result<PushStatus, NotforgeError> {
            Ok(PushStatus::Pushed)
        }
    }

    #[test]
    fn ensure_git_remote_delegates_to_manager() {
        let manager = FakeGitRemoteManager {
            result: RemoteStatus::Added,
            called_with: Cell::new(None),
        };
        let local = LocalRepository {
            path: std::path::PathBuf::from("/tmp/repo"),
        };

        let status = ensure_git_remote(
            &manager,
            &local,
            &repo_spec(),
            "ssh://git@gitea.local/x.git",
        )
        .unwrap();

        assert_eq!(status, RemoteStatus::Added);
        assert!(manager.called_with.get().is_some());
    }
}
