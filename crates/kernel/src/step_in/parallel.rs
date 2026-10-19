//! Bounded desktop preparation of independent imported bodies, collected in file order.
use super::{Body, Result, guard, mt};

pub(super) struct Job {
    pub solid: mt::Solid,
    /// A successfully prepared closed instance and its original placement.
    pub prepared: Option<(Body, mt::Matrix4)>,
    pub check_mesh: bool,
}

type Prepared = Result<(Body, usize)>;

pub(super) fn workers() -> usize {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::thread::available_parallelism().map_or(1, |n| n.get().min(8))
    }
    #[cfg(target_arch = "wasm32")]
    {
        1
    }
}

fn prepare(job: &Job, placement: &mt::Matrix4) -> Prepared {
    guard("STEP body preparation", || {
        // Preparing the same part again would repeat orientation meshing. Placement through
        // the body API preserves its orientation, face order and cached mesh warnings.
        if let Some((body, first)) = &job.prepared
            && let Some(delta) = rigid_delta(first, placement)
            && let Ok(body) = crate::transform_matrix(body, delta)
        {
            let missing = if job.check_mesh { body.unmeshed_faces() } else { 0 };
            return Ok((body, missing));
        }
        let solid = if *placement == mt::Matrix4::from_scale(1.0) { job.solid.clone() } else { mt::builder::transformed(&job.solid, *placement) };
        let body = Body::new(solid)?;
        let missing = if job.check_mesh { body.unmeshed_faces() } else { 0 };
        Ok((body, missing))
    })
}

fn rigid_delta(first: &mt::Matrix4, placement: &mt::Matrix4) -> Option<[[f64; 4]; 4]> {
    use mt::SquareMatrix as _;
    let rigid = |m: &mt::Matrix4| {
        let a = super::to_array(m);
        a.iter().flatten().all(|x| x.is_finite()) && m.x.w == 0.0 && m.y.w == 0.0 && m.z.w == 0.0 && m.w.w == 1.0 && crate::ops::rigid_placement(&a)
    };
    if !rigid(first) || !rigid(placement) {
        return None;
    }
    let delta = placement * first.invert()?;
    rigid(&delta).then(|| super::to_array(&delta))
}

pub(super) fn bodies(solids: &[Job], placement: &mt::Matrix4, workers: usize) -> Vec<Prepared> {
    #[cfg(not(target_arch = "wasm32"))]
    if solids.len() >= 4 && workers > 1 {
        return std::thread::scope(|scope| {
            let size = solids.len().div_ceil(workers.min(8));
            let mut pending = Vec::new();
            for chunk in solids.chunks(size) {
                let handle = std::thread::Builder::new()
                    .name("step-body".into())
                    .spawn_scoped(scope, move || chunk.iter().map(|solid| prepare(solid, placement)).collect::<Vec<_>>())
                    .ok();
                pending.push((chunk, handle));
            }
            let mut output = Vec::with_capacity(solids.len());
            for (chunk, handle) in pending {
                match handle {
                    Some(handle) => match handle.join() {
                        Ok(results) => output.extend(results),
                        Err(_) => output.extend(chunk.iter().map(|_| Err(crate::KernelError::Internal("STEP body worker panicked".into())))),
                    },
                    // Resource exhaustion must not lose an import: use the serial path.
                    None => output.extend(chunk.iter().map(|solid| prepare(solid, placement))),
                }
            }
            output
        });
    }
    let _ = workers;
    solids.iter().map(|solid| prepare(solid, placement)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use solidcraft_geom::Vec3;

    #[test]
    fn prepared_rigid_instances_do_not_rebuild_or_remesh_the_raw_solid() {
        let body = crate::box_solid(Vec3::ZERO, Vec3::new(2.0, 3.0, 4.0)).unwrap();
        let first = mt::Matrix4::from_translation(mt::Vector3::new(10.0, 20.0, 30.0));
        let prepared = crate::transform_matrix(&body, super::super::to_array(&first)).unwrap();
        // If preparation falls back to Body::new this empty solid fails. This checks reuse
        // without a wall-clock assertion or depending on a particular triangulator.
        let job = Job { solid: mt::Solid::new(Vec::new()), prepared: Some((prepared, first)), check_mesh: false };
        let reflection = mt::Matrix4::from_nonuniform_scale(-1.0, 1.0, 1.0);
        let rotated = mt::Matrix4::from_angle_z(mt::Rad(std::f64::consts::FRAC_PI_2));
        for placement in [mt::Matrix4::from_scale(1.0), first, rotated, reflection] {
            let (placed, missing) = prepare(&job, &placement).unwrap();
            assert_eq!(missing, 0);
            let measured = crate::measure(&placed).unwrap();
            assert!((measured.volume - 24.0).abs() < 1e-8);
            let expected = crate::transform_matrix(&body, super::super::to_array(&placement)).unwrap();
            let expected = crate::measure(&expected).unwrap();
            assert!(measured.bbox.min.dist(expected.bbox.min) < 1e-8);
            assert!(measured.bbox.max.dist(expected.bbox.max) < 1e-8);
        }
    }

    #[test]
    fn unusable_prepared_instance_falls_back_to_raw_preparation() {
        let body = crate::box_solid(Vec3::ZERO, Vec3::new(2.0, 3.0, 4.0)).unwrap();
        let identity = mt::Matrix4::from_scale(1.0);
        for first in [mt::Matrix4::from_scale(2.0), mt::Matrix4::from_scale(0.0), mt::Matrix4::from_scale(f64::NAN)] {
            let job = Job { solid: body.solid.as_ref().clone(), prepared: Some((body.clone(), first)), check_mesh: true };
            let (placed, missing) = prepare(&job, &identity).unwrap();
            assert_eq!(missing, 0);
            assert!((crate::measure(&placed).unwrap().volume - 24.0).abs() < 1e-8);
        }
        let job = Job { solid: body.solid.as_ref().clone(), prepared: Some((body, identity)), check_mesh: true };
        let scale = mt::Matrix4::from_scale(2.0);
        assert!(rigid_delta(&identity, &scale).is_none());
        let (placed, _) = prepare(&job, &scale).unwrap();
        assert!((crate::measure(&placed).unwrap().volume - 192.0).abs() < 1e-8);
        let mut perspective = identity;
        perspective.x.w = 0.1;
        assert!(rigid_delta(&identity, &perspective).is_none());
    }
}
