# Workflow structure

This map follows the public shape of a parametric design workspace: a locked sketch, solid and surface modelling, mesh and form, assemblies, then drawings and a minimal machining contour. Command ids and labels are SolidCraft's own.

## Already in place

Sketch creation, the geometric constraints, dimensions and a fully-defined state in the browser. Extrude, revolve, sweep, loft, primitives, hole, thread, pattern, mirror, shell, draft, combine, split. Seven joint types, grounding, interference and exploded views. Sheet metal rules, flanges, unfold and flat pattern. Parameters and configurations. The timeline, browser, command palette, axis view with a home button, and shaded display styles.

## Filled in this pass

| Gap | Command |
| --- | --- |
| Inside edges only cut | `solid.fillet` and `solid.chamfer` fill a concave edge (the body grows) and still trim a convex edge |
| Mirror of faces | `solid.mirror` with `faces` |
| Loft guide | `solid.loft` `rails` |
| Thread start | `solid.thread` `offset` and `length`; hole `thread_offset` and `thread_length` |
| Position panel | `component.capture_position`, `component.revert_position` on SOLID › POSITION |
| Surface and sheet metal share construct, inspect, insert | existing commands, shown on those tabs |
| Active component timeline | the timeline lists that component's features |
| Zero-thickness solids | `surface.extrude`, `surface.revolve`, `surface.sweep`, `surface.loft`; `surface.untrim` drops the latest trim |
| Mesh prepare | `mesh.plane_cut`, `mesh.repair`, `mesh.reduce`, `mesh.face_groups`, `mesh.section`; `mesh.convert` `mode` `prismatic` or `faceted` |
| Form | `form.create`, `form.pull` on the FORM tab |
| Selection and colour | `view.select_mode` (`box`, `lasso`, `paint`; keys 1, 2, 3), `view.component_colors`, Alt-click sets the orbit pivot. The axis view already calls `view.standard.*` |
| Drawing | `drawing.create`, `drawing.base_view`, `drawing.project`, `drawing.section`, `drawing.dimension`, `drawing.export_svg` |
| Explode steps | `explode.frames` |
| One machining contour | `cam.contour` on UTILITIES › MAKE writes G-code for an outside profile |

## Still later

Generative studies, structural simulation, electronics, a full machining strategy list, and a physically based render studio are not in this pass. Mesh remesh quality and a closed T-spline box with symmetry are not either: form is a 4×4 patch that thickens into a solid, and a concave chamfer or fillet still refuses curved edges and loops that mix convex and concave.
