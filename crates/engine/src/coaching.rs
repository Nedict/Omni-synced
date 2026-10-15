//! Program-wide sketch coaching progress. It is application settings, not part of a design.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Onboarding progress kept by the host across sessions. Not stored in a `.solidcraft` file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CoachingPreference {
    /// The coaching layer is on. Missing saved data stays on.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Index of the current step (0-based).
    pub index: usize,
    /// The current step's callout is hidden; the layer stays enabled.
    pub dismissed: bool,
    /// The sequence has been completed at least once.
    pub completed: bool,
}

fn default_enabled() -> bool {
    true
}

impl Default for CoachingPreference {
    fn default() -> Self {
        CoachingPreference { enabled: true, index: 0, dismissed: false, completed: false }
    }
}

/// Read saved settings. A missing or malformed value is the first-run default.
pub fn coaching_from_value(v: &Value) -> CoachingPreference {
    if !v.is_object() {
        return CoachingPreference::default();
    }
    serde_json::from_value(v.clone()).unwrap_or_default()
}

/// Enable or disable the layer. Enabling again shows the current step.
pub fn coaching_set(pref: &mut CoachingPreference, enabled: bool) {
    pref.enabled = enabled;
    pref.dismissed = false;
}

/// Move to the next step. At the last step, mark the sequence complete and keep the index in range.
pub fn coaching_advance(pref: &mut CoachingPreference, steps: usize) {
    pref.dismissed = false;
    if steps == 0 {
        pref.index = 0;
        pref.completed = true;
        return;
    }
    let last = steps - 1;
    if pref.index >= last {
        pref.index = last;
        pref.completed = true;
    } else {
        pref.index += 1;
    }
}

/// Hide the current step's callout without turning the layer off.
pub fn coaching_dismiss(pref: &mut CoachingPreference) {
    pref.dismissed = true;
}

/// JSON the coaching commands return. `steps` is the table size supplied by the UI.
pub fn coaching_state(pref: &CoachingPreference, steps: usize) -> Value {
    let index = if steps == 0 { 0 } else { pref.index.min(steps - 1) };
    json!({
        "enabled": pref.enabled,
        "index": index,
        "dismissed": pref.dismissed,
        "completed": pref.completed,
        "steps": steps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feature: ux-ui-shell-improvements, Property 12: Coaching advances monotonically and completes at the end
    #[test]
    fn property_12_coaching_advances_monotonically_and_completes_at_the_end() {
        let mut cases = 0usize;
        for steps in 1..=8 {
            for start in 0..12 {
                for enabled in [false, true] {
                    for dismissed in [false, true] {
                        for completed in [false, true] {
                            let mut pref = CoachingPreference { enabled, index: start, dismissed, completed };
                            coaching_advance(&mut pref, steps);
                            let last = steps - 1;
                            if start >= last {
                                assert!(pref.completed, "past the last step completes");
                                assert_eq!(pref.index, last);
                            } else {
                                assert_eq!(pref.index, start + 1);
                            }
                            assert!(pref.index < steps, "index stays inside the table");
                            assert!(!pref.dismissed, "advance shows the step");
                            cases += 1;
                        }
                    }
                }
            }
        }
        assert!(cases >= 100, "property ran {cases} times");
    }

    /// Feature: ux-ui-shell-improvements, Property 13: Dismissing a step keeps the layer enabled
    #[test]
    fn property_13_dismissing_a_step_keeps_the_layer_enabled() {
        let mut cases = 0usize;
        for enabled in [false, true] {
            for index in 0..30 {
                for completed in [false, true] {
                    let mut pref = CoachingPreference { enabled, index, dismissed: false, completed };
                    coaching_dismiss(&mut pref);
                    assert_eq!(pref.enabled, enabled, "dismiss does not change enabled");
                    assert!(pref.dismissed);
                    assert_eq!(pref.index, index);
                    assert_eq!(pref.completed, completed);
                    cases += 1;
                }
            }
        }
        assert!(cases >= 100, "property ran {cases} times");
    }

    /// Feature: ux-ui-shell-improvements, Property 14: Coaching preference persists across sessions
    #[test]
    fn property_14_coaching_preference_persists_across_sessions() {
        let mut cases = 0usize;
        for steps in [1usize, 3, 6] {
            for mask in 0..64 {
                let mut pref = CoachingPreference::default();
                if mask & 1 != 0 {
                    coaching_set(&mut pref, false);
                }
                if mask & 2 != 0 {
                    coaching_advance(&mut pref, steps);
                }
                if mask & 4 != 0 {
                    coaching_dismiss(&mut pref);
                }
                if mask & 8 != 0 {
                    coaching_set(&mut pref, true);
                }
                if mask & 16 != 0 {
                    coaching_advance(&mut pref, steps);
                    coaching_advance(&mut pref, steps);
                }
                if mask & 32 != 0 {
                    coaching_set(&mut pref, false);
                }
                let saved = serde_json::to_value(&pref).expect("serialize");
                let loaded = coaching_from_value(&saved);
                assert_eq!(loaded, pref);
                if !pref.enabled {
                    let again = coaching_from_value(&serde_json::to_value(&loaded).expect("serialize again"));
                    assert!(!again.enabled, "a disabled preference stays disabled");
                }
                cases += 1;
            }
        }
        assert!(cases >= 100, "property ran {cases} times");
        assert_eq!(coaching_from_value(&Value::Null), CoachingPreference::default());
        assert_eq!(coaching_from_value(&json!("nope")), CoachingPreference::default());
        assert_eq!(coaching_from_value(&json!({"enabled": "yes"})), CoachingPreference::default());
    }
}
