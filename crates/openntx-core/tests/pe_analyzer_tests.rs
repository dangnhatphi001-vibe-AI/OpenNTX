use openntx_core::pe::{analyze_pe, PeArchitecture, PeFormat};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

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
    assert!(!analysis.warnings.is_empty());

    let _ = fs::remove_file(path);
}

#[test]
fn minimal_pe_header_is_detected() {
    let path = temp_path("setup.exe");
    fs::write(&path, minimal_pe64()).expect("write PE fixture");

    let analysis = analyze_pe(&path).expect("PE fixture should analyze");
    assert!(analysis.is_pe);
    assert_eq!(analysis.format, PeFormat::Pe32Plus);
    assert_eq!(analysis.architecture, PeArchitecture::X86_64);
    assert_eq!(analysis.suggested_mode, "capture-install");

    let _ = fs::remove_file(path);
}

fn temp_path(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("openntx-{unique}-{name}"))
}

fn minimal_pe64() -> Vec<u8> {
    let mut bytes = vec![0_u8; 512];
    bytes[0] = b'M';
    bytes[1] = b'Z';
    write_u32(&mut bytes, 0x3c, 0x80);
    bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
    write_u16(&mut bytes, 0x84, 0x8664);
    write_u16(&mut bytes, 0x94, 0x00f0);
    write_u16(&mut bytes, 0x96, 0x0002);
    write_u16(&mut bytes, 0x98, 0x020b);
    write_u16(&mut bytes, 0x98 + 68, 2);
    bytes
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
