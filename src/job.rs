//! File checking and PDF building.

use crate::image_source::{LoadError, Raster, load, probe};
use crate::layout::{LayoutId, Paper, RectMm, paginate, slots};
use crate::render::{draw_caption, draw_picture, fit, split_caption};
use crate::sharpen::sharpen;
use std::path::{Path, PathBuf};

pub const PRINT_DPI: u32 = 300;
const MM_PER_INCH: f64 = 25.4;
const POINTS_PER_MM: f64 = 72.0 / MM_PER_INCH;

#[derive(Clone, Debug)]
pub struct JobSpec {
    pub files: Vec<PathBuf>,
    pub layout: LayoutId,
    pub paper: Paper,
    pub copies: u32,
    pub fill: bool,
    pub sharpen: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Checked {
    pub usable: Vec<PathBuf>,
    pub rejected: Vec<(PathBuf, String)>,
}

pub fn check_files(files: &[PathBuf]) -> Checked {
    let mut checked = Checked::default();
    for file in files {
        match probe(file) {
            Ok(_) => checked.usable.push(file.clone()),
            Err(e) => checked.rejected.push((file.clone(), e.to_string())),
        }
    }
    checked
}

#[derive(Clone, Debug)]
pub struct Built {
    pub pages: usize,
    /// Pictures that passed the check but then failed to load.
    pub skipped: Vec<(PathBuf, String)>,
}

#[derive(Debug, thiserror::Error)]
pub enum JobError {
    /// Carries the pictures that were skipped, if that is why nothing is left.
    #[error("there is nothing to print")]
    NothingToPrint(Vec<(PathBuf, String)>),
    #[error("this layout does not fit the paper")]
    LayoutUnavailable,
    #[error("could not make the PDF: {0}")]
    Pdf(String),
}

fn pdf_error(e: impl std::fmt::Display) -> JobError {
    JobError::Pdf(e.to_string())
}

enum PageError {
    Load(PathBuf, LoadError),
    Cairo(cairo::Error),
}

impl From<cairo::Error> for PageError {
    fn from(e: cairo::Error) -> Self {
        PageError::Cairo(e)
    }
}

/// Writes the job to `out`, which only appears once it is complete.
pub fn build_pdf(spec: &JobSpec, out: &Path) -> Result<Built, JobError> {
    if slots(spec.layout, &spec.paper).is_empty() {
        return Err(JobError::LayoutUnavailable);
    }
    let dir = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut files = spec.files.clone();
    let mut skipped = Vec::new();
    // shortcut: one rebuild per damaged picture; fine while they are rare
    while !files.is_empty() {
        let temp = tempfile::NamedTempFile::new_in(dir).map_err(pdf_error)?;
        match write_pages(spec, &files, temp.path()) {
            Ok(pages) => {
                temp.persist(out).map_err(pdf_error)?;
                return Ok(Built { pages, skipped });
            }
            Err(PageError::Load(path, e)) => {
                files.retain(|f| f != &path);
                skipped.push((path, e.to_string()));
            }
            Err(PageError::Cairo(e)) => return Err(pdf_error(e)),
        }
    }
    Err(JobError::NothingToPrint(skipped))
}

fn write_pages(spec: &JobSpec, files: &[PathBuf], out: &Path) -> Result<usize, PageError> {
    let size = spec.paper.size;
    let surface = cairo::PdfSurface::new(size.w * POINTS_PER_MM, size.h * POINTS_PER_MM, out)?;
    let cr = cairo::Context::new(&surface)?;
    cr.scale(POINTS_PER_MM, POINTS_PER_MM);

    let pages = paginate(spec.layout, &spec.paper, files.len(), spec.copies);
    // Copies sit next to each other, so keeping one decoded picture is enough.
    let mut decoded: Option<(usize, Raster)> = None;
    for page in &pages {
        for placement in page {
            let path = &files[placement.image];
            let (area, strip) = if spec.layout == LayoutId::ContactSheet {
                let (area, strip) = split_caption(placement.slot);
                (area, Some(strip))
            } else {
                (placement.slot, None)
            };
            let raster = match decoded.take() {
                Some((image, raster)) if image == placement.image => raster,
                _ => load_for(path, area, spec.sharpen)
                    .map_err(|e| PageError::Load(path.clone(), e))?,
            };
            draw_picture(
                &cr,
                &raster,
                &fit(raster.width, raster.height, area, spec.fill),
            )?;
            decoded = Some((placement.image, raster));
            if let Some(strip) = strip {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                draw_caption(&cr, &name, strip)?;
            }
        }
        cr.show_page()?;
    }
    surface.finish();
    surface.status()?;
    Ok(pages.len())
}

fn load_for(path: &Path, area: RectMm, sharpened: bool) -> Result<Raster, LoadError> {
    let pixels = |mm: f64| (mm / MM_PER_INCH * f64::from(PRINT_DPI)).ceil() as u32;
    let mut raster = load(path, pixels(area.w.max(area.h)), pixels(area.w.min(area.h)))?;
    if sharpened {
        sharpen(&mut raster);
    }
    Ok(raster)
}
