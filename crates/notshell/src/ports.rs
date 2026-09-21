use crate::ir::ShellConfig;

/// A shell-specific code generator. One adapter per target shell.
pub trait ShellEmitter {
    /// Shell identifier used in config (`"nu"`, `"fish"`, `"zsh"`, `"bash"`).
    fn id(&self) -> &'static str;

    /// File extension for generated files, without the leading dot.
    fn ext(&self) -> &'static str;

    /// Render the IR as native shell syntax.
    fn emit(&self, config: &ShellConfig) -> String;
}
