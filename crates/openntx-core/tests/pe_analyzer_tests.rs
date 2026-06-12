use openntx_core::pe::{analyze_pe, PeArchitecture, PeFormat, PeImageKind, WindowsSubsystem};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const PE_OFFSET: usize = 0x80;
const OPTIONAL_OFFSET: usize = PE_OFFSET + 24;
const SECTION_OFFSET_PE32: usize = OPTIONAL_OFFSET + 0xe0;
const SECTION_OFFSET_PE64: usize = OPTIONAL_OFFSET + 0xf0;
const TEXT_RVA: u32 = 0x1000;
const TEXT_RAW: u32 = 0x200;
const IMPORT_RVA: u32 = 0x1100;
const IMPORT_RAW: u32 = 0x300;
const IMPORT_NAME_RVA: u32 = 0x1140;
const IMPORT_NAME_RAW: u32 = 0x340;

#[test]
fn nonexistent_file_is_graceful_error() {
    let path = temp_path("missing.exe");
    let error = analyze_pe(&path).expect_err("missing file should fail gracefully");
    assert!(error.to_string().contains("does not exist"));
}

#[test]
fn invalid_file_is_reported_as_not_pe() {
    let path = temp_path("invalid.bin");
    fs::write(&path, b"not a pe file").expect("write test fixture");

    let analysis = analyze_pe(&path).expect("invalid file should analyze");
    assert!(!analysis.is_pe);
    assert_eq!(analysis.status, "not-pe");
    assert_eq!(analysis.sections.len(), 0);
    assert_eq!(analysis.imported_dlls.len(), 0);
    assert!(!analysis.warnings.is_empty());

    let _ = fs::remove_file(path);
}

#[test]
fn mz_only_file_is_reported_as_not_pe() {
    let path = temp_path("mz-only.exe");
    let mut bytes = vec![0_u8; 128];
    bytes[0..2].copy_from_slice(b"MZ");
    write_u32(&mut bytes, 0x3c, 0x80);
    fs::write(&path, bytes).expect("write MZ-only fixture");

    let analysis = analyze_pe(&path).expect("MZ-only file should analyze");
    assert!(!analysis.is_pe);
    assert_eq!(analysis.status, "not-pe");
    assert!(analysis
        .warnings
        .iter()
        .any(|warning| warning.contains("PE header offset")));

    let _ = fs::remove_file(path);
}

#[test]
fn pe32_x86_console_metadata_is_detected() {
    let path = temp_path("console-tool.exe");
    fs::write(
        &path,
        fixture(PeFixture {
            machine: 0x014c,
            magic: 0x010b,
            subsystem: 3,
            characteristics: 0x0002,
            image_base: 0x0040_0000,
            imports: &["KERNEL32.dll"],
        }),
    )
    .expect("write PE fixture");

    let analysis = analyze_pe(&path).expect("PE fixture should analyze");
    assert!(analysis.is_pe);
    assert_eq!(analysis.format, PeFormat::Pe32);
    assert_eq!(analysis.architecture, PeArchitecture::X86);
    assert_eq!(analysis.subsystem, Some(WindowsSubsystem::WindowsCui));
    assert_eq!(analysis.image_kind, PeImageKind::Executable);
    assert_eq!(analysis.suggested_mode, "run-once");

    let dos = analysis.dos_header.expect("DOS header");
    assert_eq!(dos.e_lfanew, PE_OFFSET as u32);

    let coff = analysis.coff_header.expect("COFF header");
    assert_eq!(coff.machine_raw, 0x014c);
    assert_eq!(coff.number_of_sections, 1);
    assert_eq!(coff.size_of_optional_header, 0xe0);

    let optional = analysis.optional_header.expect("optional header");
    assert_eq!(optional.magic_raw, 0x010b);
    assert_eq!(optional.entry_point_rva, TEXT_RVA + 0x10);
    assert_eq!(optional.image_base, 0x0040_0000);
    assert_eq!(optional.subsystem, WindowsSubsystem::WindowsCui);
    assert_eq!(optional.number_of_rva_and_sizes, 16);

    assert_eq!(analysis.sections.len(), 1);
    assert_eq!(analysis.sections[0].name, ".text");
    assert_eq!(analysis.imported_dlls, vec!["KERNEL32.dll"]);

    let _ = fs::remove_file(path);
}

#[test]
fn pe64_x86_64_gui_metadata_and_imports_are_detected() {
    let path = temp_path("setup.exe");
    fs::write(
        &path,
        fixture(PeFixture {
            machine: 0x8664,
            magic: 0x020b,
            subsystem: 2,
            characteristics: 0x0002,
            image_base: 0x0000_0001_4000_0000,
            imports: &["KERNEL32.dll", "USER32.dll"],
        }),
    )
    .expect("write PE fixture");

    let analysis = analyze_pe(&path).expect("PE fixture should analyze");
    assert!(analysis.is_pe);
    assert_eq!(analysis.format, PeFormat::Pe32Plus);
    assert_eq!(analysis.architecture, PeArchitecture::X86_64);
    assert_eq!(analysis.subsystem, Some(WindowsSubsystem::WindowsGui));
    assert_eq!(analysis.suggested_mode, "capture-install");

    let optional = analysis.optional_header.expect("optional header");
    assert_eq!(optional.magic_raw, 0x020b);
    assert_eq!(optional.entry_point_rva, TEXT_RVA + 0x10);
    assert_eq!(optional.image_base, 0x0000_0001_4000_0000);

    assert_eq!(analysis.sections.len(), 1);
    assert_eq!(analysis.sections[0].virtual_address, TEXT_RVA);
    assert_eq!(analysis.sections[0].raw_data_ptr, TEXT_RAW);
    assert_eq!(analysis.imported_dlls, vec!["KERNEL32.dll", "USER32.dll"]);

    let _ = fs::remove_file(path);
}

#[test]
fn dll_image_kind_is_detected_from_coff_characteristics() {
    let path = temp_path("library.dll");
    fs::write(
        &path,
        fixture(PeFixture {
            machine: 0x8664,
            magic: 0x020b,
            subsystem: 2,
            characteristics: 0x2000,
            image_base: 0x0000_0001_8000_0000,
            imports: &[],
        }),
    )
    .expect("write DLL fixture");

    let analysis = analyze_pe(&path).expect("DLL fixture should analyze");
    assert_eq!(analysis.image_kind, PeImageKind::DynamicLibrary);
    assert!(analysis.imported_dlls.is_empty());

    let _ = fs::remove_file(path);
}

fn temp_path(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("openntx-{unique}-{name}"))
}

struct PeFixture<'a> {
    machine: u16,
    magic: u16,
    subsystem: u16,
    characteristics: u16,
    image_base: u64,
    imports: &'a [&'a str],
}

fn fixture(config: PeFixture<'_>) -> Vec<u8> {
    let optional_size = if config.magic == 0x010b { 0xe0 } else { 0xf0 };
    let section_offset = if config.magic == 0x010b {
        SECTION_OFFSET_PE32
    } else {
        SECTION_OFFSET_PE64
    };
    let mut bytes = vec![0_u8; 0x800];

    bytes[0..2].copy_from_slice(b"MZ");
    write_u32(&mut bytes, 0x3c, PE_OFFSET as u32);
    bytes[PE_OFFSET..PE_OFFSET + 4].copy_from_slice(b"PE\0\0");

    write_u16(&mut bytes, PE_OFFSET + 4, config.machine);
    write_u16(&mut bytes, PE_OFFSET + 6, 1);
    write_u16(&mut bytes, PE_OFFSET + 20, optional_size);
    write_u16(&mut bytes, PE_OFFSET + 22, config.characteristics);

    write_u16(&mut bytes, OPTIONAL_OFFSET, config.magic);
    write_u32(&mut bytes, OPTIONAL_OFFSET + 16, TEXT_RVA + 0x10);
    if config.magic == 0x010b {
        write_u32(&mut bytes, OPTIONAL_OFFSET + 28, config.image_base as u32);
        write_u32(&mut bytes, OPTIONAL_OFFSET + 92, 16);
        write_import_directory(&mut bytes, OPTIONAL_OFFSET + 96, config.imports);
    } else {
        write_u64(&mut bytes, OPTIONAL_OFFSET + 24, config.image_base);
        write_u32(&mut bytes, OPTIONAL_OFFSET + 108, 16);
        write_import_directory(&mut bytes, OPTIONAL_OFFSET + 112, config.imports);
    }
    write_u16(&mut bytes, OPTIONAL_OFFSET + 68, config.subsystem);

    write_section(&mut bytes, section_offset);
    write_imports(&mut bytes, config.imports);

    bytes
}

fn write_import_directory(bytes: &mut [u8], data_directory_offset: usize, imports: &[&str]) {
    if imports.is_empty() {
        return;
    }

    let import_directory_offset = data_directory_offset + 8;
    write_u32(bytes, import_directory_offset, IMPORT_RVA);
    write_u32(
        bytes,
        import_directory_offset + 4,
        ((imports.len() + 1) * 20) as u32,
    );
}

fn write_section(bytes: &mut [u8], offset: usize) {
    let mut name = [0_u8; 8];
    name[..5].copy_from_slice(b".text");
    bytes[offset..offset + 8].copy_from_slice(&name);
    write_u32(bytes, offset + 8, 0x400);
    write_u32(bytes, offset + 12, TEXT_RVA);
    write_u32(bytes, offset + 16, 0x400);
    write_u32(bytes, offset + 20, TEXT_RAW);
    write_u32(bytes, offset + 36, 0x6000_0020);
}

fn write_imports(bytes: &mut [u8], imports: &[&str]) {
    let mut next_name_raw = IMPORT_NAME_RAW as usize;
    let mut next_name_rva = IMPORT_NAME_RVA;

    for (index, dll_name) in imports.iter().enumerate() {
        let descriptor_offset = IMPORT_RAW as usize + index * 20;
        write_u32(bytes, descriptor_offset, 0);
        write_u32(bytes, descriptor_offset + 12, next_name_rva);
        write_u32(
            bytes,
            descriptor_offset + 16,
            TEXT_RVA + 0x280 + (index as u32 * 8),
        );

        let name_bytes = dll_name.as_bytes();
        bytes[next_name_raw..next_name_raw + name_bytes.len()].copy_from_slice(name_bytes);
        bytes[next_name_raw + name_bytes.len()] = 0;

        let aligned_len = (name_bytes.len() + 1 + 3) & !3;
        next_name_raw += aligned_len;
        next_name_rva += aligned_len as u32;
    }
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
