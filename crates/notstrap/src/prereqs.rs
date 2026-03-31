use anyhow::Result;
use which::which;

pub struct Prereq {
    pub cmd: &'static str,
    pub install_hint: &'static str,
}

const PREREQS: &[Prereq] = &[
    Prereq { cmd: "nu",   install_hint: "brew install nushell  OR  nix-env -iA nixpkgs.nushell" },
    Prereq { cmd: "sops", install_hint: "brew install sops  OR  nix-env -iA nixpkgs.sops" },
    Prereq { cmd: "age",  install_hint: "brew install age   OR  nix-env -iA nixpkgs.age" },
];

pub fn check_prerequisites() -> Result<()> {
    let missing: Vec<&Prereq> = PREREQS.iter().filter(|p| which(p.cmd).is_err()).collect();
    if missing.is_empty() {
        return Ok(());
    }
    eprintln!("Missing required tools:\n");
    for p in &missing {
        eprintln!("  {} — {}", p.cmd, p.install_hint);
    }
    anyhow::bail!("{} prerequisite(s) missing", missing.len())
}
