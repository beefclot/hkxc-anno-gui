// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AnnotationFile {
    hkx_path: String,     // Original HKX file path
    anno_path: String,    // Path to annotation .txt file (same dir as HKX)
    display_name: String, // Filename to show in UI
    content: String,      // Annotation content
}

// Get the CLI executable path
fn get_cli_path(_app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    let cli_name = "hkxc-anno-cli.exe";
    
    // Check 1: Current working directory
    let cwd_path = std::env::current_dir()
        .ok()
        .map(|p| p.join(cli_name));
    if let Some(ref path) = cwd_path {
        if path.exists() {
            return Ok(path.clone());
        }
    }
    
    // Check 2: Project root (for development)
    if let Ok(exe) = std::env::current_exe() {
        let project_root = exe
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .and_then(|p| p.parent());
        
        if let Some(root) = project_root {
            let cli_in_root = root.parent().map(|p| p.join(cli_name));
            if let Some(ref path) = cli_in_root {
                if path.exists() {
                    return Ok(path.clone());
                }
            }
        }
    }
    
    // Check 3: Alongside the executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let cli_path = exe_dir.join(cli_name);
            if cli_path.exists() {
                return Ok(cli_path);
            }
        }
    }
    
    Err(format!(
        "Could not find {}. Please ensure it's in the same folder as the application.",
        cli_name
    ))
}

// Collect all HKX files from input paths
fn collect_hkx_files(input_paths: &[String]) -> Result<Vec<PathBuf>, String> {
    let mut hkx_files = Vec::new();
    
    for input_path in input_paths {
        // Tauri provides absolute paths for dropped files
        // Convert string to PathBuf directly
        let path = PathBuf::from(input_path);
        
        // Verify path exists
        if !path.exists() {
            return Err(format!("Path does not exist: {}", path.display()));
        }
        
        if path.is_file() {
            // Case-insensitive extension check
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if ext.eq_ignore_ascii_case("hkx") {
                    hkx_files.push(path);
                }
            }
        } else if path.is_dir() {
            visit_dirs(&path, &mut hkx_files)?;
        }
    }
    
    Ok(hkx_files)
}

// Recursively visit directories to find HKX files
fn visit_dirs(dir: &Path, hkx_files: &mut Vec<PathBuf>) -> Result<(), String> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)
            .map_err(|e| format!("Failed to read directory {}: {}", dir.display(), e))? 
        {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let path = entry.path();
            
            if path.is_dir() {
                visit_dirs(&path, hkx_files)?;
            } else if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if ext.eq_ignore_ascii_case("hkx") {
                    hkx_files.push(path);
                }
            }
        }
    }
    Ok(())
}

#[tauri::command]
async fn dump_annotations(
    input: Vec<String>,
    app_handle: tauri::AppHandle,
) -> Result<Vec<AnnotationFile>, String> {
    let cli_path = get_cli_path(&app_handle)?;
    
    // Collect all HKX files
    let hkx_files = collect_hkx_files(&input)?;
    
    if hkx_files.is_empty() {
        return Err(format!(
            "No HKX files found in the provided input. Input paths: {:?}",
            input
        ));
    }
    
    let mut results = Vec::new();
    
    // Process each HKX file individually
    for hkx_path in hkx_files.iter() {
        // Run: hkxc-anno-cli dump -i <file.hkx>
        // CLI will create <file>.txt in the same directory as the HKX
        let output = Command::new(&cli_path)
            .arg("dump")
            .arg("-i")
            .arg(hkx_path)
            .output()
            .map_err(|e| format!("Failed to execute CLI: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "CLI dump failed for {}: {}",
                hkx_path.display(),
                stderr
            ));
        }
        
        // The annotation file is created next to the HKX with .txt extension
        let anno_path = hkx_path.with_extension("txt");
        
        // Read the dumped annotation file
        let content = fs::read_to_string(&anno_path)
            .map_err(|e| format!(
                "Failed to read annotation file {}: {}",
                anno_path.display(),
                e
            ))?;
        
        // Immediately delete the .txt file after reading (we have it in memory now)
        if anno_path.exists() {
            fs::remove_file(&anno_path)
                .map_err(|e| format!(
                    "Failed to delete temporary annotation file {}: {}",
                    anno_path.display(),
                    e
                ))?;
        }
        
        let display_name = hkx_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown.hkx")
            .to_string();
        
        // Add to results - anno_path is kept for reference but file is deleted
        results.push(AnnotationFile {
            hkx_path: hkx_path.to_string_lossy().to_string(),
            anno_path: anno_path.to_string_lossy().to_string(),
            display_name,
            content,
        });
    }
    
    Ok(results)
}

#[tauri::command]
async fn update_annotations(
    files: Vec<AnnotationFile>,
    format: String,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    if files.is_empty() {
        return Err("No files to update".to_string());
    }
    
    let cli_path = get_cli_path(&app_handle)?;
    let mut total_updated = 0;
    
    // Update each file individually
    for file in files {
        // Verify original HKX file still exists
        let hkx_path = Path::new(&file.hkx_path);
        if !hkx_path.exists() {
            return Err(format!(
                "Original HKX file not found: {}. It may have been moved or deleted.",
                file.hkx_path
            ));
        }
        
        // Create temporary .txt file with updated content
        let anno_path = PathBuf::from(&file.anno_path);
        fs::write(&anno_path, &file.content)
            .map_err(|e| format!("Failed to write temporary annotation file: {}", e))?;
        
        // Run: hkxc-anno-cli update -i <file.hkx> -v <format>
        // CLI will use the .txt file in the same directory (default behavior)
        let output = Command::new(&cli_path)
            .arg("update")
            .arg("-i")
            .arg(&file.hkx_path)
            .arg("-v")
            .arg(&format)
            .output()
            .map_err(|e| format!("Failed to execute CLI: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Clean up temp file on error
            let _ = fs::remove_file(&anno_path);
            return Err(format!(
                "Update failed for {}: {}\nOutput: {}",
                file.display_name, stderr, stdout
            ));
        }
        
        // Delete the temporary .txt file after successful update
        if anno_path.exists() {
            fs::remove_file(&anno_path)
                .map_err(|e| format!(
                    "Failed to delete temporary annotation file: {}",
                    e
                ))?;
        }
        
        total_updated += 1;
    }
    
    Ok(format!("Updated {} file(s)", total_updated))
}

#[tauri::command]
async fn cleanup_annotation(
    anno_path: String,
) -> Result<(), String> {
    let path = PathBuf::from(&anno_path);
    
    if !path.exists() {
        // File doesn't exist, nothing to clean up
        return Ok(());
    }
    
    // Check if it's a .txt file (case-insensitive)
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        if ext.eq_ignore_ascii_case("txt") {
            fs::remove_file(&path)
                .map_err(|e| format!("Failed to delete annotation file {}: {}", path.display(), e))?;
        }
    }
    
    Ok(())
}

#[tauri::command]
async fn cleanup_all_annotations(
    anno_paths: Vec<String>,
) -> Result<(), String> {
    for anno_path in anno_paths {
        let path = PathBuf::from(&anno_path);
        
        if path.exists() {
            // Check if it's a .txt file (case-insensitive)
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if ext.eq_ignore_ascii_case("txt") {
                    // Ignore errors during bulk cleanup
                    if let Err(e) = fs::remove_file(&path) {
                        eprintln!("Warning: Failed to delete {}: {}", path.display(), e);
                    }
                }
            }
        }
    }
    
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            dump_annotations, 
            update_annotations,
            cleanup_annotation,
            cleanup_all_annotations
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

