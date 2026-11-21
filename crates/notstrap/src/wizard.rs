//! The bootstrap wizard: confirm or adjust what `notstrap run` will do before it does it.

use anyhow::Result;
use notcore::HookSpec;

use crate::NotstrapConfig;
use crate::prompt::Prompter;

/// What the user settled on. Applied over the loaded config before the run starts.
#[derive(Debug)]
pub struct Choices {
    /// Clone URL; `None` when the Tailscale gitea URL is used instead.
    pub dotfiles_repo: Option<String>,
    pub dotfiles_dir: String,
    pub use_tailscale: bool,
    pub hooks: Vec<HookSpec>,
}

fn hook_label(h: &HookSpec) -> String {
    format!("{} ({:?} phase, {})", h.name, h.phase, h.script)
}

/// Ask the questions. Returns `None` when the user declines the final confirmation.
pub fn run(cfg: &NotstrapConfig, p: &dyn Prompter) -> Result<Option<Choices>> {
    println!("notstrap wizard: answer the questions or press enter for the default.\n");

    let use_tailscale = match &cfg.tailscale {
        Some(_) => p.confirm("Join the tailnet before cloning?", true)?,
        None => false,
    };

    let dotfiles_repo = if cfg.tailscale.is_some() && use_tailscale {
        None
    } else {
        let default = cfg.bootstrap.dotfiles_repo.clone().unwrap_or_default();
        let url = p.input("Dotfiles repository URL", &default)?;
        anyhow::ensure!(!url.is_empty(), "a dotfiles repository URL is required");
        Some(url)
    };

    let dotfiles_dir = p.input("Dotfiles directory", &cfg.bootstrap.dotfiles_dir)?;

    let hooks = if cfg.hooks.is_empty() {
        Vec::new()
    } else {
        let labels: Vec<String> = cfg.hooks.iter().map(hook_label).collect();
        let chosen = p.multi_select("Hooks to run:", &labels)?;
        chosen.into_iter().map(|i| cfg.hooks[i].clone()).collect()
    };

    println!("\nPlan:");
    match &dotfiles_repo {
        Some(url) => println!("  clone    {url}"),
        None => println!("  clone    via the tailnet gitea URL"),
    }
    println!("  into     {dotfiles_dir}");
    println!("  tailnet  {}", if use_tailscale { "join" } else { "skip" });
    println!("  hooks    {}", hooks.len());
    for h in &hooks {
        println!("    - {}", hook_label(h));
    }
    if !p.confirm("\nProceed?", true)? {
        return Ok(None);
    }
    Ok(Some(Choices {
        dotfiles_repo,
        dotfiles_dir,
        use_tailscale,
        hooks,
    }))
}

/// Overlay the wizard's choices onto the loaded config.
pub fn apply(cfg: &mut NotstrapConfig, choices: Choices) {
    if let Some(url) = choices.dotfiles_repo {
        cfg.bootstrap.dotfiles_repo = Some(url);
    }
    cfg.bootstrap.dotfiles_dir = choices.dotfiles_dir;
    if !choices.use_tailscale {
        cfg.tailscale = None;
    }
    cfg.hooks = choices.hooks;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt::{AssumeYes, LinePrompter};
    use std::io::Cursor;

    fn cfg(extra: &str) -> NotstrapConfig {
        let text = format!(
            "[bootstrap]\ndotfiles_repo = \"https://example.com/d.git\"\ndotfiles_dir = \"~/.notfiles\"\n{extra}"
        );
        toml::from_str(&text).expect("from_str should succeed")
    }

    const HOOKS: &str = "[[hooks]]\nname = \"a\"\nscript = \"a.rs\"\nphase = \"setup\"\n[[hooks]]\nname = \"b\"\nscript = \"b.rs\"\nphase = \"setup\"\n";

    fn scripted(input: &str) -> LinePrompter<Cursor<Vec<u8>>, Vec<u8>> {
        LinePrompter::new(Cursor::new(input.as_bytes().to_vec()), Vec::new())
    }

    #[test]
    fn assume_yes_keeps_the_config_as_is() {
        let c = cfg(HOOKS);
        let got = run(&c, &AssumeYes)
            .expect("run should succeed")
            .expect("the value should be present");
        assert_eq!(
            got.dotfiles_repo.as_deref(),
            Some("https://example.com/d.git")
        );
        assert_eq!(got.dotfiles_dir, "~/.notfiles");
        assert_eq!(got.hooks.len(), 2);
        assert!(!got.use_tailscale);
    }

    #[test]
    fn answers_override_repo_dir_and_hooks() {
        let c = cfg(HOOKS);
        // repo, dir, hook selection, proceed
        let got = run(
            &c,
            &scripted("https://example.com/other.git\n~/dots\n2\n\n"),
        )
        .expect("run should succeed")
        .expect("the value should be present");
        assert_eq!(
            got.dotfiles_repo.as_deref(),
            Some("https://example.com/other.git")
        );
        assert_eq!(got.dotfiles_dir, "~/dots");
        assert_eq!(
            got.hooks
                .iter()
                .map(|h| h.name.as_str())
                .collect::<Vec<_>>(),
            ["b"]
        );
    }

    #[test]
    fn declining_the_plan_aborts() {
        let c = cfg("");
        assert!(
            run(&c, &scripted("\n\nn\n"))
                .expect("run should succeed")
                .is_none()
        );
    }

    #[test]
    fn empty_repo_is_rejected() {
        let c: NotstrapConfig = toml::from_str("[bootstrap]\ndotfiles_dir = \"~/x\"\n")
            .expect("from_str should succeed");
        assert!(run(&c, &scripted("\n\n")).is_err());
    }

    #[test]
    fn apply_overlays_choices() {
        let mut c = cfg(HOOKS);
        let choices = Choices {
            dotfiles_repo: Some("https://example.com/z.git".into()),
            dotfiles_dir: "~/z".into(),
            use_tailscale: false,
            hooks: vec![c.hooks[1].clone()],
        };
        apply(&mut c, choices);
        assert_eq!(
            c.bootstrap.dotfiles_repo.as_deref(),
            Some("https://example.com/z.git")
        );
        assert_eq!(c.bootstrap.dotfiles_dir, "~/z");
        assert_eq!(c.hooks.len(), 1);
    }
}
