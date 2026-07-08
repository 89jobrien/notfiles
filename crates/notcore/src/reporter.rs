use std::path::Path;

/// Events emitted during link/unlink/status operations.
#[derive(Debug, Clone)]
pub enum LinkEvent<'a> {
    Link {
        source: &'a str,
        target: &'a Path,
    },
    Copy {
        source: &'a str,
        target: &'a Path,
    },
    Skip {
        source: &'a str,
        reason: &'a str,
    },
    Backup {
        from: &'a Path,
        to: &'a Path,
    },
    Remove {
        target: &'a Path,
    },
    CreateDir {
        path: &'a Path,
    },
    DryRun {
        action: &'a str,
        source: &'a str,
        target: &'a Path,
    },
}

/// Port for reporting link/unlink progress. Library code calls this
/// instead of println!.
pub trait Reporter {
    fn report(&self, event: &LinkEvent<'_>);
}

/// No-op reporter for when output is unwanted.
pub struct SilentReporter;

impl Reporter for SilentReporter {
    fn report(&self, _event: &LinkEvent<'_>) {}
}
