use crate::pe::types::{
    CoffHeader, DosHeader, OptionalHeader, PeAnalysis, PeArchitecture, PeFormat, PeImageKind,
    PeSection, WindowsSubsystem,
};
use crate::{OpenNtxError, Result};
use std::fs;
use std::path::Path;

const DOS_MAGIC: &[u8; 2] = b"MZ";
const PE_MAGIC: &[u8; 4] = b"PE\0\0";
const IMAGE_FILE_DLL: u16 = 0x2000;
const COFF_HEADER_SIZE: usize = 20;
const SECTION_HEADER_SIZE: usize = 40;
const IMPORT_DIRECTORY_INDEX: usize = 1;
const DATA_DIRECTORY_SIZE: usize = 8;
const IMPORT_DESCRIPTOR_SIZE: usize = 20;
const MAX_IMPORT_DESCRIPTORS: usize = 4096;

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

    if pe_offset + 4 + COFF_HEADER_SIZE > bytes.len() {
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

    let coff_offset = pe_offset + 4;
    let coff_header = CoffHeader {
        machine_raw: read_u16(&bytes, coff_offset).unwrap_or(0),
        number_of_sections: read_u16(&bytes, coff_offset + 2).unwrap_or(0),
        time_date_stamp: read_u32(&bytes, coff_offset + 4).unwrap_or(0),
        pointer_to_symbol_table: read_u32(&bytes, coff_offset + 8).unwrap_or(0),
        number_of_symbols: read_u32(&bytes, coff_offset + 12).unwrap_or(0),
        size_of_optional_header: read_u16(&bytes, coff_offset + 16).unwrap_or(0),
        characteristics: read_u16(&bytes, coff_offset + 18).unwrap_or(0),
    };

    let optional_header_size = coff_header.size_of_optional_header as usize;
    let optional_offset = pe_offset + 4 + COFF_HEADER_SIZE;
    let optional_end = optional_offset.saturating_add(optional_header_size);
    let mut warnings = Vec::new();

    if optional_end > bytes.len() {
        warnings.push("Optional header extends beyond file size".to_string());
    }

    let format = match read_u16(&bytes, optional_offset) {
        Some(0x10b) => PeFormat::Pe32,
        Some(0x20b) => PeFormat::Pe32Plus,
        _ => PeFormat::Unknown,
    };

    let optional_header = parse_optional_header(&bytes, optional_offset, optional_header_size);
    let subsystem = optional_header
        .as_ref()
        .map(|header| header.subsystem.clone());

    let sections = parse_sections(
        &bytes,
        optional_end,
        coff_header.number_of_sections,
        &mut warnings,
    );
    let imported_dlls = parse_imported_dlls(
        &bytes,
        optional_offset,
        optional_header_size,
        &format,
        &sections,
        &mut warnings,
    );

    let image_kind = if coff_header.characteristics & IMAGE_FILE_DLL != 0 {
        PeImageKind::DynamicLibrary
    } else {
        PeImageKind::Executable
    };

    let (mode, mode_reason) = suggested_mode(&file_name, &image_kind, subsystem.as_ref());

    Ok(PeAnalysis {
        path: path.to_path_buf(),
        file_name: file_name.clone(),
        is_pe: true,
        format,
        architecture: map_architecture(coff_header.machine_raw),
        image_kind,
        subsystem,
        dos_header: Some(DosHeader {
            e_lfanew: pe_offset as u32,
        }),
        coff_header: Some(coff_header),
        optional_header,
        sections,
        imported_dlls,
        suggested_mode: mode.to_string(),
        install_mode_reason: mode_reason.to_string(),
        status: "analysis-only".to_string(),
        warnings,
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
        dos_header: None,
        coff_header: None,
        optional_header: None,
        sections: Vec::new(),
        imported_dlls: Vec::new(),
        suggested_mode: "unsupported".to_string(),
        install_mode_reason: "not a PE file".to_string(),
        status: "not-pe".to_string(),
        warnings: vec![warning.to_string()],
    }
}

fn parse_optional_header(
    bytes: &[u8],
    optional_offset: usize,
    optional_header_size: usize,
) -> Option<OptionalHeader> {
    if optional_header_size < 70 {
        return None;
    }

    let optional_end = optional_offset.checked_add(optional_header_size)?;
    if optional_end > bytes.len() {
        return None;
    }

    let magic_raw = read_u16(bytes, optional_offset)?;
    let entry_point_rva = read_u32(bytes, optional_offset + 16)?;
    let image_base = match magic_raw {
        0x10b => read_u32(bytes, optional_offset + 28)? as u64,
        0x20b => read_u64(bytes, optional_offset + 24)?,
        _ => 0,
    };
    let subsystem = read_u16(bytes, optional_offset + 68).map(map_subsystem)?;
    let number_of_rva_and_sizes = match magic_raw {
        0x10b if optional_header_size >= 96 => read_u32(bytes, optional_offset + 92).unwrap_or(0),
        0x20b if optional_header_size >= 112 => read_u32(bytes, optional_offset + 108).unwrap_or(0),
        _ => 0,
    };

    Some(OptionalHeader {
        magic_raw,
        entry_point_rva,
        image_base,
        subsystem,
        number_of_rva_and_sizes,
    })
}

fn parse_sections(
    bytes: &[u8],
    section_offset: usize,
    number_of_sections: u16,
    warnings: &mut Vec<String>,
) -> Vec<PeSection> {
    let mut sections = Vec::with_capacity(number_of_sections as usize);

    for index in 0..number_of_sections as usize {
        let offset = match section_offset.checked_add(index * SECTION_HEADER_SIZE) {
            Some(value) => value,
            None => {
                warnings.push("Section table offset overflowed".to_string());
                break;
            }
        };

        if offset + SECTION_HEADER_SIZE > bytes.len() {
            warnings.push(format!("Section header {index} extends beyond file size"));
            break;
        }

        let name = section_name(&bytes[offset..offset + 8]);
        sections.push(PeSection {
            name,
            virtual_size: read_u32(bytes, offset + 8).unwrap_or(0),
            virtual_address: read_u32(bytes, offset + 12).unwrap_or(0),
            raw_data_size: read_u32(bytes, offset + 16).unwrap_or(0),
            raw_data_ptr: read_u32(bytes, offset + 20).unwrap_or(0),
            characteristics: read_u32(bytes, offset + 36).unwrap_or(0),
        });
    }

    sections
}

fn parse_imported_dlls(
    bytes: &[u8],
    optional_offset: usize,
    optional_header_size: usize,
    format: &PeFormat,
    sections: &[PeSection],
    warnings: &mut Vec<String>,
) -> Vec<String> {
    let Some((import_rva, import_size)) =
        import_directory(bytes, optional_offset, optional_header_size, format)
    else {
        return Vec::new();
    };

    if import_rva == 0 || import_size == 0 {
        return Vec::new();
    }

    let Some(mut descriptor_offset) = rva_to_file_offset(import_rva, sections) else {
        warnings.push(format!(
            "Import directory RVA 0x{import_rva:08x} does not map to a section"
        ));
        return Vec::new();
    };

    let mut names = Vec::new();
    for descriptor_index in 0..MAX_IMPORT_DESCRIPTORS {
        if descriptor_offset + IMPORT_DESCRIPTOR_SIZE > bytes.len() {
            warnings.push("Import descriptor table extends beyond file size".to_string());
            break;
        }

        let original_first_thunk = read_u32(bytes, descriptor_offset).unwrap_or(0);
        let time_date_stamp = read_u32(bytes, descriptor_offset + 4).unwrap_or(0);
        let forwarder_chain = read_u32(bytes, descriptor_offset + 8).unwrap_or(0);
        let name_rva = read_u32(bytes, descriptor_offset + 12).unwrap_or(0);
        let first_thunk = read_u32(bytes, descriptor_offset + 16).unwrap_or(0);

        if original_first_thunk == 0
            && time_date_stamp == 0
            && forwarder_chain == 0
            && name_rva == 0
            && first_thunk == 0
        {
            break;
        }

        if let Some(name_offset) = rva_to_file_offset(name_rva, sections) {
            if let Some(name) = read_c_string(bytes, name_offset) {
                if !names.iter().any(|existing| existing == &name) {
                    names.push(name);
                }
            } else {
                warnings.push(format!(
                    "Import descriptor {descriptor_index} has an unterminated DLL name"
                ));
            }
        } else {
            warnings.push(format!(
                "Import descriptor {descriptor_index} DLL name RVA 0x{name_rva:08x} does not map to a section"
            ));
        }

        descriptor_offset += IMPORT_DESCRIPTOR_SIZE;
    }

    names
}

fn import_directory(
    bytes: &[u8],
    optional_offset: usize,
    optional_header_size: usize,
    format: &PeFormat,
) -> Option<(u32, u32)> {
    let data_directory_offset = match format {
        PeFormat::Pe32 => optional_offset + 96,
        PeFormat::Pe32Plus => optional_offset + 112,
        PeFormat::Unknown => return None,
    };
    let import_directory_offset =
        data_directory_offset.checked_add(IMPORT_DIRECTORY_INDEX * DATA_DIRECTORY_SIZE)?;
    let optional_end = optional_offset.checked_add(optional_header_size)?;

    if import_directory_offset + DATA_DIRECTORY_SIZE > optional_end
        || import_directory_offset + DATA_DIRECTORY_SIZE > bytes.len()
    {
        return None;
    }

    Some((
        read_u32(bytes, import_directory_offset)?,
        read_u32(bytes, import_directory_offset + 4)?,
    ))
}

fn rva_to_file_offset(rva: u32, sections: &[PeSection]) -> Option<usize> {
    for section in sections {
        let start = section.virtual_address;
        let span = section.virtual_size.max(section.raw_data_size);
        let end = start.checked_add(span)?;
        if rva >= start && rva < end {
            let relative = rva.checked_sub(start)?;
            let raw = section.raw_data_ptr.checked_add(relative)?;
            return Some(raw as usize);
        }
    }

    None
}

fn section_name(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).to_string()
}

fn read_c_string(bytes: &[u8], offset: usize) -> Option<String> {
    let tail = bytes.get(offset..)?;
    let end = tail.iter().position(|byte| *byte == 0)?;
    Some(String::from_utf8_lossy(&tail[..end]).to_string())
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let slice = bytes.get(offset..offset + 2)?;
    Some(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    let slice = bytes.get(offset..offset + 8)?;
    Some(u64::from_le_bytes([
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
    ]))
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

fn suggested_mode(
    file_name: &str,
    image_kind: &PeImageKind,
    subsystem: Option<&WindowsSubsystem>,
) -> (&'static str, &'static str) {
    let lower = file_name.to_ascii_lowercase();

    // DLL images are not runnable executables
    if *image_kind == PeImageKind::DynamicLibrary {
        return ("unsupported", "DLL / library image");
    }

    // Installer-looking filenames are always captured
    if lower.contains("setup")
        || lower.contains("install")
        || lower.contains("installer")
        || lower.contains("wizard")
        || lower.contains("bootstrapper")
    {
        return ("capture-install", "installer-looking filename");
    }

    // Console executables are run-once / portable
    if subsystem == Some(&WindowsSubsystem::WindowsCui) {
        return ("run-once", "console executable");
    }

    // GUI executable without installer-looking filename is a portable/tool candidate
    if subsystem == Some(&WindowsSubsystem::WindowsGui) {
        return ("run-once", "portable/tool candidate");
    }

    // Fallback
    ("run-once", "Windows executable fallback")
}
