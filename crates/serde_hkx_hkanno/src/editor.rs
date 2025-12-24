use snafu::ResultExt;
use std::{path::Path, str::FromStr as _};
use tokio::fs;

use crate::{parse_as_hkanno, parse_hkanno_str, HkannoError, IoSnafu, OutFormat};

/// Read hkanno from `xml`, `hkx` file.
///
/// # Return
/// - Returns hkanno string.
///
/// # Errors
/// - Returns `HkannoError` if reading the input file fails, or if parsing the hkx bytes fails.
pub async fn read_hkanno(input: &Path) -> Result<String, HkannoError> {
    let bytes = fs::read(&input)
        .await
        .with_context(|_| IoSnafu { path: input })?;

    let mut buffer = String::new();
    parse_as_hkanno(&bytes, &mut buffer, input).map(|anno| anno.to_string())
}

/// Apply hkanno to `xml`, `hkx` file.
///
/// # Errors
/// - Returns `HkannoError` if reading the input file fails,
///   if parsing the hkanno string fails, or if updating the hkx bytes fails.
pub async fn apply_hkanno(
    input: &Path,
    output: &Path,
    hkanno: &str,
    format: &str,
) -> Result<(), HkannoError> {
    let mut bytes = fs::read(&input)
        .await
        .with_context(|_| IoSnafu { path: input })?;

    let format = OutFormat::from_str(format).map_err(|_| HkannoError::InvalidOutputFormat {
        format: format.to_string(),
    })?;
    let updated = parse_hkanno_str(hkanno)?.update_hkx_bytes(&mut bytes, format, input)?;

    fs::write(&output, updated)
        .await
        .with_context(|_| IoSnafu { path: output })?;

    Ok(())
}

/// Apply hkanno to `xml`, `hkx` file and return updated xml string.
///
/// It can be used for previews to let users know about updated states.
///
/// # Errors
/// - Returns `HkannoError` if reading the input file fails,
///   if parsing the hkanno string fails, or if updating the hkx bytes fails.
pub async fn hkanno_apply_xml_string(input: &Path, hkanno: &str) -> Result<String, HkannoError> {
    let mut bytes = fs::read(&input)
        .await
        .with_context(|_| IoSnafu { path: input })?;

    let new_xml = parse_hkanno_str(hkanno)?.update_hkx_bytes(&mut bytes, OutFormat::Xml, input)?;
    Ok(String::from_utf8(new_xml)?)
}
