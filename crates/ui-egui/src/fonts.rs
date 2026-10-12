//! System font loading and fallback setup for multi-language (CJK: Japanese, Chinese, Korean) support.

#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;

/// Candidate font paths per script, in order of preference on each operating system. The first
/// readable font of every group is added as a fallback, so a Japanese font that lacks Hangul
/// (Meiryo, Hiragino) doesn't leave Korean text as empty boxes.
#[cfg(not(target_arch = "wasm32"))]
const FONT_GROUPS: &[(&str, &[&str])] = &[
    (
        "cjk_fallback",
        &[
            // Windows
            "C:\\Windows\\Fonts\\meiryo.ttc",
            "C:\\Windows\\Fonts\\YuGothM.ttc",
            "C:\\Windows\\Fonts\\YuGothR.ttc",
            "C:\\Windows\\Fonts\\msgothic.ttc",
            "C:\\Windows\\Fonts\\BIZ-UDGothicR.ttc",
            // macOS
            "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
            "/System/Library/Fonts/PingFang.ttc",
            "/Library/Fonts/Arial Unicode.ttf",
            // Linux
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
        ],
    ),
    (
        "hangul_fallback",
        &[
            // Windows: Malgun Gothic
            "C:\\Windows\\Fonts\\malgun.ttf",
            // macOS: Apple SD Gothic Neo
            "/System/Library/Fonts/AppleSDGothicNeo.ttc",
            "/System/Library/Fonts/Supplemental/AppleGothic.ttf",
            // Linux (Noto Sans CJK above already covers Hangul)
            "/usr/share/fonts/truetype/nanum/NanumGothic.ttf",
            "/usr/share/fonts/nanum/NanumGothic.ttf",
        ],
    ),
];

/// Configure font definitions with system CJK and Hangul fallbacks where available.
#[cfg(not(target_arch = "wasm32"))]
pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let mut found = false;
    for (name, paths) in FONT_GROUPS {
        let bytes = paths.iter().find_map(|path| std::fs::read(path).ok().filter(|bytes| ttf_parser::Face::parse(bytes, 0).is_ok()));
        let Some(bytes) = bytes else { continue };
        fonts.font_data.insert((*name).to_owned(), Arc::new(egui::FontData::from_owned(bytes)));
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts.families.entry(family).or_default().push((*name).to_owned());
        }
        found = true;
    }
    if found {
        ctx.set_fonts(fonts);
    }
}

/// Browsers cannot read system font files. A web host can supply licensed fonts through egui.
#[cfg(target_arch = "wasm32")]
pub fn setup_fonts(_ctx: &egui::Context) {}
