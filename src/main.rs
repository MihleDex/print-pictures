use clap::{Parser, Subcommand};
use print_pictures::cups::{self, SubmitRequest};
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
    /// List the printers and what each one offers
    Printers,
    /// Print without opening the window
    Print {
        #[command(flatten)]
        job: JobArgs,
        #[arg(long)]
        printer: String,
        /// A paper size id from `printers`; the printer's default if left out
        #[arg(long)]
        paper: Option<String>,
        /// A printer option, as KEY=VALUE; may be repeated
        #[arg(short = 'o', value_name = "KEY=VALUE", value_parser = option)]
        options: Vec<(String, String)>,
    },
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

fn option(s: &str) -> Result<(String, String), String> {
    let (key, value) = s.split_once('=').ok_or("write it as KEY=VALUE")?;
    Ok((key.into(), value.into()))
}

fn report_skipped(skipped: &[(PathBuf, String)]) {
    for (path, why) in skipped {
        eprintln!("Skipped {}: {why}", path.display());
    }
}

/// Checks the files and builds the PDF, naming every picture left out.
/// Returns the page count and the pictures that are on those pages.
fn make_pdf(
    job: JobArgs,
    paper: Paper,
    out: &std::path::Path,
) -> Result<(usize, Vec<PathBuf>), JobError> {
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
            let mut printed = spec.files;
            printed.retain(|f| !built.skipped.iter().any(|(s, _)| s == f));
            Ok((built.pages, printed))
        }
        Err(e) => {
            if let JobError::NothingToPrint(skipped) = &e {
                report_skipped(skipped);
            }
            Err(e)
        }
    }
}

fn list_printers() -> ExitCode {
    let printers = match cups::printers() {
        Ok(printers) => printers,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    if printers.is_empty() {
        println!("No printers are installed.");
    }
    for printer in printers {
        let default = if printer.is_default { " (default)" } else { "" };
        let stopped = if printer.stopped { " (stopped)" } else { "" };
        let caps = cups::capabilities(&printer.name);
        let papers: Vec<&str> = caps.papers.iter().map(|p| p.id.as_str()).collect();
        let options: Vec<&str> = caps.options.iter().map(|o| o.keyword.as_str()).collect();
        println!("{}{default}{stopped}", printer.name);
        println!("  default paper: {}", caps.default_paper);
        println!("  papers: {}", papers.join(", "));
        println!("  options: {}", options.join(", "));
    }
    ExitCode::SUCCESS
}

fn print(
    job: JobArgs,
    printer: String,
    paper: Option<String>,
    options: Vec<(String, String)>,
) -> Result<String, String> {
    let caps = cups::capabilities(&printer);
    let id = paper.unwrap_or_else(|| caps.default_paper.clone());
    let Some(choice) = caps.paper(&id) else {
        let known: Vec<&str> = caps.papers.iter().map(|p| p.id.as_str()).collect();
        return Err(format!(
            "unknown paper {id}; choose one of {}",
            known.join(", ")
        ));
    };
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let pdf = dir.path().join("job.pdf");
    let (pages, printed) = make_pdf(job, choice.paper, &pdf).map_err(|e| e.to_string())?;
    let request = SubmitRequest {
        title: cups::job_title(&printed),
        printer,
        paper_id: id,
        options,
    };
    let job_id = cups::submit(&request, &pdf).map_err(|e| e.to_string())?;
    Ok(format!("{pages} page(s) sent as job {job_id}"))
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Printers => list_printers(),
        Command::Print {
            job,
            printer,
            paper,
            options,
        } => match print(job, printer, paper, options) {
            Ok(message) => {
                println!("{message}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        },
        Command::Render { job, paper, out } => {
            let paper = Paper {
                size: paper,
                margins: Margins::default(),
            };
            match make_pdf(job, paper, &out) {
                Ok((pages, _)) => {
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
