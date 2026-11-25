use clap::{Parser, Subcommand};
use print_pictures::job::{JobError, JobSpec, build_pdf, check_files};
use print_pictures::layout::{LayoutId, Margins, Paper, SizeMm};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "print-pictures", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Make the print PDF without sending it to a printer
    Render {
        #[command(flatten)]
        job: JobArgs,
        /// A4, A3, Letter or A6
        #[arg(long, default_value = "A4", value_parser = paper_size)]
        paper: SizeMm,
        /// The PDF to write
        #[arg(short, long)]
        out: PathBuf,
    },
}

#[derive(clap::Args)]
struct JobArgs {
    /// full, 20x25, 13x18, 10x15, 9x13, wallet or contact
    #[arg(long, value_parser = layout_id)]
    layout: LayoutId,
    /// Copies of each picture
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..=99))]
    copies: u32,
    /// Show the whole picture instead of filling the frame
    #[arg(long)]
    no_fit: bool,
    /// Do not sharpen for printing
    #[arg(long)]
    no_sharpen: bool,
    #[arg(required = true)]
    files: Vec<PathBuf>,
}

fn layout_id(s: &str) -> Result<LayoutId, String> {
    LayoutId::from_id(s).ok_or_else(|| "unknown layout".into())
}

fn paper_size(s: &str) -> Result<SizeMm, String> {
    let (w, h) = match s {
        "A4" => (210.0, 297.0),
        "A3" => (297.0, 420.0),
        "Letter" => (215.9, 279.4),
        "A6" => (105.0, 148.0),
        _ => return Err("use A4, A3, Letter or A6".into()),
    };
    Ok(SizeMm { w, h })
}

fn report_skipped(skipped: &[(PathBuf, String)]) {
    for (path, why) in skipped {
        eprintln!("Skipped {}: {why}", path.display());
    }
}

/// Checks the files and builds the PDF, naming every picture left out.
fn make_pdf(job: JobArgs, paper: Paper, out: &std::path::Path) -> Result<usize, JobError> {
    let checked = check_files(&job.files);
    report_skipped(&checked.rejected);
    let spec = JobSpec {
        files: checked.usable,
        layout: job.layout,
        paper,
        copies: job.copies,
        fill: !job.no_fit,
        sharpen: !job.no_sharpen,
    };
    match build_pdf(&spec, out) {
        Ok(built) => {
            report_skipped(&built.skipped);
            Ok(built.pages)
        }
        Err(e) => {
            if let JobError::NothingToPrint(skipped) = &e {
                report_skipped(skipped);
            }
            Err(e)
        }
    }
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Render { job, paper, out } => {
            let paper = Paper {
                size: paper,
                margins: Margins::default(),
            };
            match make_pdf(job, paper, &out) {
                Ok(pages) => {
                    println!("{pages} page(s) written to {}", out.display());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
    }
}
