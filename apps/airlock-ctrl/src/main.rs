mod branding;
mod effects;
mod file_tree;
mod icons;
mod model;
mod selection;
mod theme;
mod ui;

use anyhow::{Context, Result, bail};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Decision {
    Approve,
    Reject,
}

#[derive(Parser, Debug)]
#[command(
    version,
    about = "Review Airlock source locally. Never connects to or deploys a machine."
)]
struct Args {
    /// Show the built-in example (also the default when no repository is supplied).
    #[arg(long, conflicts_with = "repo")]
    demo: bool,
    /// Repository containing the committed baseline and candidate.
    #[arg(long, requires = "base")]
    repo: Option<PathBuf>,
    /// Explicit comparison baseline. Local Git history is not a deployed baseline.
    #[arg(long, requires = "repo")]
    base: Option<String>,
    /// Candidate commit or ref, resolved once to an immutable commit.
    #[arg(long, default_value = "HEAD")]
    candidate: String,
    /// Optional saved dom0 plan/response JSON. Displayed as supplied, not authenticated.
    #[arg(long, requires = "repo")]
    plan: Option<PathBuf>,
    /// Write local decision receipts here. Receipts grant no deployment authority.
    #[arg(long)]
    decisions_dir: Option<PathBuf>,
    /// Print the review and its content fingerprint as JSON without opening a window.
    #[arg(long)]
    inspect: bool,
    /// Record a local decision without a window; intended for offline review tooling.
    #[arg(long, requires = "decisions_dir", conflicts_with = "screenshot")]
    record_decision: Option<Decision>,
    /// Render a PNG of the window and exit, for local visual checks.
    #[arg(long, conflicts_with = "inspect")]
    screenshot: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let review = if let Some(repo) = &args.repo {
        model::Review::from_git(
            repo,
            args.base.as_deref().unwrap(),
            &args.candidate,
            args.plan.as_deref(),
        )?
    } else {
        if args.record_decision.is_some() {
            bail!("demo decisions stay in memory; use --repo to write a local receipt");
        }
        model::Review::demo()
    };
    if let Some(decision) = args.record_decision {
        let path = review.record_decision(decision, args.decisions_dir.as_deref().unwrap())?;
        println!("{}", path.display());
        return Ok(());
    }
    if args.inspect {
        println!("{}", serde_json::to_string_pretty(&review)?);
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Airlock Control")
            .with_decorations(true)
            .with_app_id("org.airlock.ctrl")
            .with_icon(branding::logo().context("loading the embedded Airlock logo")?)
            .with_inner_size([1600.0, 820.0])
            .with_min_inner_size([880.0, 600.0]),
        renderer: eframe::Renderer::Glow,
        hardware_acceleration: eframe::HardwareAcceleration::Preferred,
        ..Default::default()
    };
    eframe::run_native(
        "Airlock Control",
        options,
        Box::new(move |cc| {
            Ok(Box::new(ui::Controller::new(
                cc,
                review,
                args.decisions_dir,
                args.screenshot,
            )))
        }),
    )
    .map_err(|error| anyhow::anyhow!(error.to_string()))
    .context("opening Airlock Control")
}
