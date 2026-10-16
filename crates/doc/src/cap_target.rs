//! Logical start/end caps retain their producer and profile across travel reversals.
use crate::config::{ConfigFaceTarget, FaceCertificate, cap_key};
use crate::{Direction, DocError, Document, FeatureKind, Result};

impl Document {
    pub fn extrusion_cap_high(&self, producer: u64, end: bool) -> Result<bool> {
        let feature =
            self.feature(producer).filter(|f| !f.suppressed).ok_or_else(|| DocError::Unknown("extrusion cap producer is unavailable".into()))?;
        let FeatureKind::Extrude { extent, .. } = &feature.kind else {
            return Err(DocError::Invalid("logical cap requires an extrusion producer".into()));
        };
        if extent.has_to()
            || extent.through_all
            || extent.distance2.is_some()
            || extent.direction == Direction::Symmetric
            || extent.taper.as_ref().is_some_and(|a| !self.eval(a, crate::expr::Kind::Angle).is_ok_and(|v| v == 0.))
        {
            return Err(DocError::Invalid("logical cap requires a one-sided untapered distance extrusion".into()));
        }
        let distance = self.eval(&extent.distance, crate::expr::Kind::Length)?;
        if !distance.is_finite() || distance.abs() < 1e-9 {
            return Err(DocError::Invalid("logical cap requires nonzero extrusion travel".into()));
        }
        Ok(end ^ (distance < 0.) ^ (extent.direction == Direction::Negative))
    }

    /// Convert an already certified picked cap, never selecting another face by proximity.
    pub fn track_extrusion_cap(&self, target: &ConfigFaceTarget, end: bool) -> Result<ConfigFaceTarget> {
        self.certify_config_face(target)?;
        let Some(FaceCertificate::ExtrudeCap { sketch, high, outer, holes }) = &target.certificate else {
            return Err(DocError::Invalid("selected face is not an exact extrusion cap".into()));
        };
        if *high != self.extrusion_cap_high(target.origin_feature, end)? {
            return Err(DocError::Invalid("selected face does not match the requested extrusion cap".into()));
        }
        let mut tracked = target.clone();
        tracked.name = format!("F{}:cap:{}:{}", target.origin_feature, if end { "end" } else { "start" }, cap_key(outer, holes));
        tracked.certificate = Some(FaceCertificate::ExtrudeTravelCap { sketch: *sketch, end, outer: outer.clone(), holes: holes.clone() });
        self.certify_config_face(&tracked)?;
        Ok(tracked)
    }

    pub(crate) fn resolve_travel_cap_target(&self, target: &ConfigFaceTarget) -> Result<Option<ConfigFaceTarget>> {
        let Some(FaceCertificate::ExtrudeTravelCap { sketch, end, outer, holes }) = &target.certificate else { return Ok(None) };
        let high = self.extrusion_cap_high(target.origin_feature, *end)?;
        let mut resolved = target.clone();
        resolved.name = format!("F{}:cap:{}:{}", target.origin_feature, if high { "high" } else { "low" }, cap_key(outer, holes));
        resolved.certificate = Some(FaceCertificate::ExtrudeCap { sketch: *sketch, high, outer: outer.clone(), holes: holes.clone() });
        Ok(Some(resolved))
    }
}
