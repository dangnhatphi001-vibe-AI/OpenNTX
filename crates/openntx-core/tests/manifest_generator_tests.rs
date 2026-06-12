use openntx_core::app_id::generate_app_id;
use openntx_core::manifest::{
    generate_manifest_from_pe, validate_manifest, AppManifest, ManifestGenerationInput,
};
use openntx_core::pe::analyze_pe;
use std::fs;
use std::path::{Path, PathBuf};
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
fn generates_manifest_from_x86_pe32_console_fixture() {
    let path = write_fixture(
        "console-tool.exe",
        PeFixture {
            machine: 0x014c,
            magic: 0x010b,
            subsystem: 3,
            characteristics: 0x0002,
            image_base: 0x0040_0000,
            imports: &["KERNEL32.dll"],
        },
    );

    let manifest = generate_for_path(&path);
    assert_eq!(manifest.architecture, "x86");
    assert_eq!(manifest.install_mode, "portable");
    assert_eq!(manifest.source.source_type, "portable");
    assert_eq!(manifest.diagnostics.imported_dlls, vec!["KERNEL32.dll"]);
    assert_eq!(
        manifest.diagnostics.entry_point_rva.as_deref(),
        Some("0x00001010")
    );
    assert_eq!(
        manifest.diagnostics.image_base.as_deref(),
        Some("0x0000000000400000")
    );
    validate_manifest(&manifest).expect("generated manifest should validate");

    let _ = fs::remove_file(path);
}

#[test]
fn generates_manifest_from_x86_64_pe32_plus_gui_fixture() {
    let path = write_fixture(
        "desktop-app.exe",
        PeFixture {
            machine: 0x8664,
            magic: 0x020b,
            subsystem: 2,
            characteristics: 0x0002,
            image_base: 0x0000_0001_4000_0000,
            imports: &["KERNEL32.dll", "USER32.dll"],
        },
    );

    let manifest = generate_for_path(&path);
    assert_eq!(manifest.architecture, "x86_64");
    assert_eq!(manifest.install_mode, "captured");
    assert_eq!(manifest.source.source_type, "installer");
    assert_eq!(
        manifest.diagnostics.imported_dlls,
        vec!["KERNEL32.dll", "USER32.dll"]
    );
    validate_manifest(&manifest).expect("generated manifest should validate");

    let _ = fs::remove_file(path);
}

#[test]
fn installer_looking_filename_suggests_captured_mode() {
    let path = write_fixture(
        "TLauncher-Installer-1.9.5.1.exe",
        PeFixture {
            machine: 0x014c,
            magic: 0x010b,
            subsystem: 3,
            characteristics: 0x0002,
            image_base: 0x0040_0000,
            imports: &[],
        },
    );

    let manifest = generate_for_path(&path);
    assert!(manifest.app_id.starts_with("tlauncher-installer-1-9-5-1-"));
    assert_eq!(manifest.install_mode, "captured");
    assert_eq!(manifest.source.source_type, "installer");

    let _ = fs::remove_file(path);
}

#[test]
fn invalid_file_fails_gracefully() {
    let path = temp_path("invalid.exe");
    fs::write(&path, b"not a PE").expect("write invalid fixture");
    let analysis = analyze_pe(&path).expect("invalid file should analyze as not-pe");
    let app_id = generate_app_id("invalid", Some(&path.display().to_string()));

    let error = generate_manifest_from_pe(ManifestGenerationInput {
        input_path: &path,
        analysis: &analysis,
        app_id,
    })
    .expect_err("invalid PE should not generate a manifest");

    assert!(error.to_string().contains("requires a valid PE file"));

    let _ = fs::remove_file(path);
}

#[test]
fn generated_manifest_json_is_valid() {
    let path = write_fixture(
        "json-app.exe",
        PeFixture {
            machine: 0x8664,
            magic: 0x020b,
            subsystem: 2,
            characteristics: 0x0002,
            image_base: 0x0000_0001_4000_0000,
            imports: &["KERNEL32.dll"],
        },
    );

    let manifest = generate_for_path(&path);
    let json = serde_json::to_string_pretty(&manifest).expect("manifest should serialize");
    let decoded: AppManifest = serde_json::from_str(&json).expect("manifest JSON should parse");
    validate_manifest(&decoded).expect("manifest JSON should validate after parsing");

    let _ = fs::remove_file(path);
}

fn generate_for_path(path: &Path) -> AppManifest {
    let analysis = analyze_pe(path).expect("PE fixture should analyze");
    let display_name = path.file_stem().and_then(|value| value.to_str()).unwrap();
    let app_id = generate_app_id(display_name, Some(&path.display().to_string()));
    generate_manifest_from_pe(ManifestGenerationInput {
        input_path: path,
        analysis: &analysis,
        app_id,
    })
    .expect("manifest should generate")
    .manifest
}

fn write_fixture(name: &str, config: PeFixture<'_>) -> PathBuf {
    let path = temp_path(name);
    fs::write(&path, fixture(config)).expect("write PE fixture");
    path
}

fn temp_path(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("openntx-{unique}"));
    fs::create_dir_all(&dir).expect("create temp fixture directory");
    dir.join(name)
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
