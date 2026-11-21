use notnet::NotnetError;
use notnet::adapters::{FakeCommandRunner, PackageManagerInstaller};
use notnet::ports::TailscaleInstaller;

const SCRIPT: &str = "sh -c curl -fsSL https://tailscale.com/install.sh | sh";

#[test]
fn apt_is_preferred_when_present() {
    let runner = FakeCommandRunner::default().present(&["apt-get", "brew"]);
    PackageManagerInstaller::new(&runner)
        .install()
        .expect("install");
    assert_eq!(runner.calls(), ["apt-get install -y tailscale"]);
}

#[test]
fn falls_through_to_brew_when_apt_is_absent() {
    let runner = FakeCommandRunner::default().present(&["brew"]);
    PackageManagerInstaller::new(&runner)
        .install()
        .expect("install");
    assert_eq!(runner.calls(), ["brew install tailscale"]);
}

#[test]
fn falls_through_when_a_present_manager_fails() {
    let runner = FakeCommandRunner::default()
        .present(&["apt-get", "pacman"])
        .failing("apt-get install -y tailscale", "no repo");
    PackageManagerInstaller::new(&runner)
        .install()
        .expect("install");
    assert_eq!(
        runner.calls(),
        [
            "apt-get install -y tailscale",
            "pacman -S --noconfirm tailscale"
        ]
    );
}

#[test]
fn uses_the_official_script_when_no_manager_is_present() {
    let runner = FakeCommandRunner::default();
    PackageManagerInstaller::new(&runner)
        .install()
        .expect("install");
    assert_eq!(runner.calls(), [SCRIPT]);
}

#[test]
fn script_failure_is_an_install_error_with_stderr() {
    let runner = FakeCommandRunner::default().failing(SCRIPT, "curl: (6) offline");
    let err = PackageManagerInstaller::new(&runner)
        .install()
        .expect_err("fails");
    assert!(matches!(err, NotnetError::InstallFailed(m) if m == "curl: (6) offline"));
}
