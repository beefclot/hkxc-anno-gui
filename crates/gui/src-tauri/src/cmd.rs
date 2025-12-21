use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use serde_hkx_hkanno::{editor::read_hkanno, file_collector::par_collect_hkx_files, HkannoError};
use tokio::task::JoinSet;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct AnnotationFile {
    /// Original HKX file path
    hkx_path: PathBuf,
    /// Path to annotation .txt file (same dir as HKX)
    anno_path: PathBuf,
    /// Filename to show in UI
    display_name: String,
    /// Annotation content
    content: String,
}

#[tauri::command]
pub(crate) async fn dump_annotations(input: Vec<PathBuf>) -> Result<Vec<AnnotationFile>, String> {
    let hkx_files = par_collect_hkx_files(input).map_err(|e| e.to_string())?;

    let mut handles: JoinSet<Result<AnnotationFile, HkannoError>> = JoinSet::new();

    for hkx_path in hkx_files {
        handles.spawn(async move {
            let content = read_hkanno(&hkx_path)
                .await
                .map_err(|e| HkannoError::HkxError {
                    source: Box::new(e),
                    path: hkx_path.clone(),
                })?;
            let anno_path = hkx_path.with_extension("txt"); // dummy

            let display_name = hkx_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown.hkx")
                .to_string();

            Ok(AnnotationFile {
                hkx_path,
                anno_path,
                display_name,
                content,
            })
        });
    }

    let mut annotation_files = Vec::new();
    let mut errors = Vec::new();

    while let Some(result) = handles.join_next().await {
        match result {
            Ok(Ok(file)) => annotation_files.push(file),
            Ok(Err(err)) => errors.push(err.to_string()),
            Err(join_err) => errors.push(format!("Task panicked: {}", join_err)),
        }
    }

    let err_msg = if !errors.is_empty() {
        let err_msg = errors.join("\n");
        #[cfg(feature = "tracing")]
        tracing::error!("Errors during dump:\n{err_msg}");
        err_msg
    } else {
        String::new()
    };

    if !annotation_files.is_empty() {
        return Ok(annotation_files);
    }

    // When the user selects a directory, some HKX files lack animations.
    // Therefore, to avoid a poor user experience caused by errors in a single file, we will only log the issue.
    if annotation_files.is_empty() && !errors.is_empty() {
        return Err(err_msg);
    }

    Ok(annotation_files)
}

#[tauri::command]
pub(crate) async fn update_annotations(
    files: Vec<AnnotationFile>,
    format: String,
) -> Result<String, String> {
    let total_updated = Arc::new(AtomicUsize::new(0));
    let format = Arc::new(format);

    let mut handles: JoinSet<Result<(), HkannoError>> = JoinSet::new();
    for AnnotationFile {
        hkx_path, content, ..
    } in files
    {
        let total_updated = Arc::clone(&total_updated);
        let format = Arc::clone(&format);

        handles.spawn(async move {
            let output_path = match format.as_str() {
                format if format.eq_ignore_ascii_case("xml") => hkx_path.with_extension("xml"),
                _ => hkx_path.with_extension("hkx"),
            };
            serde_hkx_hkanno::editor::apply_hkanno(
                &hkx_path,
                &output_path, // in-place update
                &content,     // hkanno text
                &format,
            )
            .await?;
            total_updated.fetch_add(1, Ordering::Relaxed);
            Ok(())
        });
    }

    let mut errors = Vec::new();
    while let Some(result) = handles.join_next().await {
        match result {
            Ok(Ok(())) => (),
            Ok(Err(err)) => errors.push(err.to_string()),
            Err(err) => errors.push(err.to_string()),
        }
    }

    if !errors.is_empty() {
        let err_msg = errors.join("\n");
        #[cfg(feature = "tracing")]
        tracing::error!("Errors during update annotations:\n{err_msg}");
        return Err(err_msg);
    }

    Ok(format!(
        "Updated {} file(s)",
        total_updated.load(Ordering::Relaxed)
    ))
}
