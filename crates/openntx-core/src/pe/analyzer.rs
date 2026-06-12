use crate::pe::types::{PeAnalysis, PeArchitecture, PeFormat, PeImageKind, WindowsSubsystem};
use crate::{OpenNtxError, Result};
use std::fs;
use std::path::Path;

const DOS_MAGIC: &[u8; 2] = b"MZ";
const PE_MAGIC: &[u8; 4] = b"PE\0\0";
const IMAGE_FILE_DLL: u16 = 0x2000;

pub fn analyze_pe(path: impl AsRef<Path>) -> Result<PeAnalysis> {
    let path = path.as_ref();
    let metadata = fs::metadata(path).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            OpenNtxError::InvalidInput(format!("input file does not exist: {}", path.display()))
        } else {
            OpenNtxError::io(path, source)
        }
    })?;

    if !metadata.is_file() {
        return Err(OpenNtxError::InvalidInput(format!(
            "input is not a file: {}",
            path.display()
        )));
    }

    let bytes = fs::read(path).map_err(|source| OpenNtxError::io(path, source))?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("unknown")
        .to_string();

    if bytes.len() < 64 {
        return Ok(not_pe(
            path,
            file_name,
            "File is too small to contain a PE header",
        ));
    }

    if &bytes[0..2] != DOS_MAGIC {
        return Ok(not_pe(
            path,
            file_name,
            "File does not contain an MZ DOS header",
        ));
    }

    let pe_offset = read_u32(&bytes, 0x3c)
        .ok_or_else(|| OpenNtxError::InvalidInput("file ended before DOS PE offset".to_string()))?
        as usize;

    if pe_offset + 24 > bytes.len() {
        return Ok(not_pe(
            path,
            file_name,
            "MZ header exists, but PE header offset is outside the file",
        ));
    }

    if &bytes[pe_offset..pe_offset + 4] != PE_MAGIC {
        return Ok(not_pe(
            path,
            file_name,
            "MZ header exists, but PE signature was not found",
        ));
    }

    let machine = read_u16(&bytes, pe_offset + 4).unwrap_or(0);
    let optional_header_size = read_u16(&bytes, pe_offset + 20).unwrap_or(0) as usize;
    let characteristics = read_u16(&bytes, pe_offset + 22).unwrap_or(0);
    let optional_offset = pe_offset + 24;
    let optional_end = optional_offset.saturating_add(optional_header_size);

    let format = match read_u16(&bytes, optional_offset) {
        Some(0x10b) => PeFormat::Pe32,
        Some(0x20b) => PeFormat::Pe32Plus,
        _ => PeFormat::Unknown,
    };

    let subsystem = if optional_end <= bytes.len() && optional_header_size >= 70 {
        read_u16(&bytes, optional_offset + 68).map(map_subsystem)
    } else {
        None
    };

    let image_kind = if characteristics & IMAGE_FILE_DLL != 0 {
        PeImageKind::DynamicLibrary
    } else {
        PeImageKind::Executable
    };

    Ok(PeAnalysis {
        path: path.to_path_buf(),
        file_name: file_name.clone(),
        is_pe: true,
        format,
        architecture: map_architecture(machine),
        image_kind,
        subsystem,
        suggested_mode: suggested_mode(&file_name).to_string(),
        status: "analysis-only".to_string(),
        warnings: Vec::new(),
    })
}

fn not_pe(path: &Path, file_name: String, warning: &str) -> PeAnalysis {
    PeAnalysis {
        path: path.to_path_buf(),
        file_name,
        is_pe: false,
        format: PeFormat::Unknown,
        architecture: PeArchitecture::Unknown(0),
        image_kind: PeImageKind::Unknown,
        subsystem: None,
        suggested_mode: "unsupported".to_string(),
        status: "not-pe".to_string(),
        warnings: vec![warning.to_string()],
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let slice = bytes.get(offset..offset + 2)?;
    Some(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn map_architecture(machine: u16) -> PeArchitecture {
    match machine {
        0x014c => PeArchitecture::X86,
        0x8664 => PeArchitecture::X86_64,
        0x01c0 | 0x01c4 => PeArchitecture::Arm,
        0xaa64 => PeArchitecture::Arm64,
        other => PeArchitecture::Unknown(other),
    }
}

fn map_subsystem(value: u16) -> WindowsSubsystem {
    match value {
        1 => WindowsSubsystem::Native,
        2 => WindowsSubsystem::WindowsGui,
        3 => WindowsSubsystem::WindowsCui,
        other => WindowsSubsystem::Unknown(other),
    }
}

fn suggested_mode(file_name: &str) -> &'static str {
    let lower = file_name.to_ascii_lowercase();
    if lower.contains("setup") || lower.contains("install") || lower.contains("installer") {
        "capture-install"
    } else {
        "run-once"
    }
}
