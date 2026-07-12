use std::path::PathBuf;

use notforge::config::{
    ForgeAuthConfig, ForgeConfig, ForgeSecretRef, GiteaMode, RepoSpec, VmSystemdConfig, load_config,
};
use notforge::error::NotforgeError;
use notforge::ports::{
    ForgeApi, ForgeAuth, ForgeLifecycle, ForgeVersion, GitRemoteManager, LifecycleStatus,
    LocalRepository, PushStatus, RemoteRepository, RemoteStatus, SecretResolverPort,
};

fn assert_diagnostic<E: miette::Diagnostic>() {}

#[test]
fn notforge_error_implements_miette_diagnostic() {
    assert_diagnostic::<NotforgeError>();
}

#[test]
fn config_parses_vm_systemd_gitea_mode() {
    let dir = tempfile::TempDir::new().unwrap();
    let config_path = dir.path().join("notforge.toml");
    std::fs::write(
        &config_path,
        r#"
            [gitea]
            base_url = "http://gitea.local:3000"
            owner = "joe"
            ssh_host = "gitea.local"
            ssh_port = 2222

            [gitea.mode]
            type = "vm-systemd"

            [gitea.mode.config]
            ssh_user = "dev"
            ssh_host = "gitea.local"
            service_name = "gitea"

            [gitea.auth.token]
            source = "env"
            key = "GITEA_TOKEN"

            [[repositories]]
            owner = "joe"
            name = "notfiles-config"
            private = true
            description = "Personal config payload"
            remote_name = "gitea"
        "#,
    )
    .unwrap();

    let config = load_config(&config_path).unwrap();

    assert_eq!(config.gitea.base_url, "http://gitea.local:3000");
    assert_eq!(config.gitea.owner, "joe");
    assert_eq!(config.gitea.ssh_port, 2222);
    assert_eq!(
        config.gitea.auth.token,
        Some(ForgeSecretRef::Env {
            key: "GITEA_TOKEN".to_string()
        })
    );
    assert_eq!(
        config.gitea.mode,
        GiteaMode::VmSystemd(VmSystemdConfig {
            ssh_user: "dev".to_string(),
            ssh_host: "gitea.local".to_string(),
            service_name: "gitea".to_string(),
        })
    );
    assert_eq!(
        config.repositories,
        vec![RepoSpec {
            owner: "joe".to_string(),
            name: "notfiles-config".to_string(),
            private: true,
            description: Some("Personal config payload".to_string()),
            remote_name: "gitea".to_string(),
        }]
    );
}

#[test]
fn config_defaults_repositories_to_empty() {
    let config: ForgeConfig = toml::from_str(
        r#"
            [gitea]
            base_url = "http://localhost:3000"
            owner = "joe"
            ssh_host = "localhost"
            ssh_port = 2222
            mode = "existing"
        "#,
    )
    .unwrap();

    assert!(config.repositories.is_empty());
    assert_eq!(config.gitea.auth, ForgeAuthConfig::default());
}

#[test]
fn ports_are_object_safe_for_adapters() {
    struct FakeApi;
    impl ForgeApi for FakeApi {
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
            Ok(None)
        }

        fn create_repo(
            &self,
            spec: &RepoSpec,
            _auth: &ForgeAuth,
        ) -> Result<RemoteRepository, NotforgeError> {
            Ok(RemoteRepository {
                owner: spec.owner.clone(),
                name: spec.name.clone(),
                clone_url: "http://example/repo.git".to_string(),
                ssh_url: "ssh://git@example/joe/repo.git".to_string(),
            })
        }
    }

    struct FakeLifecycle;
    impl ForgeLifecycle for FakeLifecycle {
        fn ensure_available(
            &self,
            _config: &ForgeConfig,
        ) -> Result<LifecycleStatus, NotforgeError> {
            Ok(LifecycleStatus::AlreadyAvailable)
        }
    }

    struct FakeGit;
    impl GitRemoteManager for FakeGit {
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
            Ok(RemoteStatus::Added)
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

    struct FakeSecrets;
    impl SecretResolverPort for FakeSecrets {
        fn resolve(&self, _secret: &ForgeSecretRef) -> Result<String, NotforgeError> {
            Ok("secret".to_string())
        }
    }

    let api: &dyn ForgeApi = &FakeApi;
    let lifecycle: &dyn ForgeLifecycle = &FakeLifecycle;
    let git: &dyn GitRemoteManager = &FakeGit;
    let secrets: &dyn SecretResolverPort = &FakeSecrets;

    assert_eq!(api.version().unwrap().version, "1.0.0");
    assert_eq!(
        lifecycle
            .ensure_available(&ForgeConfig {
                gitea: notforge::config::GiteaConfig {
                    base_url: "http://localhost:3000".to_string(),
                    owner: "joe".to_string(),
                    ssh_host: "localhost".to_string(),
                    ssh_port: 2222,
                    mode: GiteaMode::Existing,
                    auth: ForgeAuthConfig::default(),
                },
                repositories: Vec::new(),
            })
            .unwrap(),
        LifecycleStatus::AlreadyAvailable
    );
    assert_eq!(
        git.remote_url(
            &LocalRepository {
                path: PathBuf::from(".")
            },
            "gitea"
        )
        .unwrap(),
        None
    );
    assert_eq!(
        secrets
            .resolve(&ForgeSecretRef::Env {
                key: "GITEA_TOKEN".to_string()
            })
            .unwrap(),
        "secret"
    );
}
