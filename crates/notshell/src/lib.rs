pub mod emit;
pub mod error;
pub mod ir;
pub mod ports;

pub use error::NotshellError;
pub use ir::ShellConfig;
pub use ports::ShellEmitter;

use emit::fish::FishEmitter;
use emit::nu::NuEmitter;

/// Look up the emitter for a shell id (`"nu"`, `"fish"`; `"zsh"`/`"bash"` not yet implemented).
pub fn emitter_for(shell: &str) -> Result<Box<dyn ShellEmitter>, NotshellError> {
    match shell {
        "nu" => Ok(Box::new(NuEmitter)),
        "fish" => Ok(Box::new(FishEmitter)),
        other => Err(NotshellError::UnknownShell(other.to_string())),
    }
}

/// Render `config` for every requested shell, returning `(shell_id, file_ext, contents)`.
pub fn generate(
    config: &ShellConfig,
    shells: &[String],
) -> Result<Vec<(String, String, String)>, NotshellError> {
    shells
        .iter()
        .map(|id| {
            let emitter = emitter_for(id)?;
            Ok((
                emitter.id().to_string(),
                emitter.ext().to_string(),
                emitter.emit(config),
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ir::{Alias, EnvVar, Guard, PathEntry};

    fn sample_config() -> ShellConfig {
        ShellConfig {
            env: vec![EnvVar {
                name: "RUSTC_WRAPPER".to_string(),
                value: "sccache".to_string(),
                guard: Some(Guard::CommandExists("sccache".to_string())),
            }],
            path: vec![PathEntry {
                dir: "$HOME/.local/bin".to_string(),
                prepend: false,
                guard: Some(Guard::PathExists("$HOME/.local/bin".to_string())),
            }],
            aliases: vec![
                Alias {
                    name: "x".to_string(),
                    command: "cargo xtask".to_string(),
                    guard: None,
                    else_command: None,
                },
                Alias {
                    name: "ide".to_string(),
                    command: "zed .".to_string(),
                    guard: Some(Guard::CommandExists("zed".to_string())),
                    else_command: Some("nvim .".to_string()),
                },
            ],
        }
    }

    #[test]
    fn nu_emits_unconditional_and_conditional_aliases() {
        let out = emitter_for("nu").unwrap().emit(&sample_config());
        assert!(out.contains("alias x = cargo xtask"));
        assert!(out.contains("alias ide = zed ."));
        assert!(out.contains("alias ide = nvim ."));
        assert!(out.contains("if (which zed | is-not-empty)"));
    }

    #[test]
    fn fish_emits_unconditional_and_conditional_aliases() {
        let out = emitter_for("fish").unwrap().emit(&sample_config());
        assert!(out.contains("alias x \"cargo xtask\""));
        assert!(out.contains("if command -q zed"));
        assert!(out.contains("alias ide \"zed .\""));
        assert!(out.contains("alias ide \"nvim .\""));
    }

    #[test]
    fn unknown_shell_errors() {
        match emitter_for("bash") {
            Err(NotshellError::UnknownShell(s)) => assert_eq!(s, "bash"),
            Err(other) => panic!("expected UnknownShell, got {other:?}"),
            Ok(_) => panic!("expected UnknownShell error, got Ok"),
        }
    }

    #[test]
    fn generate_covers_requested_shells_only() {
        let out = generate(&sample_config(), &["nu".to_string()]).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, "nu");
        assert_eq!(out[0].1, "nu");
    }

    #[test]
    fn ir_parses_from_toml() {
        let toml = r#"
            [[env]]
            name = "SOPS_AGE_KEY_FILE"
            value = "$HOME/.config/sops/age/keys.txt"

            [[aliases]]
            name = "m"
            command = "mise"
        "#;
        let cfg = ShellConfig::from_toml_str(toml).unwrap();
        assert_eq!(cfg.env.len(), 1);
        assert_eq!(cfg.aliases.len(), 1);
        assert_eq!(cfg.aliases[0].name, "m");
    }
}
