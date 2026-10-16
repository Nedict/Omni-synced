//! Explicit, reported partial recovery. Strict import remains available to callers.
use super::*;
use serde::{Deserialize, Serialize};
use solidcraft_doc::{Document, Model};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportPreference {
    #[default]
    Editability,
    Correctness,
}

pub struct ImportOutcome {
    pub document: Document,
    pub warnings: Vec<String>,
    pub complete_history: bool,
    pub representation: &'static str,
}

impl Design {
    pub(super) fn recover_parameters(&self, parameters: &mut Vec<Parameter>) {
        let mut names = BTreeSet::new();
        parameters.retain(|p| {
            let valid = !p.name.is_empty() && names.insert(p.name.clone()) && solidcraft_doc::expr::unit_info(&p.display_unit).is_some();
            if !valid {
                self.warn(format!("Omitted parameter {}: duplicate name or unsupported unit", p.name));
            }
            valid
        });
        let mut check = Document::new("Parameter validation");
        check.params = parameters
            .iter()
            .map(|p| solidcraft_doc::Param {
                name: p.name.clone(),
                expr: p.expression.clone(),
                unit: p.display_unit.clone(),
                comment: p.comment.clone(),
                model: false,
            })
            .collect();
        let (values, errors) = check.param_values();
        for p in parameters {
            let kind = solidcraft_doc::expr::Kind::from_unit(&p.display_unit);
            let expected = p.value * if kind == solidcraft_doc::expr::Kind::Length { 10. } else { 1. };
            let matches =
                values.get(&p.name).and_then(|v| v.to_kind(kind).ok()).is_some_and(|v| (v - expected).abs() <= expected.abs().max(1.) * 1e-8);
            if p.driven || errors.contains_key(&p.name) || !matches {
                self.warn(format!("Parameter {}: replaced unavailable expression or measurement '{}' with its saved value", p.name, p.expression));
                let value = if kind == solidcraft_doc::expr::Kind::Angle { p.value.to_degrees() } else { expected };
                p.expression = format!("{value} {}", kind.default_unit());
                p.display_unit = kind.default_unit().into();
                p.driven = false;
            }
        }
    }

    pub(super) fn recovering(&self) -> bool {
        self.recovery.is_some()
    }

    pub(super) fn warn(&self, message: String) {
        if let Some(warnings) = &self.recovery {
            let mut warnings = warnings.borrow_mut();
            if warnings.len() < 1000 && !warnings.contains(&message) {
                warnings.push(message);
            } else if warnings.len() == 1000 {
                warnings.push("Additional import warnings omitted (report limit: 1000).".into());
            }
        }
    }

    pub(super) fn recover_records<T>(&self, label: &str, records: impl IntoIterator<Item = Result<T>>) -> Result<Vec<T>> {
        let mut result = Vec::new();
        for (index, record) in records.into_iter().enumerate() {
            match record {
                Ok(record) => result.push(record),
                Err(error) if self.recovering() => self.warn(format!("Omitted {label} record {}: {error}", index + 1)),
                Err(error) => return Err(error),
            }
        }
        Ok(result)
    }

    pub(super) fn recover_collection<T>(&self, label: &str, result: Result<Vec<T>>) -> Result<Vec<T>> {
        match result {
            Err(error) if self.recovering() => {
                self.warn(format!("Could not reconstruct {label}: {error}"));
                Ok(Vec::new())
            }
            result => result,
        }
    }
}

pub fn import_f3d_with_preference(bytes: &[u8], name: &str, preference: ImportPreference) -> Result<ImportOutcome> {
    // Validate the bounded container and indexes even when all features are unsupported.
    let mut designs = read_f3d(bytes)?;
    let mut warnings = Vec::new();
    if preference == ImportPreference::Correctness {
        #[cfg(not(target_arch = "wasm32"))]
        match super::native_bodies::recover(bytes, name) {
            Ok((mut document, warnings)) => {
                document.import_warnings = warnings.clone();
                return Ok(ImportOutcome { document, warnings, complete_history: false, representation: "native_bodies" });
            }
            Err(error) => warnings.push(format!("Native body recovery failed; falling back to partial history: {error}")),
        }
        #[cfg(target_arch = "wasm32")]
        warnings.push("Native body recovery is available in the desktop app. Falling back to partial history.".into());
    }
    let mut document = Document::new(name);
    for (index, design) in designs.iter_mut().enumerate() {
        design.recovery = Some(Default::default());
        if index != 0 {
            warnings.push(format!("Omitted additional design stream {}: assembly reconstruction is not supported yet", design.path));
            continue;
        }
        match design.to_document(name) {
            Ok(recovered) => document = recovered,
            Err(error) => warnings.push(format!("History could not be reconstructed: {error}")),
        }
        if let Some(report) = design.recovery.take() {
            warnings.extend(report.into_inner());
        }
    }
    let mut model = Model::new();
    model.evaluate(&document);
    if model.results.iter().any(|r| r.error.is_some()) {
        return Err(invalid("recovered history failed final validation"));
    }
    #[cfg(not(target_arch = "wasm32"))]
    let mut representation = "editable_history";
    #[cfg(target_arch = "wasm32")]
    let representation = "editable_history";
    #[cfg(not(target_arch = "wasm32"))]
    if preference == ImportPreference::Editability && model.state().bodies.is_empty() {
        match super::native_bodies::recover(bytes, name) {
            Ok((native, body_warnings)) => {
                let had_history = !document.features.is_empty();
                for feature in native.features {
                    let id = document.add_feature(feature.kind, Some(&feature.name))?;
                    if let Some(imported) = document.feature_mut(id) {
                        imported.body_names = feature.body_names;
                    }
                }
                representation = if had_history { "history_with_native_bodies" } else { "native_bodies" };
                warnings.push("No solid could be rebuilt from editable history; added recovered native bodies as a separate base feature.".into());
                warnings.extend(body_warnings);
            }
            Err(error) => warnings.push(format!("Native body fallback unavailable: {error}")),
        }
    }
    if document.features.is_empty() {
        warnings.push("No supported features could be recovered. This document is empty; the source archive is unchanged.".into());
    }
    let complete_history = warnings.is_empty();
    document.import_warnings = warnings.clone();
    Ok(ImportOutcome { document, warnings, complete_history, representation })
}

#[cfg(test)]
mod tests {
    use super::super::import_tests::{fixture, volume};
    use super::*;

    #[test]
    fn recovery_preserves_supported_history_and_analytic_volume() {
        let mut design = fixture(false).design();
        design.recovery = Some(Default::default());
        let doc = design.to_document("Cylinder").unwrap();
        volume(&doc, std::f64::consts::PI * 30. * 30. * 17.);
        assert!(design.recovery.as_ref().unwrap().borrow().is_empty());
    }

    #[test]
    fn unknown_root_identity_does_not_block_independently_decoded_features() {
        let mut design = fixture(false).design();
        design.classes.iter_mut().find(|c| c.objects.contains(&3)).unwrap().version = 17;
        assert!(design.to_document("Strict").is_err());
        design.recovery = Some(Default::default());
        let mut doc = design.to_document("Legacy").unwrap();
        assert_eq!(doc.features.len(), 2);
        volume(&doc, std::f64::consts::PI * 30. * 30. * 17.);
        doc.import_warnings.push("A source operation was omitted".into());
        let reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
        assert_eq!(reopened.import_warnings, doc.import_warnings);
    }

    #[test]
    fn unsupported_feature_does_not_discard_its_supported_sketch() {
        let mut fixture = fixture(false);
        fixture.0.get_mut(&9).unwrap().0 = "unsupported-feature".into();
        let mut design = fixture.design();
        assert!(design.to_document("Strict").is_err());
        design.recovery = Some(Default::default());
        let doc = design.to_document("Partial").unwrap();
        assert_eq!(doc.features.len(), 1);
        assert!(matches!(doc.features[0].kind, solidcraft_doc::FeatureKind::Sketch { .. }));
        assert!(design.recovery.as_ref().unwrap().borrow().iter().any(|w| w.contains("timeline feature")));
    }

    #[test]
    fn corrupt_archives_are_not_reported_as_successful_partial_imports() {
        for preference in [ImportPreference::Editability, ImportPreference::Correctness] {
            assert!(import_f3d_with_preference(b"not an archive", "Corrupt", preference).is_err());
        }
    }
}
