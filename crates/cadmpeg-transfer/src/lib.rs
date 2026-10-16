//! Transfer [cadmpeg](https://github.com/cadmpeg/cadmpeg) [`CadIr`] into SolidCraft geometry.
//!
//! v1 uses an in-memory STEP bridge inside [`step_bridge`]. Callers persist canonical CadIr;
//! direct IR→portable mapping will replace the bridge over time.

mod step_bridge;

use cadmpeg_ir::CadIr;
use solidcraft_kernel::StepImport;

pub use step_bridge::TransferError;

/// Loss notes from transfer (warnings from STEP export/import).
#[derive(Debug, Default)]
pub struct TransferReport {
    pub warnings: Vec<String>,
}

/// Build a [`StepImport`]-shaped preview from neutral IR (assembly planning and body names).
pub fn step_import_from_ir(ir: &CadIr) -> Result<(StepImport, TransferReport), TransferError> {
    step_bridge::step_import_from_ir(ir)
}

/// Importable solids via portable B-rep (preferred for document eval).
pub fn import_from_ir(ir: &CadIr) -> Result<(StepImport, TransferReport), TransferError> {
    step_bridge::portable_import_from_ir(ir)
}

#[cfg(test)]
mod tests {
    use cadmpeg_ir::examples::unit_cube;

    #[test]
    fn unit_cube_transfers_one_body() {
        let ir = unit_cube();
        let (imp, _) = super::import_from_ir(&ir).expect("unit cube");
        assert_eq!(imp.bodies.len(), 1);
        assert!(imp.bodies[0].closed);
    }
}
