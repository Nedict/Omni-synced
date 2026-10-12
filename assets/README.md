# SolidCraft macOS icon

`SolidCraft.icon/` is the approved Icon Composer source, containing separate
background, grid and solid SVG layers and the owner's appearance settings.
The white edges are clipped to the solid silhouette. Artwork is original and
attributed in `ATTRIBUTION.md`.

`Assets.car` is compiled by Apple's `actool` and supplies the layered icon on
macOS 26 and later. It is checked in so the existing release workflow can still
package on older Xcode runners. After editing and saving `SolidCraft.icon`,
regenerate the catalog with Xcode 26 or later from the repository root:

```sh
mkdir -p target/macos-icon
xcrun actool assets/SolidCraft.icon \
  --compile target/macos-icon --platform macosx \
  --minimum-deployment-target 11.0 --app-icon SolidCraft \
  --output-partial-info-plist target/macos-icon/icon-info.plist \
  --output-format human-readable-text
cp target/macos-icon/Assets.car assets/Assets.car
```

`icon.vectorcraft` and `icon.svg` preserve the traditional artwork before
Icon Composer's system material effects. `icon.png` is its transparent 1024 px
VectorCraft export with native rounded corners and optical padding. The
`icon.iconset/` variants and `icon.icns` provide the older-macOS fallback.
After re-exporting the traditional PNG, regenerate them with Apple's tools:

```sh
for s in 16 32 128 256 512; do
  sips -z "$s" "$s" assets/icon.png \
    --out "assets/icon.iconset/icon_${s}x${s}.png" >/dev/null
  sips -z "$((s * 2))" "$((s * 2))" assets/icon.png \
    --out "assets/icon.iconset/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns -o assets/icon.icns assets/icon.iconset
```

Only `packaging/macos/package.sh` consumes these resources. Its existing signed
app/DMG workflow copies the catalog and ICNS into `Contents/Resources`, and
`Info.plist` references `SolidCraft` through `CFBundleIconName` and
`CFBundleIconFile`. The macOS viewport opts out of eframe's runtime bitmap so
Finder, the Dock and the application switcher use the bundle icon. Other
platforms retain the existing code-drawn icon and packaging.

Design references: [Apple app-icon HIG](https://developer.apple.com/design/human-interface-guidelines/app-icons)
and [Icon Composer documentation](https://developer.apple.com/documentation/xcode/creating-your-app-icon-using-icon-composer).
No Apple artwork is shipped.
