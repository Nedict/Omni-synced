# Estimate materials

Open **SOLID → INSPECT → Estimate Materials**, or search for **Estimate Materials** in the
command palette. The floating window estimates material for every placed solid in the design.
This first version applies one editable wood purchase reference to all parts; it does not
infer prices from physical material names or supply market prices.

## Purchase reference

Enter a reference name, a purchase price in euros and the purchased board's length, width and
thickness in millimetres. The example starts at **180 € for 1400 × 750 × 40 mm**, which is
42 litres and approximately **4285.71 €/m³**. It is an illustrative reference to replace with
a supplier's price. A price of zero is valid; dimensions must be positive.

Choose the quantity basis:

- **Rectangular blanks** estimates one bounding-box blank per part. Holes and notches do not
  reduce that blank. Boxes are measured in the body's definition frame, so rotating assembly
  instances does not inflate their purchase volumes. A rotation baked into a body's geometry
  can change its axis-aligned blank; this is not an oriented-box optimizer or a cutting layout.
- **Net solid volume** uses the finished solid after cuts, including recorded sheet
  perforations. It is useful for comparing the material left in the design but generally
  undercounts wood needed before machining.

The editable waste allowance increases the chosen quantity once. The table shows each part,
its placed quantity, per-part blank dimensions, total volume including allowance and cost.
Hidden solids are counted: visibility is a presentation choice, not a purchase quantity.

The calculation is:

```text
reference volume m³ = length mm × width mm × thickness mm / 1,000,000,000
reference price €/m³ = purchase price € / reference volume m³
estimated volume = selected quantity × (1 + waste percent / 100)
estimated cost = estimated volume × reference price €/m³
```

Assembly occurrences, including nested repeated components, count separately. Meshes, open
surfaces, failed features, stale models and estimates beyond supported numeric/traversal
limits return an explicit error rather than a partial total.

## Save and reuse

Editing fields recalculates a read-only preview. **Save reference** stores the settings in the
project in one undo step; then save the design file to retain them after closing the app.
Closing the window without saving the reference discards its draft. Changing designs closes
the window so a reference draft cannot leak into another project.

The optional settings use the existing informational metadata namespace
`org.solidcraft.material_estimate`. They do not affect geometry. Other metadata is preserved.
A reader that supports the file's design format retains this namespace as unknown
informational data. Normal format-version and pending-change compatibility checks still
apply. Invalid saved settings are reported, never silently replaced by another price.

## Commands, CLI and MCP

Programmatic calls never open the window. Use `ui.start` for the interactive surface:

```json
{"method":"ui.start","params":{"command":"material.estimate"}}
```

The same engine calculation is available through `execute` in MCP or `engine.execute` on the
control channel:

```json
{"command":"material.estimate","params":{}}
```

An optional complete `settings` object overrides the saved reference for a read-only
calculation. `material.estimate.settings` persists the same object:

```json
{
  "command": "material.estimate.settings",
  "params": {
    "settings": {
      "reference_name": "Supplier wood reference",
      "reference_price": 180,
      "reference_length_mm": 1400,
      "reference_width_mm": 750,
      "reference_thickness_mm": 40,
      "waste_percent": 15,
      "basis": "bounding_box"
    }
  }
}
```

`basis` is `bounding_box` or `net_volume`. The report returns the validated settings,
`reference_price_per_m3`, `bodies` and `total`. Row dimensions are per part; row volumes and
cost already include `quantity`. Volumes in reports are m³, not the mm³ used by geometric
measurement commands.

This is a material estimate, not a supplier quotation. It excludes assembly, hardware,
finishes, delivery and cutting optimization. Price-per-mass, mixed material rates and print
process allowances for filament/resin are future work; applying solid volume alone would not
predict a printer's infill, supports or waste.
