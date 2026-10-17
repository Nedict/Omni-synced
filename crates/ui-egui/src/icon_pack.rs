//! The SolidCraft iconpack (`assets/icons/ui/pack`): SVG artwork rendered once per size and colour
//! set into a texture, used in place of the painter-drawn icon of the same name.
//!
//! Ink is the pack's `currentColor`, the accent is `var(--icon-accent, ..)`, and the contact badge
//! fill is `var(--icon-bg, ..)`; each is replaced with the caller's colour before the SVG is parsed.
//! A name not in `PACK`, or an SVG that fails to parse or render, returns `false` and the caller
//! paints the drawn icon instead.
use egui::{Color32, Painter, Rect, TextureHandle, TextureOptions, pos2};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// App icon name, pack artwork: the pack's own icons first, then the additive `3d-*` ones drawn
/// for commands the pack did not cover.
const PACK: &[(&str, &str)] = &[
    ("line", include_str!("../../../assets/icons/ui/pack/2d-line.svg")),
    ("rect", include_str!("../../../assets/icons/ui/pack/2d-rectangle.svg")),
    ("circle", include_str!("../../../assets/icons/ui/pack/2d-circle.svg")),
    ("arc3", include_str!("../../../assets/icons/ui/pack/2d-arc.svg")),
    ("dimension", include_str!("../../../assets/icons/ui/pack/2d-dimension.svg")),
    ("trim", include_str!("../../../assets/icons/ui/pack/2d-trim.svg")),
    ("offset", include_str!("../../../assets/icons/ui/pack/2d-offset.svg")),
    ("sketch_mirror", include_str!("../../../assets/icons/ui/pack/2d-mirror.svg")),
    ("move", include_str!("../../../assets/icons/ui/pack/2d-move.svg")),
    ("sketch_fillet", include_str!("../../../assets/icons/ui/pack/2d-fillet.svg")),
    ("point", include_str!("../../../assets/icons/ui/pack/2d-point.svg")),
    ("plane", include_str!("../../../assets/icons/ui/pack/2d-plane.svg")),
    ("axis", include_str!("../../../assets/icons/ui/pack/2d-axis.svg")),
    ("configuration_table", include_str!("../../../assets/icons/ui/pack/2d-config-table.svg")),
    ("box", include_str!("../../../assets/icons/ui/pack/3d-box.svg")),
    ("cylinder", include_str!("../../../assets/icons/ui/pack/3d-cylinder.svg")),
    ("sphere", include_str!("../../../assets/icons/ui/pack/3d-sphere.svg")),
    ("extrude", include_str!("../../../assets/icons/ui/pack/3d-extrude.svg")),
    ("revolve", include_str!("../../../assets/icons/ui/pack/3d-revolve.svg")),
    ("fillet", include_str!("../../../assets/icons/ui/pack/3d-fillet-3d.svg")),
    ("shell", include_str!("../../../assets/icons/ui/pack/3d-shell.svg")),
    ("hole", include_str!("../../../assets/icons/ui/pack/3d-hole.svg")),
    ("pattern_rect", include_str!("../../../assets/icons/ui/pack/3d-pattern.svg")),
    ("section", include_str!("../../../assets/icons/ui/pack/3d-section.svg")),
    ("split", include_str!("../../../assets/icons/ui/pack/3d-split-body.svg")),
    ("pipe", include_str!("../../../assets/icons/ui/pack/3d-pipe.svg")),
    ("thread", include_str!("../../../assets/icons/ui/pack/3d-thread.svg")),
    ("coil", include_str!("../../../assets/icons/ui/pack/3d-coil.svg")),
    ("emboss", include_str!("../../../assets/icons/ui/pack/3d-emboss.svg")),
    ("boundary_fill", include_str!("../../../assets/icons/ui/pack/3d-boundary-fill.svg")),
    ("rib", include_str!("../../../assets/icons/ui/pack/3d-rib.svg")),
    ("offset_face", include_str!("../../../assets/icons/ui/pack/3d-offset-face.svg")),
    ("capture", include_str!("../../../assets/icons/ui/pack/2d-capture.svg")),
    ("revert", include_str!("../../../assets/icons/ui/pack/2d-revert.svg")),
    ("material", include_str!("../../../assets/icons/ui/pack/2d-material.svg")),
    ("delete", include_str!("../../../assets/icons/ui/pack/3d-delete-face.svg")),
    ("sketch", include_str!("../../../assets/icons/ui/pack/3d-sketch.svg")),
    ("sweep", include_str!("../../../assets/icons/ui/pack/3d-sweep.svg")),
    ("loft", include_str!("../../../assets/icons/ui/pack/3d-loft.svg")),
    ("torus", include_str!("../../../assets/icons/ui/pack/3d-torus.svg")),
    ("pattern_circ", include_str!("../../../assets/icons/ui/pack/3d-pattern-circ.svg")),
    ("mirror", include_str!("../../../assets/icons/ui/pack/3d-mirror.svg")),
    ("joint_origin", include_str!("../../../assets/icons/ui/pack/3d-joint-origin.svg")),
    ("chamfer", include_str!("../../../assets/icons/ui/pack/3d-chamfer.svg")),
    ("combine", include_str!("../../../assets/icons/ui/pack/3d-combine.svg")),
    ("draft", include_str!("../../../assets/icons/ui/pack/3d-draft.svg")),
    ("presspull", include_str!("../../../assets/icons/ui/pack/3d-press-pull.svg")),
    ("compute", include_str!("../../../assets/icons/ui/pack/3d-compute.svg")),
    ("params", include_str!("../../../assets/icons/ui/pack/3d-params.svg")),
    ("appearance", include_str!("../../../assets/icons/ui/pack/3d-appearance.svg")),
    ("component", include_str!("../../../assets/icons/ui/pack/3d-component.svg")),
    ("joint", include_str!("../../../assets/icons/ui/pack/3d-joint.svg")),
    ("as_built", include_str!("../../../assets/icons/ui/pack/3d-as-built.svg")),
    ("rigid_group", include_str!("../../../assets/icons/ui/pack/3d-rigid-group.svg")),
    ("drive", include_str!("../../../assets/icons/ui/pack/3d-drive.svg")),
    ("motion_link", include_str!("../../../assets/icons/ui/pack/3d-motion-link.svg")),
    ("gear", include_str!("../../../assets/icons/ui/pack/3d-gear.svg")),
    ("rack_pinion", include_str!("../../../assets/icons/ui/pack/3d-rack-pinion.svg")),
    ("screw", include_str!("../../../assets/icons/ui/pack/3d-screw.svg")),
    ("cam", include_str!("../../../assets/icons/ui/pack/3d-cam.svg")),
    ("trace", include_str!("../../../assets/icons/ui/pack/3d-trace.svg")),
    ("interference", include_str!("../../../assets/icons/ui/pack/3d-interference.svg")),
    ("measure", include_str!("../../../assets/icons/ui/pack/3d-measure.svg")),
    ("fastener", include_str!("../../../assets/icons/ui/pack/3d-fastener.svg")),
    ("import", include_str!("../../../assets/icons/ui/pack/3d-import.svg")),
    ("export", include_str!("../../../assets/icons/ui/pack/3d-export.svg")),
    ("boss", include_str!("../../../assets/icons/ui/pack/3d-boss.svg")),
    ("lip", include_str!("../../../assets/icons/ui/pack/3d-lip.svg")),
    ("snap", include_str!("../../../assets/icons/ui/pack/3d-snap.svg")),
    ("rest", include_str!("../../../assets/icons/ui/pack/3d-rest.svg")),
    ("plastic_rule", include_str!("../../../assets/icons/ui/pack/3d-plastic-rule.svg")),
    ("plastic_assign", include_str!("../../../assets/icons/ui/pack/3d-plastic-assign.svg")),
    ("flange", include_str!("../../../assets/icons/ui/pack/3d-flange.svg")),
    ("hem", include_str!("../../../assets/icons/ui/pack/3d-hem.svg")),
    ("fold", include_str!("../../../assets/icons/ui/pack/3d-fold.svg")),
    ("convert_sheet", include_str!("../../../assets/icons/ui/pack/3d-convert-sheet.svg")),
    ("flat", include_str!("../../../assets/icons/ui/pack/3d-flat-pattern.svg")),
    ("unfold", include_str!("../../../assets/icons/ui/pack/3d-unfold.svg")),
    ("refold", include_str!("../../../assets/icons/ui/pack/3d-refold.svg")),
    ("sheet_rules", include_str!("../../../assets/icons/ui/pack/3d-sheet-rules.svg")),
    ("c_coincident", include_str!("../../../assets/icons/ui/pack/2d-c-coincident.svg")),
    ("c_concentric", include_str!("../../../assets/icons/ui/pack/2d-c-concentric.svg")),
    ("c_parallel", include_str!("../../../assets/icons/ui/pack/2d-c-parallel.svg")),
    ("c_tangent", include_str!("../../../assets/icons/ui/pack/2d-c-tangent.svg")),
    ("c_hv", include_str!("../../../assets/icons/ui/pack/2d-c-hv.svg")),
    ("c_perpendicular", include_str!("../../../assets/icons/ui/pack/2d-c-perpendicular.svg")),
    ("c_equal", include_str!("../../../assets/icons/ui/pack/2d-c-equal.svg")),
    ("c_midpoint", include_str!("../../../assets/icons/ui/pack/2d-c-midpoint.svg")),
    ("c_collinear", include_str!("../../../assets/icons/ui/pack/2d-c-collinear.svg")),
    ("c_symmetry", include_str!("../../../assets/icons/ui/pack/2d-c-symmetry.svg")),
    ("c_fix", include_str!("../../../assets/icons/ui/pack/2d-c-fix.svg")),
    ("c_smooth", include_str!("../../../assets/icons/ui/pack/2d-c-smooth.svg")),
    ("c_polygon", include_str!("../../../assets/icons/ui/pack/2d-c-polygon.svg")),
    ("arc_tangent", include_str!("../../../assets/icons/ui/pack/2d-arc-tangent.svg")),
    ("line_mid", include_str!("../../../assets/icons/ui/pack/2d-line-mid.svg")),
    ("rect_rounded", include_str!("../../../assets/icons/ui/pack/2d-rect-rounded.svg")),
    ("conic", include_str!("../../../assets/icons/ui/pack/2d-conic.svg")),
    ("ellipse", include_str!("../../../assets/icons/ui/pack/2d-ellipse.svg")),
    ("polygon", include_str!("../../../assets/icons/ui/pack/2d-polygon.svg")),
    ("slot", include_str!("../../../assets/icons/ui/pack/2d-slot.svg")),
    ("spline", include_str!("../../../assets/icons/ui/pack/2d-spline.svg")),
    ("text", include_str!("../../../assets/icons/ui/pack/2d-text.svg")),
    ("project", include_str!("../../../assets/icons/ui/pack/2d-project.svg")),
    ("extend", include_str!("../../../assets/icons/ui/pack/2d-extend.svg")),
    ("canvas", include_str!("../../../assets/icons/ui/pack/2d-canvas.svg")),
    ("dxf", include_str!("../../../assets/icons/ui/pack/2d-dxf.svg")),
    ("scale", include_str!("../../../assets/icons/ui/pack/2d-scale.svg")),
    ("break", include_str!("../../../assets/icons/ui/pack/2d-break.svg")),
    ("intersect", include_str!("../../../assets/icons/ui/pack/2d-intersect.svg")),
    ("sketch_chamfer", include_str!("../../../assets/icons/ui/pack/2d-sketch-chamfer.svg")),
    ("sketch_circ_pattern", include_str!("../../../assets/icons/ui/pack/2d-sketch-circ-pattern.svg")),
    ("sketch_rect_pattern", include_str!("../../../assets/icons/ui/pack/2d-sketch-rect-pattern.svg")),
    ("thicken", include_str!("../../../assets/icons/ui/pack/3d-thicken.svg")),
    ("ruled", include_str!("../../../assets/icons/ui/pack/3d-ruled.svg")),
    ("boundary", include_str!("../../../assets/icons/ui/pack/3d-boundary.svg")),
    ("surface_offset", include_str!("../../../assets/icons/ui/pack/3d-surface-offset.svg")),
    ("surface_extrude", include_str!("../../../assets/icons/ui/pack/3d-surface-extrude.svg")),
    ("surface_revolve", include_str!("../../../assets/icons/ui/pack/3d-surface-revolve.svg")),
    ("surface_sweep", include_str!("../../../assets/icons/ui/pack/3d-surface-sweep.svg")),
    ("surface_loft", include_str!("../../../assets/icons/ui/pack/3d-surface-loft.svg")),
    ("surface_blend", include_str!("../../../assets/icons/ui/pack/3d-surface-blend.svg")),
    ("stitch", include_str!("../../../assets/icons/ui/pack/3d-stitch.svg")),
    ("unstitch", include_str!("../../../assets/icons/ui/pack/3d-unstitch.svg")),
    ("untrim", include_str!("../../../assets/icons/ui/pack/3d-untrim.svg")),
    ("reverse_normal", include_str!("../../../assets/icons/ui/pack/3d-reverse-normal.svg")),
    ("iso_curve", include_str!("../../../assets/icons/ui/pack/3d-iso-curve.svg")),
    ("construction", include_str!("../../../assets/icons/ui/pack/2d-construction.svg")),
];

type Key = (usize, u32, [u8; 3], [u8; 3], [u8; 3]);

/// One cache per egui context: a texture handle is only good in the context that loaded it.
type Cache = Arc<Mutex<HashMap<Key, Option<TextureHandle>>>>;

fn hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

fn rasterize(svg: &str, ink: Color32, accent: Color32, bg: Color32, px: u32) -> Option<egui::ColorImage> {
    let svg = svg.replace("currentColor", &hex(ink)).replace("var(--icon-accent,#2F6FED)", &hex(accent)).replace("var(--icon-bg,#fff)", &hex(bg));
    let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(px, px)?;
    let s = px as f32 / 24.0;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(s, s), &mut pixmap.as_mut());
    let side = px as usize;
    Some(egui::ColorImage::from_rgba_premultiplied([side, side], pixmap.data()))
}

/// Paint the pack artwork for `name` into `r` (a square, centred in `r`). Returns whether it did.
pub fn paint(p: &Painter, r: Rect, name: &str, ink: Color32, fill: Color32, accent: Color32) -> bool {
    // Unit tests build short-lived contexts and drop their output unapplied, which a texture
    // upload would trip over (`TexturesDelta` asserts on drop). They test the code-drawn icons.
    if cfg!(test) {
        return false;
    }
    let Some(idx) = PACK.iter().position(|(n, _)| *n == name) else { return false };
    let side = r.width().min(r.height());
    let px = (side * p.ctx().pixels_per_point()).round().clamp(8.0, 512.0) as u32;
    let key: Key = (idx, px, [ink.r(), ink.g(), ink.b()], [accent.r(), accent.g(), accent.b()], [fill.r(), fill.g(), fill.b()]);
    let tex = {
        let cache: Cache = p.ctx().data_mut(|d| d.get_temp_mut_or_insert_with(egui::Id::new("icon_pack_cache"), Cache::default).clone());
        let Ok(mut cache) = cache.lock() else { return false };
        if cache.len() > 4096 {
            cache.clear();
        }
        cache
            .entry(key)
            .or_insert_with(|| {
                let img = rasterize(PACK[idx].1, ink, accent, fill, px)?;
                Some(p.ctx().load_texture(format!("iconpack/{name}/{px}"), img, TextureOptions::LINEAR))
            })
            .clone()
    };
    let Some(tex) = tex else { return false };
    let square = Rect::from_center_size(r.center(), egui::vec2(side, side));
    p.image(tex.id(), square, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::from_white_alpha(ink.a()));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every mapped SVG parses and draws something, with the caller's ink and accent substituted.
    #[test]
    fn every_pack_icon_renders_with_the_callers_colours() {
        let (ink, accent, bg) = (Color32::from_rgb(10, 20, 30), Color32::from_rgb(200, 40, 40), Color32::WHITE);
        let mut ignores_accent = Vec::new();
        for (name, svg) in PACK {
            let img = rasterize(svg, ink, accent, bg, 48).unwrap_or_else(|| panic!("{name} did not render"));
            assert!(img.pixels.iter().any(|c| c.a() > 0), "{name} rendered blank");
            if svg.contains("icon-accent") {
                let other = rasterize(svg, ink, Color32::from_rgb(40, 200, 40), bg, 48).unwrap_or_else(|| panic!("{name} did not render"));
                if img.pixels == other.pixels {
                    ignores_accent.push(*name);
                }
            }
        }
        // c_parallel's only accent element is drawn at stroke-opacity 0, so recolouring it changes nothing.
        ignores_accent.retain(|name| *name != "c_parallel");
        assert!(ignores_accent.is_empty(), "{ignores_accent:?}");
    }
}
