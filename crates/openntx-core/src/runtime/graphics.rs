// openntx-core/src/runtime/graphics.rs — Virtual window surface & GDI stub.
//
// Provides `WindowStubManager` which allocates virtual window handles (HWND)
// for sandboxed Windows applications and manages framebuffer memory for
// headless or X11-connected rendering.
//
// Architecture:
//
//   Windows App (GDI/DirectX)
//         │
//         ▼
//   WindowStubManager::create_headless_surface()
//         │
//         ├─ X11 mode:  connect via DISPLAY env var, create off-screen pixmap
//         └─ Headless:   allocate memory-mapped framebuffer file
//
//   WindowStubManager::map_gdi_flush()
//         │
//         └─ Copy raw pixel data into the isolated framebuffer
//
// The X11 integration is optional and only activated when the `DISPLAY`
// environment variable is set.  In headless mode (CI, server, no display),
// a memory-mapped file is used as the render target.

use crate::{OpenNtxError, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Default directory for headless framebuffer files.
const FRAMEBUFFER_DIR: &str = "/tmp/openntx-graphics";

/// Magic HWND base value (Windows HWNDs are pointer-sized integers).
const HWND_BASE: u64 = 0x000A_0000;

/// Atomic counter for generating unique surface IDs.
static SURFACE_COUNTER: AtomicU64 = AtomicU64::new(1);

// ── Public types ─────────────────────────────────────────────────────────────

/// Metadata for a single virtual window surface.
#[derive(Debug, Clone)]
pub struct SurfaceInfo {
    /// Unique surface identifier (virtual HWND).
    pub surface_id: u64,
    /// Application that owns this surface.
    pub app_id: String,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Byte size of the RGBA framebuffer (width × height × 4).
    pub buffer_size: usize,
    /// Path to the memory-mapped framebuffer file (headless mode).
    pub framebuffer_path: Option<PathBuf>,
    /// Whether this surface is connected to an X11 display.
    pub x11_connected: bool,
}

/// Virtual window surface manager for sandboxed Windows applications.
///
/// Allocates virtual HWNDs, manages framebuffer memory, and provides
/// a GDI flush interface for pixel data from the Windows emulation layer.
///
/// # Examples
///
/// ```no_run
/// use openntx_core::runtime::graphics::WindowStubManager;
///
/// let mut mgr = WindowStubManager::new();
/// let surface = mgr.create_headless_surface("notepadpp-a3f2", 800, 600).unwrap();
/// let pixels = vec![0u8; 800 * 600 * 4]; // RGBA
/// mgr.map_gdi_flush(surface, &pixels).unwrap();
/// ```
pub struct WindowStubManager {
    /// Base directory for framebuffer files.
    framebuffer_dir: PathBuf,
    /// Map of active surfaces by surface_id.
    surfaces: HashMap<u64, SurfaceInfo>,
    /// Whether X11 display is available.
    x11_available: bool,
}

impl WindowStubManager {
    /// Create a new `WindowStubManager` with default settings.
    ///
    /// Automatically detects X11 availability via the `DISPLAY` environment
    /// variable.  Falls back to headless mode if `DISPLAY` is not set.
    pub fn new() -> Self {
        let framebuffer_dir = PathBuf::from(FRAMEBUFFER_DIR);
        let x11_available = std::env::var("DISPLAY").is_ok();

        Self {
            framebuffer_dir,
            surfaces: HashMap::new(),
            x11_available,
        }
    }

    /// Create a new `WindowStubManager` with a custom framebuffer directory.
    pub fn with_framebuffer_dir(framebuffer_dir: PathBuf) -> Self {
        let x11_available = std::env::var("DISPLAY").is_ok();

        Self {
            framebuffer_dir,
            surfaces: HashMap::new(),
            x11_available,
        }
    }

    /// Return whether X11 display is available.
    pub fn x11_available(&self) -> bool {
        self.x11_available
    }

    /// Return the framebuffer directory.
    pub fn framebuffer_dir(&self) -> &Path {
        &self.framebuffer_dir
    }

    /// Return the number of active surfaces.
    pub fn surface_count(&self) -> usize {
        self.surfaces.len()
    }

    /// Return metadata for a surface by ID.
    pub fn get_surface(&self, surface_id: u64) -> Option<&SurfaceInfo> {
        self.surfaces.get(&surface_id)
    }

    // ── Public API ───────────────────────────────────────────────────────────

    /// Create a headless virtual window surface for an application.
    ///
    /// Allocates a unique surface ID (virtual HWND) and prepares either:
    /// - **X11 mode:** Validates the display connection (actual pixmap
    ///   creation would require linking libX11, so we record the intent).
    /// - **Headless mode:** Creates a memory-mapped framebuffer file at
    ///   `/tmp/openntx-graphics/<app_id>/<surface_id>.rgba`.
    ///
    /// # Parameters
    ///
    /// - `app_id` — Application identifier (used in framebuffer path).
    /// - `width` — Surface width in pixels.
    /// - `height` — Surface height in pixels.
    ///
    /// # Returns
    ///
    /// The surface ID (virtual HWND) on success.
    ///
    /// # Errors
    ///
    /// - `GraphicsContextCreationFailed` if the framebuffer directory cannot
    ///   be created or the framebuffer file cannot be allocated.
    /// - `InvalidInput` if width or height is zero.
    pub fn create_headless_surface(
        &mut self,
        app_id: &str,
        width: u32,
        height: u32,
    ) -> Result<u64> {
        if width == 0 || height == 0 {
            return Err(OpenNtxError::InvalidInput(format!(
                "surface dimensions must be non-zero (got {}×{})",
                width, height
            )));
        }

        let surface_id = HWND_BASE + SURFACE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let buffer_size = (width as usize)
            .checked_mul(height as usize)
            .and_then(|v| v.checked_mul(4)) // RGBA
            .ok_or_else(|| {
                OpenNtxError::GraphicsContextCreationFailed(format!(
                    "framebuffer size overflow for {}×{}",
                    width, height
                ))
            })?;

        let mut framebuffer_path = None;

        if !self.x11_available {
            // Headless mode: allocate memory-mapped framebuffer file.
            let app_dir = self.framebuffer_dir.join(app_id);
            fs::create_dir_all(&app_dir).map_err(|source| {
                OpenNtxError::GraphicsContextCreationFailed(format!(
                    "failed to create framebuffer directory {}: {}",
                    app_dir.display(),
                    source
                ))
            })?;

            let fb_path = app_dir.join(format!("{}.rgba", surface_id));

            // Pre-allocate the framebuffer file with zeros.
            let file = fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&fb_path)
                .map_err(|source| {
                    OpenNtxError::GraphicsContextCreationFailed(format!(
                        "failed to create framebuffer file {}: {}",
                        fb_path.display(),
                        source
                    ))
                })?;

            // Set the file size to the required buffer size.
            file.set_len(buffer_size as u64).map_err(|source| {
                OpenNtxError::GraphicsContextCreationFailed(format!(
                    "failed to allocate {} bytes for framebuffer: {}",
                    buffer_size, source
                ))
            })?;

            framebuffer_path = Some(fb_path);
        }

        let info = SurfaceInfo {
            surface_id,
            app_id: app_id.to_string(),
            width,
            height,
            buffer_size,
            framebuffer_path,
            x11_connected: self.x11_available,
        };

        self.surfaces.insert(surface_id, info);

        Ok(surface_id)
    }

    /// Flush raw RGBA pixel data into the surface's isolated framebuffer.
    ///
    /// In headless mode, writes the pixel data directly to the
    /// memory-mapped file.  In X11 mode, records the flush event
    /// (actual X11 rendering requires libX11 linkage).
    ///
    /// # Parameters
    ///
    /// - `surface_id` — The surface ID returned by `create_headless_surface`.
    /// - `raw_pixels` — Raw RGBA pixel data.  Length must match the surface's
    ///   expected buffer size (width × height × 4).
    ///
    /// # Errors
    ///
    /// - `InvalidInput` if the surface ID is unknown or pixel data length
    ///   does not match.
    /// - `GraphicsContextCreationFailed` if the framebuffer write fails.
    pub fn map_gdi_flush(&self, surface_id: u64, raw_pixels: &[u8]) -> Result<()> {
        let info = self.surfaces.get(&surface_id).ok_or_else(|| {
            OpenNtxError::InvalidInput(format!("unknown surface ID: {}", surface_id))
        })?;

        if raw_pixels.len() != info.buffer_size {
            return Err(OpenNtxError::InvalidInput(format!(
                "pixel data length mismatch: expected {} bytes for {}×{} surface, got {}",
                info.buffer_size,
                info.width,
                info.height,
                raw_pixels.len()
            )));
        }

        if let Some(ref fb_path) = info.framebuffer_path {
            // Headless mode: write pixels to framebuffer file.
            fs::write(fb_path, raw_pixels).map_err(|source| {
                OpenNtxError::GraphicsContextCreationFailed(format!(
                    "failed to flush pixels to framebuffer {}: {}",
                    fb_path.display(),
                    source
                ))
            })?;
        }
        // X11 mode: in a full implementation, this would call XPutImage
        // or similar.  For now, the flush is a no-op (the pixel data is
        // accepted but not rendered to a physical display).

        Ok(())
    }

    /// Destroy a surface and release its resources.
    ///
    /// Removes the framebuffer file (if headless) and frees the surface
    /// metadata.
    pub fn destroy_surface(&mut self, surface_id: u64) -> Result<()> {
        let info = self.surfaces.remove(&surface_id).ok_or_else(|| {
            OpenNtxError::InvalidInput(format!("unknown surface ID: {}", surface_id))
        })?;

        if let Some(ref fb_path) = info.framebuffer_path {
            if fb_path.exists() {
                fs::remove_file(fb_path).map_err(|source| {
                    OpenNtxError::GraphicsContextCreationFailed(format!(
                        "failed to remove framebuffer file {}: {}",
                        fb_path.display(),
                        source
                    ))
                })?;
            }
        }

        Ok(())
    }

    /// Destroy all surfaces for a given application.
    pub fn destroy_app_surfaces(&mut self, app_id: &str) -> Result<u32> {
        let surface_ids: Vec<u64> = self
            .surfaces
            .iter()
            .filter(|(_, info)| info.app_id == app_id)
            .map(|(&id, _)| id)
            .collect();

        let count = surface_ids.len() as u32;
        for sid in surface_ids {
            self.destroy_surface(sid)?;
        }

        Ok(count)
    }
}

impl Default for WindowStubManager {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_mgr() -> WindowStubManager {
        let dir = tempfile::tempdir().expect("temp dir");
        // Leak the dir so it persists for the test.
        let path = dir.path().to_path_buf();
        std::mem::forget(dir);

        // Temporarily unset DISPLAY to force headless mode.
        let old_display = std::env::var("DISPLAY").ok();
        std::env::remove_var("DISPLAY");
        let mgr = WindowStubManager::with_framebuffer_dir(path);
        // Restore DISPLAY.
        if let Some(val) = old_display {
            std::env::set_var("DISPLAY", val);
        }
        mgr
    }

    // ── Surface creation ─────────────────────────────────────────────────

    #[test]
    fn create_surface_returns_unique_ids() {
        let mut mgr = temp_mgr();
        let id1 = mgr.create_headless_surface("app1", 320, 240).unwrap();
        let id2 = mgr.create_headless_surface("app1", 640, 480).unwrap();
        assert_ne!(id1, id2);
        assert_eq!(mgr.surface_count(), 2);
    }

    #[test]
    fn create_surface_id_starts_above_base() {
        let mut mgr = temp_mgr();
        let id = mgr.create_headless_surface("test", 100, 100).unwrap();
        assert!(
            id >= HWND_BASE,
            "surface ID {} < HWND_BASE {}",
            id,
            HWND_BASE
        );
    }

    #[test]
    fn create_surface_rejects_zero_width() {
        let mut mgr = temp_mgr();
        let result = mgr.create_headless_surface("test", 0, 100);
        assert!(result.is_err());
        match result.unwrap_err() {
            OpenNtxError::InvalidInput(_) => {}
            other => panic!("expected InvalidInput, got: {:?}", other),
        }
    }

    #[test]
    fn create_surface_rejects_zero_height() {
        let mut mgr = temp_mgr();
        let result = mgr.create_headless_surface("test", 100, 0);
        assert!(result.is_err());
    }

    #[test]
    fn create_surface_rejects_both_zero() {
        let mut mgr = temp_mgr();
        let result = mgr.create_headless_surface("test", 0, 0);
        assert!(result.is_err());
    }

    #[test]
    fn create_surface_stores_metadata() {
        let mut mgr = temp_mgr();
        let id = mgr.create_headless_surface("my-app", 800, 600).unwrap();
        let info = mgr.get_surface(id).unwrap();
        assert_eq!(info.app_id, "my-app");
        assert_eq!(info.width, 800);
        assert_eq!(info.height, 600);
        assert_eq!(info.buffer_size, 800 * 600 * 4);
        assert!(info.framebuffer_path.is_some());
    }

    #[test]
    fn create_surface_headless_creates_file() {
        let mut mgr = temp_mgr();
        let id = mgr.create_headless_surface("test", 64, 64).unwrap();
        let info = mgr.get_surface(id).unwrap();
        let fb_path = info.framebuffer_path.as_ref().unwrap();
        assert!(fb_path.exists(), "framebuffer file should exist");

        let metadata = fs::metadata(fb_path).unwrap();
        assert_eq!(metadata.len(), 64 * 64 * 4);
    }

    #[test]
    fn create_surface_headless_file_path_format() {
        let mut mgr = temp_mgr();
        let id = mgr.create_headless_surface("my-app", 32, 32).unwrap();
        let info = mgr.get_surface(id).unwrap();
        let fb_path = info.framebuffer_path.as_ref().unwrap();
        let path_str = fb_path.display().to_string();
        assert!(
            path_str.contains("my-app"),
            "path should contain app_id: {}",
            path_str
        );
        assert!(
            path_str.ends_with(".rgba"),
            "path should end with .rgba: {}",
            path_str
        );
    }

    // ── GDI flush ────────────────────────────────────────────────────────

    #[test]
    fn map_gdi_flush_writes_pixels() {
        let mut mgr = temp_mgr();
        let id = mgr.create_headless_surface("test", 4, 4).unwrap();
        let pixels = vec![0xABu8; 4 * 4 * 4]; // 64 bytes of RGBA
        mgr.map_gdi_flush(id, &pixels).unwrap();

        // Verify the file contains the written data.
        let info = mgr.get_surface(id).unwrap();
        let fb_path = info.framebuffer_path.as_ref().unwrap();
        let written = fs::read(fb_path).unwrap();
        assert_eq!(written, pixels);
    }

    #[test]
    fn map_gdi_flush_rejects_unknown_surface() {
        let mgr = temp_mgr();
        let pixels = vec![0u8; 100];
        let result = mgr.map_gdi_flush(99999, &pixels);
        assert!(result.is_err());
        match result.unwrap_err() {
            OpenNtxError::InvalidInput(msg) => {
                assert!(msg.contains("unknown surface"), "msg: {}", msg);
            }
            other => panic!("expected InvalidInput, got: {:?}", other),
        }
    }

    #[test]
    fn map_gdi_flush_rejects_wrong_size() {
        let mut mgr = temp_mgr();
        let id = mgr.create_headless_surface("test", 10, 10).unwrap();
        let wrong_pixels = vec![0u8; 100]; // should be 10*10*4 = 400
        let result = mgr.map_gdi_flush(id, &wrong_pixels);
        assert!(result.is_err());
        match result.unwrap_err() {
            OpenNtxError::InvalidInput(msg) => {
                assert!(msg.contains("mismatch"), "msg: {}", msg);
            }
            other => panic!("expected InvalidInput, got: {:?}", other),
        }
    }

    #[test]
    fn map_gdi_flush_preserves_pixel_data() {
        let mut mgr = temp_mgr();
        let id = mgr.create_headless_surface("test", 2, 2).unwrap();

        // Create a pattern: each pixel is [R, G, B, A].
        let mut pixels = Vec::with_capacity(2 * 2 * 4);
        pixels.extend_from_slice(&[255, 0, 0, 255]); // red
        pixels.extend_from_slice(&[0, 255, 0, 255]); // green
        pixels.extend_from_slice(&[0, 0, 255, 255]); // blue
        pixels.extend_from_slice(&[255, 255, 255, 0]); // transparent white

        mgr.map_gdi_flush(id, &pixels).unwrap();

        let info = mgr.get_surface(id).unwrap();
        let written = fs::read(info.framebuffer_path.as_ref().unwrap()).unwrap();
        assert_eq!(written[0], 255); // R
        assert_eq!(written[1], 0); // G
        assert_eq!(written[2], 0); // B
        assert_eq!(written[3], 255); // A
        assert_eq!(written[4], 0); // R
        assert_eq!(written[5], 255); // G
    }

    // ── Surface destruction ──────────────────────────────────────────────

    #[test]
    fn destroy_surface_removes_file() {
        let mut mgr = temp_mgr();
        let id = mgr.create_headless_surface("test", 16, 16).unwrap();
        let fb_path = mgr
            .get_surface(id)
            .unwrap()
            .framebuffer_path
            .clone()
            .unwrap();
        assert!(fb_path.exists());

        mgr.destroy_surface(id).unwrap();
        assert!(!fb_path.exists(), "framebuffer file should be removed");
        assert!(mgr.get_surface(id).is_none());
        assert_eq!(mgr.surface_count(), 0);
    }

    #[test]
    fn destroy_surface_rejects_unknown() {
        let mut mgr = temp_mgr();
        let result = mgr.destroy_surface(99999);
        assert!(result.is_err());
    }

    #[test]
    fn destroy_app_surfaces_removes_all() {
        let mut mgr = temp_mgr();
        let id1 = mgr.create_headless_surface("app1", 10, 10).unwrap();
        let id2 = mgr.create_headless_surface("app1", 20, 20).unwrap();
        let id3 = mgr.create_headless_surface("app2", 30, 30).unwrap();

        let removed = mgr.destroy_app_surfaces("app1").unwrap();
        assert_eq!(removed, 2);
        assert!(mgr.get_surface(id1).is_none());
        assert!(mgr.get_surface(id2).is_none());
        assert!(mgr.get_surface(id3).is_some());
        assert_eq!(mgr.surface_count(), 1);
    }

    #[test]
    fn destroy_app_surfaces_returns_zero_for_unknown() {
        let mut mgr = temp_mgr();
        let removed = mgr.destroy_app_surfaces("nonexistent").unwrap();
        assert_eq!(removed, 0);
    }

    // ── Manager state ────────────────────────────────────────────────────

    #[test]
    fn default_trait_works() {
        let mgr = WindowStubManager::default();
        assert_eq!(mgr.surface_count(), 0);
    }

    #[test]
    fn surface_count_tracks_create_and_destroy() {
        let mut mgr = temp_mgr();
        assert_eq!(mgr.surface_count(), 0);

        let id1 = mgr.create_headless_surface("a", 10, 10).unwrap();
        assert_eq!(mgr.surface_count(), 1);

        let _id2 = mgr.create_headless_surface("b", 10, 10).unwrap();
        assert_eq!(mgr.surface_count(), 2);

        mgr.destroy_surface(id1).unwrap();
        assert_eq!(mgr.surface_count(), 1);
    }

    #[test]
    fn framebuffer_dir_is_configurable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        let mgr = WindowStubManager::with_framebuffer_dir(path.clone());
        assert_eq!(mgr.framebuffer_dir(), path.as_path());
    }

    // ── Large surface (memory boundary) ──────────────────────────────────

    #[test]
    fn create_large_surface() {
        let mut mgr = temp_mgr();
        // 1920×1080 RGBA = ~8 MB
        let id = mgr.create_headless_surface("test", 1920, 1080).unwrap();
        let info = mgr.get_surface(id).unwrap();
        assert_eq!(info.buffer_size, 1920 * 1080 * 4);
        assert!(info.framebuffer_path.as_ref().unwrap().exists());
    }

    #[test]
    fn create_small_surface() {
        let mut mgr = temp_mgr();
        let id = mgr.create_headless_surface("test", 1, 1).unwrap();
        let info = mgr.get_surface(id).unwrap();
        assert_eq!(info.buffer_size, 4); // 1×1×4
    }

    // ── Multiple flushes ─────────────────────────────────────────────────

    #[test]
    fn multiple_flushes_to_same_surface() {
        let mut mgr = temp_mgr();
        let id = mgr.create_headless_surface("test", 8, 8).unwrap();

        let pixels1 = vec![0xFFu8; 8 * 8 * 4];
        mgr.map_gdi_flush(id, &pixels1).unwrap();

        let pixels2 = vec![0x00u8; 8 * 8 * 4];
        mgr.map_gdi_flush(id, &pixels2).unwrap();

        // Last flush wins.
        let info = mgr.get_surface(id).unwrap();
        let written = fs::read(info.framebuffer_path.as_ref().unwrap()).unwrap();
        assert_eq!(written, pixels2);
    }
}
