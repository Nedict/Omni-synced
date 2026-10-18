//! A Plane at Angle can turn about a model edge (`axis`: a `GeoRef` such as
//! `{"type": "edge", "at": …, "name": …}`), found again on every evaluation so the plane follows
//! the edge; `axis_origin` and `axis_dir` keep the line as picked. Older designs need no
//! transformation: a plane without `axis` turns about its fixed line as before; 0.15.x asks for
//! an update instead of fixing the plane where the edge was.
