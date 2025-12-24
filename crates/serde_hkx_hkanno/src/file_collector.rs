use rayon::prelude::*;
use snafu::ResultExt as _;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Collect all `.hkx`, `.xml` files from the given input paths (files or directories).
pub fn par_collect_hkx_files(input_paths: Vec<PathBuf>) -> Result<Vec<PathBuf>, CollectError> {
    let (files, errors): (Vec<_>, Vec<_>) = input_paths
        .into_par_iter()
        .map(collect_from_path)
        .partition_map(|result| match result {
            Ok(paths) => rayon::iter::Either::Left(paths),
            Err(err) => rayon::iter::Either::Right(err),
        });

    if !errors.is_empty() {
        let err = CollectError::CollectErrors { errors };
        tracing::error!("{err}");
        return Err(err);
    }

    let mut files = files.into_iter().flatten().collect::<Vec<_>>();
    files.par_sort_unstable();
    files.dedup();
    Ok(files)
}

fn collect_from_path(path: PathBuf) -> Result<Vec<PathBuf>, CollectError> {
    if !path.exists() {
        return PathNotFoundSnafu { path }.fail();
    }

    if path.is_file() {
        return Ok(is_hkx(&path).then(|| vec![path]).unwrap_or_default());
    }

    if path.is_dir() {
        let read_dir = fs::read_dir(&path).context(ReadDirSnafu { path: path.clone() })?;

        return read_dir
            .par_bridge()
            .map(|entry| {
                let entry = entry.context(ReadDirEntrySnafu)?;
                collect_from_path(entry.path())
            })
            .collect::<Result<Vec<Vec<PathBuf>>, CollectError>>()
            .map(|lists| lists.into_iter().flatten().collect());
    }

    Ok(Vec::new())
}

const ALLOWED_EXTENSIONS: &[&str] = &["hkx", "xml"];

#[inline]
fn is_hkx(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|ext| ext.to_str()) else {
        return false;
    };
    ALLOWED_EXTENSIONS
        .iter()
        .any(|&ext| ext.eq_ignore_ascii_case(extension))
}

#[derive(Debug, snafu::Snafu)]
pub enum CollectError {
    #[snafu(display("Path does not exist: {}", path.display()))]
    PathNotFound { path: PathBuf },

    #[snafu(display("Failed to read directory {}: {source}", path.display()))]
    ReadDir {
        path: PathBuf,
        source: std::io::Error,
    },

    #[snafu(display("Failed to read directory entry: {source}"))]
    ReadDirEntry { source: std::io::Error },

    /// Multiple errors during file collection
    #[snafu(display("Multiple errors during file collection:\n{}", errors.par_iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n")))]
    CollectErrors { errors: Vec<CollectError> },
}
