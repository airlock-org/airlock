//! Commands understood by the GUI executable and constructed by machine authors.

use airlock_machine_core::services::CaptureKind;
use airlock_nix_core::identity::GuestName;
use clap::Subcommand;
use std::path::PathBuf;

/// Installed executable name used by package metadata and command construction.
pub const EXECUTABLE: &str = "airlock-sys-gui-service";

/// CLI spelling shared by the window identity parser and its deferred script adapter.
pub const WINDOW_IDENTITY: &str = "window-identity";

/// Desktop operations whose invocations are also authored in system configuration.
#[derive(Debug, PartialEq, Subcommand)]
pub enum DesktopCommand {
    /// Deliver GNOME captures and route their notification actions to a personal VM.
    Capture {
        /// Generation-provided sender contract; omission uses the CLI default.
        #[arg(long, global = true)]
        config: Option<PathBuf>,
        /// Sender operation; receiver operations belong to the personal VM service.
        #[command(subcommand)]
        action: CaptureAction,
    },
    /// Apply a declarative border-style request over existing administrator transport.
    GuiStyle {
        /// Restore the booted generation's default before GUI daemons start.
        #[arg(long)]
        boot: bool,
    },
    /// Read a bounded X11 window identity with GUI-daemon provenance verification.
    #[command(name = WINDOW_IDENTITY)]
    WindowIdentity {
        /// Trusted GUI daemon executable supplied by the installed generation.
        #[arg(long)]
        guid: PathBuf,
        /// X11 window identifier encoded as hexadecimal by the GNOME extension.
        xid: String,
    },
    /// Watch dom0 startup status and offer a retry notification when degraded.
    BootNotification,
}

/// Operations accepted by the capture sender, shared by runtime parsing and authoring.
#[derive(Debug, PartialEq, Subcommand)]
pub enum CaptureAction {
    /// Validate and print sender configuration without contacting a VM or writing state.
    CheckConfig,
    /// Queue a file only after GNOME has finished writing it.
    Enqueue {
        /// Capture format and corresponding GNOME source directory.
        kind: CaptureKind,
        /// Completed local file; symlinks and unrelated directories are rejected.
        filename: PathBuf,
        /// Policy-authorized personal VM receiving the bytes.
        #[arg(long)]
        target: Option<GuestName>,
    },
    /// Retry pending deliveries and backfill existing completed captures once.
    Retry {
        /// Policy-authorized personal VM receiving the bytes.
        #[arg(long)]
        target: Option<GuestName>,
    },
    /// Open a previously delivered capture inside its receiving VM.
    Open {
        /// Media category used to find the acknowledged delivery.
        kind: CaptureKind,
        /// Local source of a completed, previously queued capture.
        filename: PathBuf,
        /// VM in which the received capture should be opened.
        #[arg(long)]
        target: Option<GuestName>,
    },
    /// Reveal a previously delivered capture in the receiving VM's file manager.
    Show {
        /// Media category used to find the acknowledged delivery.
        kind: CaptureKind,
        /// Local source of a completed, previously queued capture.
        filename: PathBuf,
        /// VM in which the received capture should be revealed.
        #[arg(long)]
        target: Option<GuestName>,
    },
}

impl DesktopCommand {
    /// Returns literal process arguments without shell quoting or starting a service.
    /// Paths retain their operating-system representation; authoring checks UTF-8 at its boundary.
    pub fn arguments(&self) -> Vec<std::ffi::OsString> {
        match self {
            Self::WindowIdentity { guid, xid } => vec![
                WINDOW_IDENTITY.into(),
                "--guid".into(),
                guid.as_os_str().to_owned(),
                xid.into(),
            ],
            Self::BootNotification => vec!["boot-notification".into()],
            Self::GuiStyle { boot } => {
                let mut args = vec!["gui-style".into()];
                if *boot {
                    args.push("--boot".into());
                }

                args
            }
            Self::Capture { config, action } => {
                let mut args = vec!["capture".into()];
                args.extend(action.arguments());
                if let Some(config) = config {
                    args.push("--config".into());
                    args.push(config.as_os_str().to_owned());
                }

                args
            }
        }
    }
}

impl CaptureAction {
    /// Encodes every sender operation, preserving paths as individual arguments.
    fn arguments(&self) -> Vec<std::ffi::OsString> {
        let (name, file, target) = match self {
            Self::CheckConfig => ("check-config", None, None),
            Self::Retry { target } => ("retry", None, target.as_ref()),
            Self::Enqueue {
                kind,
                filename,
                target,
            } => ("enqueue", Some((kind, filename)), target.as_ref()),
            Self::Open {
                kind,
                filename,
                target,
            } => ("open", Some((kind, filename)), target.as_ref()),
            Self::Show {
                kind,
                filename,
                target,
            } => ("show", Some((kind, filename)), target.as_ref()),
        };
        let mut args = vec![name.into()];
        if let Some((kind, filename)) = file {
            args.push(kind.as_str().into());
            args.push(filename.as_os_str().to_owned());
        }
        if let Some(target) = target {
            args.push("--target".into());
            args.push(target.as_str().into());
        }

        args
    }
}
