use crate::output;
use openntx_core::pe::analyze_pe;
use openntx_core::Result;
use std::path::Path;

pub fn run(file: &Path) -> Result<()> {
    let analysis = analyze_pe(file)?;

    output::title("OpenNTX Analyze");
    output::field("File", &analysis.file_name);
    output::field("Format", &analysis.format);
    output::field("Architecture", &analysis.architecture);
    output::field("Type", &analysis.image_kind);
    if let Some(dos_header) = &analysis.dos_header {
        output::field("DOS e_lfanew", format!("0x{:08x}", dos_header.e_lfanew));
    }
    if let Some(coff_header) = &analysis.coff_header {
        output::field("COFF machine", format!("0x{:04x}", coff_header.machine_raw));
        output::field("COFF sections", coff_header.number_of_sections);
        output::field(
            "COFF characteristics",
            format!("0x{:04x}", coff_header.characteristics),
        );
    }
    if let Some(optional_header) = &analysis.optional_header {
        output::field(
            "Optional header magic",
            format!("0x{:04x}", optional_header.magic_raw),
        );
        output::field(
            "Entry point RVA",
            format!("0x{:08x}", optional_header.entry_point_rva),
        );
        output::field(
            "Image base",
            format!("0x{:016x}", optional_header.image_base),
        );
        output::field("Data directories", optional_header.number_of_rva_and_sizes);
    }
    if let Some(subsystem) = &analysis.subsystem {
        output::field("Subsystem", subsystem);
    }
    if analysis.sections.is_empty() {
        output::field("Sections", "none");
    } else {
        output::field("Sections", analysis.sections.len());
        for section in &analysis.sections {
            output::field(
                "Section",
                format!(
                    "{} va=0x{:08x} vsz=0x{:08x} raw=0x{:08x}+0x{:08x}",
                    section.name,
                    section.virtual_address,
                    section.virtual_size,
                    section.raw_data_ptr,
                    section.raw_data_size
                ),
            );
        }
    }
    if analysis.imported_dlls.is_empty() {
        output::field("Imported DLLs", "none");
    } else {
        output::field("Imported DLLs", analysis.imported_dlls.join(", "));
    }
    output::field("Suggested mode", &analysis.suggested_mode);
    output::field("Mode reason", &analysis.install_mode_reason);
    output::field("Status", &analysis.status);

    for warning in &analysis.warnings {
        output::field("Warning", warning);
    }

    output::blank();
    output::note(
        "Runtime execution is not implemented. This command is analysis-only and prepares metadata for future OpenNTX install flows.",
    );
    Ok(())
}
