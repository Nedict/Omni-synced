# SolidCraft iconpack

146 icons, 24x24 grid, 1.5 px stroke, round caps and joins.

- 2D tool icons: flat outline. 3D object icons: 30 degree isometric, light from the top left.
- Ink is `currentColor`. The accent is `var(--icon-accent, #2F6FED)`; set `--icon-accent` on a parent to recolor.
- The four contact-set icons also use `var(--icon-bg, #fff)` for the small badge fill. Set it to your toolbar background on dark themes.
- `svg/` has one file per icon, `sprite.svg` has the same icons as `<symbol>`s (`<svg><use href="sprite.svg#2d-line"/></svg>`), `manifest.json` lists ids, names and groups.
- Drawn from scratch for SolidCraft. No commercial CAD artwork was used.
