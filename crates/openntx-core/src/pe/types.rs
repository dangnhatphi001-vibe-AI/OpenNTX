use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeFormat {
    Pe32,
    Pe32Plus,
    Unknown,
}

impl fmt::Display for PeFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pe32 => write!(f, "PE32 executable"),
            Self::Pe32Plus => write!(f, "PE32+ executable"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeArchitecture {
    X86,
    X86_64,
    Arm,
    Arm64,
    Unknown(u16),
}

impl fmt::Display for PeArchitecture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::X86 => write!(f, "x86"),
            Self::X86_64 => write!(f, "x86_64"),
            Self::Arm => write!(f, "arm"),
            Self::Arm64 => write!(f, "arm64"),
            Self::Unknown(value) => write!(f, "unknown(0x{value:04x})"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeImageKind {
    Executable,
    DynamicLibrary,
    Unknown,
}

impl fmt::Display for PeImageKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Executable => write!(f, "Windows executable"),
            Self::DynamicLibrary => write!(f, "Windows DLL"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowsSubsystem {
    Native,
    WindowsGui,
    WindowsCui,
    Unknown(u16),
}

impl fmt::Display for WindowsSubsystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Native => write!(f, "native"),
            Self::WindowsGui => write!(f, "windows-gui"),
            Self::WindowsCui => write!(f, "windows-console"),
            Self::Unknown(value) => write!(f, "unknown(0x{value:04x})"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeAnalysis {
    pub path: PathBuf,
    pub file_name: String,
    pub is_pe: bool,
    pub format: PeFormat,
    pub architecture: PeArchitecture,
    pub image_kind: PeImageKind,
    pub subsystem: Option<WindowsSubsystem>,
    pub dos_header: Option<DosHeader>,
    pub coff_header: Option<CoffHeader>,
    pub optional_header: Option<OptionalHeader>,
    pub sections: Vec<PeSection>,
    pub imported_dlls: Vec<String>,
    pub suggested_mode: String,
    pub status: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DosHeader {
    pub e_lfanew: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoffHeader {
    pub machine_raw: u16,
    pub number_of_sections: u16,
    pub time_date_stamp: u32,
    pub pointer_to_symbol_table: u32,
    pub number_of_symbols: u32,
    pub size_of_optional_header: u16,
    pub characteristics: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionalHeader {
    pub magic_raw: u16,
    pub entry_point_rva: u32,
    pub image_base: u64,
    pub subsystem: WindowsSubsystem,
    pub number_of_rva_and_sizes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeSection {
    pub name: String,
    pub virtual_size: u32,
    pub virtual_address: u32,
    pub raw_data_size: u32,
    pub raw_data_ptr: u32,
    pub characteristics: u32,
}
