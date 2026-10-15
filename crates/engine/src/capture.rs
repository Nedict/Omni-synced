//! How headless pictures of the model are drawn: a renderer the host installs. The engine
//! describes what a picture shows ([`crate::view::scene`]); drawing it is a GPU job the engine
//! cannot do itself (the GPU crate sits above it), so the application, the CLI and the MCP
//! server install one ([`install`]) that draws with the viewport's own pipelines. Without one,
//! a picture is an error and a save simply has no thumbnail.

use std::sync::{Arc, RwLock};

use solidcraft_render::{Camera, Scene};

/// Draws `scene` seen by `cam` at `width` × `height` pixels: straight RGBA, rows top to bottom
/// (`width * height * 4` bytes), or why it could not.
pub type Renderer = dyn Fn(&Scene, &Camera, u32, u32) -> Result<Vec<u8>, String> + Send + Sync;

static RENDERER: RwLock<Option<Arc<Renderer>>> = RwLock::new(None);

/// Make `renderer` draw this process's headless pictures (replacing any installed before).
pub fn install(renderer: Arc<Renderer>) {
    if let Ok(mut r) = RENDERER.write() {
        *r = Some(renderer);
    }
}

/// The installed renderer, if any.
pub fn installed() -> Option<Arc<Renderer>> {
    RENDERER.read().ok().and_then(|r| r.clone())
}

/// `scene` drawn by the installed renderer: RGBA pixels.
pub fn rgba(scene: &Scene, cam: &Camera, width: u32, height: u32) -> Result<Vec<u8>, String> {
    rgba_with(installed().as_deref(), scene, cam, width, height)
}

/// `scene` drawn by the installed renderer, as PNG bytes.
pub fn png(scene: &Scene, cam: &Camera, width: u32, height: u32) -> Result<Vec<u8>, String> {
    png_with(installed().as_deref(), scene, cam, width, height)
}

/// [`rgba`] with `renderer` instead of the installed one.
pub fn rgba_with(renderer: Option<&Renderer>, scene: &Scene, cam: &Camera, width: u32, height: u32) -> Result<Vec<u8>, String> {
    let renderer = renderer.ok_or("no renderer is installed to draw pictures in this process")?;
    let rgba = renderer(scene, cam, width, height)?;
    if rgba.len() as u64 != u64::from(width) * u64::from(height) * 4 {
        return Err(format!("the renderer returned {} bytes for {width} × {height} pixels", rgba.len()));
    }
    Ok(rgba)
}

/// [`png`] with `renderer` instead of the installed one.
pub fn png_with(renderer: Option<&Renderer>, scene: &Scene, cam: &Camera, width: u32, height: u32) -> Result<Vec<u8>, String> {
    let rgba = rgba_with(renderer, scene, cam, width, height)?;
    solidcraft_render::png(width as usize, height as usize, rgba).ok_or_else(|| "PNG encoding failed".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_a_renderer_a_picture_is_an_error_not_a_crash() {
        let e = png_with(None, &Scene::default(), &Camera::default(), 16, 16).unwrap_err();
        assert!(e.contains("no renderer"), "{e}");
    }

    #[test]
    fn a_renderer_that_returns_the_wrong_size_is_an_error() {
        let short: &Renderer = &|_, _, _, _| Ok(vec![0; 3]);
        assert!(rgba_with(Some(short), &Scene::default(), &Camera::default(), 2, 2).is_err());
        let plain: &Renderer = &|_, _, w, h| Ok(vec![255; (w * h * 4) as usize]);
        let png = png_with(Some(plain), &Scene::default(), &Camera::default(), 2, 2).unwrap();
        assert!(png.starts_with(b"\x89PNG"));
    }
}
