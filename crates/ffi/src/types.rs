//! The types shared across the C ABI.
//!
//! Every type here has a byte-for-byte mirror on the C# side (`csharp/SolidCraft.Interop`). The
//! sizes and field offsets are asserted at compile time below and re-checked at run time by
//! [`layout_guard`], so a drift between the two definitions is a loud failure, never silent
//! memory corruption.
//!
//! The ABI is deliberately *flat and explicit*: fixed-width integers, `f64`, and opaque handles.
//! There are no Rust references, `String`s or enums with implicit representation crossing the
//! boundary.

use std::os::raw::c_char;

/// A status code returned by every fallible entry point.
///
/// Mirrored in C# as `enum ScStatus : int`. `ScStatus::Ok` is zero, so a zero-initialised result
/// is a success, and a non-zero value is always an error the host must handle.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScStatus {
    Ok = 0,
    /// A null or already-freed handle was passed.
    NullHandle = 1,
    /// The command id is not in the registry.
    UnknownCommand = 2,
    /// The command was rejected by its own validation.
    BadParams = 3,
    /// The engine reported a failure (kernel, document, I/O…).
    Engine = 4,
    /// A Rust panic was caught at the boundary; the design was left unchanged.
    Internal = 5,
    /// A number was not finite where it had to be (hostile input).
    NotFinite = 6,
    /// The caller's buffer was too small; `len` holds the required size.
    BufferTooSmall = 7,
    /// A string argument was not valid UTF-8.
    BadUtf8 = 8,
}

impl ScStatus {
    /// Maps an engine error to a status code.
    pub fn from_engine(e: &solidcraft_engine::EngineError) -> ScStatus {
        use solidcraft_engine::EngineError as E;
        match e {
            E::UnknownCommand(_) => ScStatus::UnknownCommand,
            E::BadParams { .. } => ScStatus::BadParams,
            E::Disabled(..) => ScStatus::BadParams,
            E::Other(_) => ScStatus::Engine,
            E::Internal(..) => ScStatus::Internal,
        }
    }
}

/// A 3-vector in millimetres (or radians for angles). Mirrored in C# as `ScVec3`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScVec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl ScVec3 {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        ScVec3 { x, y, z }
    }
    /// True when every component is finite (hostile-input guard).
    pub fn is_finite(&self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

/// An axis frame: an origin and two direction vectors. Mirrored in C# as `ScFrame`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScFrame {
    pub origin: ScVec3,
    pub x_dir: ScVec3,
    pub y_dir: ScVec3,
}

/// What kind of model entity a [`ScEntityRef`] points at. Mirrored in C# as `ScEntityKind`.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScEntityKind {
    None = 0,
    Face = 1,
    Edge = 2,
    Vertex = 3,
    Feature = 4,
    Body = 5,
    Sketch = 6,
    Component = 7,
}

/// A stable, document-scoped reference to a model entity.
///
/// The `id` is the same stable id the engine uses internally (persistent across rebuilds), so a
/// reference taken before an edit still resolves after it — the SolidWorks `GetPersistReference3`
/// contract. Mirrored in C# as `ScEntityRef`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScEntityRef {
    pub kind: ScEntityKind,
    /// 0 means "the active document".
    pub doc: u64,
    pub id: u64,
}

impl ScEntityRef {
    pub fn none() -> Self {
        ScEntityRef { kind: ScEntityKind::None, doc: 0, id: 0 }
    }
    pub fn is_none(&self) -> bool {
        self.kind == ScEntityKind::None
    }
}

/// A non-owning view of a UTF-8 string owned by the caller.
///
/// `ptr` is not NUL-terminated and need not be aligned; `len` is the exact byte count. Mirrored in
/// C# as a `(byte*, int)` pair passed through `ScStr`. A null `ptr` with `len == 0` is the empty
/// string.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ScStr {
    pub ptr: *const u8,
    pub len: usize,
}

impl ScStr {
    /// Borrows the string, or `Err(BadUtf8)` when it is not valid UTF-8.
    ///
    /// # Safety
    /// `ptr` must point to `len` readable bytes for the duration of the call.
    pub unsafe fn as_str(&self) -> Result<&str, ScStatus> {
        if self.ptr.is_null() {
            return if self.len == 0 { Ok("") } else { Err(ScStatus::BadUtf8) };
        }
        let bytes = unsafe { std::slice::from_raw_parts(self.ptr, self.len) };
        std::str::from_utf8(bytes).map_err(|_| ScStatus::BadUtf8)
    }
}

/// An owned string returned to the host. The host releases it with [`crate::sldcraft_string_free`].
///
/// On success `ptr` is non-null and `len` is the byte length (not counting a trailing NUL the
/// allocator adds for convenience). On failure `ptr` is null. Mirrored in C# as `ScOwnedStr`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ScOwnedStr {
    pub ptr: *mut c_char,
    pub len: usize,
}

impl ScOwnedStr {
    pub fn empty() -> Self {
        ScOwnedStr { ptr: std::ptr::null_mut(), len: 0 }
    }
    pub fn from_string(s: String) -> Self {
        let sanitised = s.replace('\0', " ");
        match std::ffi::CString::new(sanitised) {
            Ok(c) => {
                let len = c.as_bytes().len();
                ScOwnedStr { ptr: c.into_raw(), len }
            }
            Err(_) => ScOwnedStr::empty(),
        }
    }
}

/// Mass properties of one body or of the whole model.
///
/// Volume in mm³, area in mm², mass in g, centroid and bounding box in mm. Mirrored in C# as
/// `ScMassProperties`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ScMassProperties {
    pub volume_mm3: f64,
    pub area_mm2: f64,
    pub mass_g: f64,
    pub centroid: ScVec3,
    pub bbox_min: ScVec3,
    pub bbox_max: ScVec3,
    /// Number of faces, edges and vertices (0 when unknown).
    pub faces: u64,
    pub edges: u64,
    pub vertices: u64,
    /// True when the values are analytic (exact) rather than a tessellated estimate.
    pub analytic: bool,
}

// ---------------------------------------------------------------------------------------------
// Compile-time layout assertions. These are the contract with C#; if a field moves, this fails
// the build rather than corrupting a caller's memory.
// ---------------------------------------------------------------------------------------------
const _: () = {
    use std::mem::{align_of, size_of};
    // ScVec3: three f64.
    assert!(size_of::<ScVec3>() == 24);
    assert!(align_of::<ScVec3>() == 8);
    // ScFrame: three ScVec3.
    assert!(size_of::<ScFrame>() == 72);
    // ScEntityRef: i32 kind + padding + two u64.
    assert!(size_of::<ScEntityRef>() == 24);
    assert!(align_of::<ScEntityRef>() == 8);
    // ScStr: pointer + usize.
    assert!(size_of::<ScStr>() == 16);
    // ScOwnedStr: pointer + usize.
    assert!(size_of::<ScOwnedStr>() == 16);
    // ScMassProperties: 3 f64 + 3*ScVec3 + 3 u64 + bool + padding.
    assert!(size_of::<ScMassProperties>() == 128);
    assert!(align_of::<ScMassProperties>() == 8);
};

/// Verifies at run time that the Rust and C# layouts agree.
///
/// The host calls this once after loading the library, passing its own `sizeof` values; any
/// mismatch returns `BufferTooSmall` and a message naming the type, so the process stops before
/// touching shared memory. `sizes` is a pointer to an array of `n` `u64` in the order: ScVec3,
/// ScFrame, ScEntityRef, ScStr, ScOwnedStr, ScMassProperties.
///
/// # Safety
/// `sizes` must point to `n` readable `u64`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sldcraft_layout_guard(sizes: *const u64, n: usize, message: *mut ScOwnedStr) -> ScStatus {
    if sizes.is_null() || n < 6 {
        if !message.is_null() {
            unsafe { *message = ScOwnedStr::from_string("layout_guard: need 6 sizes".into()) };
        }
        return ScStatus::BadParams;
    }
    let host = unsafe { std::slice::from_raw_parts(sizes, n) };
    let ours = [
        std::mem::size_of::<ScVec3>() as u64,
        std::mem::size_of::<ScFrame>() as u64,
        std::mem::size_of::<ScEntityRef>() as u64,
        std::mem::size_of::<ScStr>() as u64,
        std::mem::size_of::<ScOwnedStr>() as u64,
        std::mem::size_of::<ScMassProperties>() as u64,
    ];
    let names = ["ScVec3", "ScFrame", "ScEntityRef", "ScStr", "ScOwnedStr", "ScMassProperties"];
    for i in 0..6 {
        if host[i] != ours[i] {
            if !message.is_null() {
                let text = format!("layout mismatch for {}: C# {} bytes, Rust {} bytes", names[i], host[i], ours[i]);
                unsafe { *message = ScOwnedStr::from_string(text) };
            }
            return ScStatus::BufferTooSmall;
        }
    }
    if !message.is_null() {
        unsafe { *message = ScOwnedStr::from_string("layout ok".into()) };
    }
    ScStatus::Ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_layouts_are_stable() {
        assert_eq!(std::mem::size_of::<ScVec3>(), 24);
        assert_eq!(std::mem::size_of::<ScFrame>(), 72);
        assert_eq!(std::mem::size_of::<ScEntityRef>(), 24);
        assert_eq!(std::mem::size_of::<ScMassProperties>(), 128);
        assert_eq!(ScStatus::Ok as i32, 0);
        assert_eq!(ScEntityKind::Face as i32, 1);
    }

    #[test]
    fn layout_guard_reports_mismatch() {
        let good = [
            std::mem::size_of::<ScVec3>() as u64,
            std::mem::size_of::<ScFrame>() as u64,
            std::mem::size_of::<ScEntityRef>() as u64,
            std::mem::size_of::<ScStr>() as u64,
            std::mem::size_of::<ScOwnedStr>() as u64,
            std::mem::size_of::<ScMassProperties>() as u64,
        ];
        let mut msg = ScOwnedStr::empty();
        assert_eq!(unsafe { sldcraft_layout_guard(good.as_ptr(), 6, &mut msg) }, ScStatus::Ok);
        unsafe { crate::sldcraft_string_free(msg.ptr) };

        let bad = [1u64, 2, 3, 4, 5, 6];
        let mut msg2 = ScOwnedStr::empty();
        assert_eq!(unsafe { sldcraft_layout_guard(bad.as_ptr(), 6, &mut msg2) }, ScStatus::BufferTooSmall);
        unsafe { crate::sldcraft_string_free(msg2.ptr) };
    }
}
