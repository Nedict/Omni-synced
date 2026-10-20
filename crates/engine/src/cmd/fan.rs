//! Ducted-fan analysis: `analysis.ducted_fan` estimates the thrust, torque and power of one or
//! more rotor stages on one shaft inside a duct, from the blade geometry, by blade-element
//! theory matched to the duct's exit.
//!
//! The model (an incompressible fan; every stage passes the same volume of air):
//! - Each rotor is cut into radial stations. At a station the blade meets the air at the angle
//!   `φ = atan(Va / (Ω r − Vθ_in))` and works at `α = β(r) − φ`, where `β` is the blade angle
//!   from the plane of rotation (`beta_deg` at 75 % radius, twisted for constant pitch unless
//!   `twist: "none"`). A thin cambered-plate polar (`polar`) gives lift and drag; the blade's
//!   axial force is the stage's thrust and its tangential force the torque and the swirl it
//!   leaves (`ΔVθ = B f_t / (ρ Va 2π r)`).
//! - A stage's static-pressure rise is its blade thrust over its annulus area. The swirl's
//!   kinetic energy came out of the same shaft work: stators behind a rotor (`stators`,
//!   default on) give `stator_recovery` of it back as pressure and the rest is lost; without
//!   them the swirl stays in the air, the next rotor sees it, and it leaves in the jet.
//! - The duct fixes the flow: the exit (`exit_diameter`, default the last annulus as an open
//!   disk) must carry the same mass flow, so `½ρ(Ve² + Vθe²) = ½ρV∞² + ΣΔp − K½ρVa²`
//!   (`duct_loss` K). The axial velocity that satisfies this is found by bisection; thrust is
//!   the momentum the jet gains, `ṁ (Ve − V∞)`. The difference between that and the blades'
//!   own thrust is what the duct and nozzle carry. No induced loss beyond the swirl is
//!   modelled (a duct removes the tip loss), so the figures lean optimistic.
//! - An electric motor (`motor`: kv, resistance, no-load current) on the shaft answers current
//!   and voltage at a given rpm, or, given `voltage` and no `rpm`, the rpm where its torque
//!   meets the fan's.
//!
//! The numbers are estimates for sizing and comparing designs (which β, how many stages, what
//! nozzle); a thrust stand or CFD settles them. Blade sections and polars are the biggest
//! unknown: a measured polar replaces the default by its coefficients.

use std::f64::consts::PI;

use serde_json::{Value, json};

use super::CommandSpec;
use crate::params::{bad, bool_, str_};
use crate::{Result, Session};

pub static COMMANDS: &[CommandSpec] = &[CommandSpec::new("analysis.ducted_fan", "Ducted Fan Analysis", ducted_fan).icon("graph").noundo().doc_only().params(
    "diameter: mm (rotor 1 tip); hub_diameter?: mm (default 0.3·diameter); blades? (per rotor, default 2); chord?: mm or [root, tip] (default 0.12·diameter); \
     beta_deg: blade angle from the plane of rotation at 75 % radius, a number or one per stage; stages?: [{beta_deg, diameter?, blades?, chord?}] instead; \
     twist?: constant_pitch|none; stators?: bool (default true, a vane row after each rotor), stator_recovery? (0…1, default 0.7); \
     exit_diameter?: mm (nozzle exit; default the last annulus area); duct_loss? (K·½ρVa², default 0.15); airspeed?: m/s (default 0, static); \
     air_density? (kg/m³, default 1.225); polar?: {cl_alpha (per rad), alpha0_deg, cd0, k, alpha_stall_deg}; stations? (4…200); \
     rpm: number, or rpm_sweep: [min, max, count ≤ 200]; motor?: {kv (rpm/V), resistance (Ω), no_load_current? (A), voltage? (V; with no rpm: the rpm where motor and fan torque meet)} \
     → thrust_n, blade_thrust_n, duct_thrust_n, torque_nm, shaft_power_w, mass_flow_kg_s, exit_velocity_m_s, pressure_rise_pa, figure_of_merit (static) or efficiency, \
     stages: [{diameter_mm, axial_velocity_m_s, flow_coefficient, thrust_n, torque_nm, pressure_rise_pa, alpha_deg: {root, mid, tip}, stalled_fraction}], \
     motor?: {rpm, current_a, voltage_v, electrical_power_w, efficiency}, warnings; a sweep answers points: [one per rpm]. An estimate for sizing (see the module notes); the design is not changed",
)];

const MAX_STAGES: usize = 20;
const MAX_STATIONS: usize = 200;
const MAX_SWEEP: usize = 200;

/// A thin cambered-plate polar with a stall.
#[derive(Clone, Copy, Debug)]
struct Polar {
    /// Lift slope per radian.
    cl_alpha: f64,
    /// Zero-lift angle (radians, negative for camber).
    alpha0: f64,
    /// Profile drag at zero lift and the induced/profile factor: `cd = cd0 + k·cl²`.
    cd0: f64,
    k: f64,
    /// Where the lift stops growing (radians from `alpha0`).
    alpha_stall: f64,
}

impl Default for Polar {
    fn default() -> Self {
        Polar { cl_alpha: 2.0 * PI * 0.9, alpha0: (-2f64).to_radians(), cd0: 0.015, k: 0.03, alpha_stall: 12f64.to_radians() }
    }
}

impl Polar {
    /// Lift and drag coefficients at `alpha` (radians) and whether the section is stalled.
    fn at(&self, alpha: f64) -> (f64, f64, bool) {
        let a = alpha - self.alpha0;
        let cl_max = self.cl_alpha * self.alpha_stall;
        if a.abs() <= self.alpha_stall {
            let cl = self.cl_alpha * a;
            return (cl, self.cd0 + self.k * cl * cl, false);
        }
        // Past the stall the lift falls towards the flat plate's `2 sin α cos α` and the drag
        // rises towards its `2 sin² α`, over 20°.
        let over = ((a.abs() - self.alpha_stall) / 20f64.to_radians()).min(1.0);
        let plate_cl = 2.0 * a.sin() * a.cos();
        let cl = (cl_max * (1.0 - over) + plate_cl.abs() * over) * a.signum();
        let cd_att = self.cd0 + self.k * cl_max * cl_max;
        let cd = cd_att * (1.0 - over) + (2.0 * a.sin() * a.sin()).max(cd_att) * over;
        (cl, cd, true)
    }
}

#[derive(Clone, Debug)]
struct Stage {
    radius: f64,
    blades: f64,
    /// Chord at the root and at the tip (metres).
    chord: (f64, f64),
    /// Blade angle from the plane of rotation at 75 % radius (radians).
    beta_ref: f64,
}

#[derive(Clone, Debug)]
struct Fan {
    hub_radius: f64,
    stages: Vec<Stage>,
    constant_pitch: bool,
    stators: Option<f64>,
    exit_area: f64,
    duct_loss: f64,
    airspeed: f64,
    rho: f64,
    polar: Polar,
    stations: usize,
}

#[derive(Clone, Debug)]
struct Motor {
    kv: f64,
    resistance: f64,
    no_load_current: f64,
    voltage: Option<f64>,
}

#[derive(Clone, Debug, Default)]
struct StageResult {
    thrust: f64,
    torque: f64,
    axial_velocity: f64,
    flow_coefficient: f64,
    pressure_rise: f64,
    alpha: [f64; 3],
    stalled_fraction: f64,
}

#[derive(Clone, Debug, Default)]
struct Operating {
    mass_flow: f64,
    exit_velocity: f64,
    exit_swirl: f64,
    pressure_rise: f64,
    blade_thrust: f64,
    thrust: f64,
    torque: f64,
    power: f64,
    stages: Vec<StageResult>,
    /// The swirl had to be clamped: not a flow the fan can hold.
    unphysical: bool,
}

impl Fan {
    fn annulus_area(&self, stage: &Stage) -> f64 {
        PI * (stage.radius * stage.radius - self.hub_radius * self.hub_radius)
    }

    /// The blades' work at `rpm` with `va1` (m/s) through the first annulus: the stages, the
    /// total-pressure rise the air gains (blade work, stator recovery) and the swirl it keeps.
    fn blades(&self, rpm: f64, va1: f64) -> Operating {
        let omega = rpm * 2.0 * PI / 60.0;
        let a1 = self.stages.first().map(|s| self.annulus_area(s)).unwrap_or(1.0);
        let n = self.stations;
        // Swirl the air carries into the next rotor, per normalised station.
        let mut swirl = vec![0.0; n];
        let mut out = Operating { mass_flow: self.rho * va1 * a1, ..Default::default() };
        for stage in &self.stages {
            let area = self.annulus_area(stage);
            let va = va1 * a1 / area;
            let span = stage.radius - self.hub_radius;
            let dr = span / n as f64;
            let r_ref = 0.75 * stage.radius;
            let tan_ref = stage.beta_ref.tan();
            let mut res = StageResult { axial_velocity: va, flow_coefficient: va / (omega * stage.radius).max(1e-9), ..Default::default() };
            let mut stalled = 0usize;
            let mut swirl_energy = 0.0;
            let mut alphas = Vec::with_capacity(n);
            for (j, sw) in swirl.iter_mut().enumerate() {
                let r = self.hub_radius + (j as f64 + 0.5) * dr;
                let t = (r - self.hub_radius) / span;
                let chord = stage.chord.0 + (stage.chord.1 - stage.chord.0) * t;
                let beta = if self.constant_pitch { (tan_ref * r_ref / r).atan() } else { stage.beta_ref };
                let wt = omega * r - *sw;
                let phi = va.atan2(wt);
                let alpha = beta - phi;
                let (cl, cd, is_stalled) = self.polar.at(alpha);
                stalled += usize::from(is_stalled);
                alphas.push(alpha);
                let q = 0.5 * self.rho * (va * va + wt * wt);
                let lift = q * chord * cl;
                let drag = q * chord * cd;
                let fa = lift * phi.cos() - drag * phi.sin();
                let ft = lift * phi.sin() + drag * phi.cos();
                res.thrust += stage.blades * fa * dr;
                res.torque += stage.blades * ft * r * dr;
                // The air cannot swirl faster than the blade that drives it: a flow so small
                // that it would is no operating point (see `operate`).
                let dv = stage.blades * ft / (self.rho * va.max(1e-3) * 2.0 * PI * r);
                *sw += dv;
                if sw.abs() > omega * r {
                    out.unphysical = true;
                    *sw = sw.clamp(-omega * r, omega * r);
                }
                swirl_energy += 0.5 * self.rho * *sw * *sw * 2.0 * PI * r * dr;
            }
            // The blades' axial force is the static pressure rise. The swirl's kinetic energy
            // came out of the same shaft work: a stator row gives `recovery` of it back as
            // pressure and the rest is lost; without stators it leaves in the jet (`exit_swirl`).
            res.pressure_rise = res.thrust / area;
            res.alpha = [alphas.first().copied().unwrap_or(0.0), alphas.get(n / 2).copied().unwrap_or(0.0), alphas.last().copied().unwrap_or(0.0)];
            res.stalled_fraction = stalled as f64 / n as f64;
            out.blade_thrust += res.thrust;
            out.torque += res.torque;
            if let Some(recovery) = self.stators {
                res.pressure_rise -= (1.0 - recovery) * swirl_energy / area;
                swirl.iter_mut().for_each(|s| *s = 0.0);
            }
            out.pressure_rise += res.pressure_rise;
            out.stages.push(res);
        }
        // The swirl that leaves, as one mean value (kinetic energy preserving).
        if let (None, Some(last)) = (self.stators, self.stages.last()) {
            let area = self.annulus_area(last);
            let span = last.radius - self.hub_radius;
            let dr = span / n as f64;
            let energy: f64 =
                swirl.iter().enumerate().map(|(j, s)| 0.5 * self.rho * s * s * 2.0 * PI * (self.hub_radius + (j as f64 + 0.5) * dr) * dr).sum();
            out.exit_swirl = (2.0 * energy / (self.rho * area)).max(0.0).sqrt();
        }
        out.power = out.torque * omega;
        out
    }

    /// What the exit needs minus what the blades give, at `va1`: positive while the blades can
    /// push more air than the duct lets out.
    fn balance(&self, rpm: f64, va1: f64) -> (f64, Operating) {
        let mut op = self.blades(rpm, va1);
        let ve = op.mass_flow / (self.rho * self.exit_area);
        op.exit_velocity = ve;
        let available = 0.5 * self.rho * self.airspeed * self.airspeed + op.pressure_rise - self.duct_loss * 0.5 * self.rho * va1 * va1;
        let required = 0.5 * self.rho * (ve * ve + op.exit_swirl * op.exit_swirl);
        let g = if op.unphysical { f64::NAN } else { available - required };
        (g, op)
    }

    /// The operating point at `rpm`: the flow the duct passes with these blades.
    fn operate(&self, rpm: f64) -> Option<Operating> {
        // The balance is positive where the blades push more than the exit passes and falls
        // through zero at the operating point; a stalled fan at very low flow may start
        // negative, so look for the last positive sample first.
        const SAMPLES: usize = 64;
        let grid = |i: usize| 1e-2 * (400.0f64 / 1e-2).powf(i as f64 / (SAMPLES - 1) as f64);
        let mut bracket = None;
        for i in 0..SAMPLES - 1 {
            let (a, b) = (self.balance(rpm, grid(i)).0, self.balance(rpm, grid(i + 1)).0);
            if a.is_finite() && b.is_finite() && a > 0.0 && b <= 0.0 {
                bracket = Some((grid(i), grid(i + 1)));
            }
        }
        let (mut lo, mut hi) = bracket?;
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            let (g, _) = self.balance(rpm, mid);
            // Not a flow the fan can hold (the swirl clamp): too little air, look higher.
            if !g.is_finite() || g > 0.0 {
                lo = mid
            } else {
                hi = mid
            }
            if hi - lo < 1e-7 {
                break;
            }
        }
        let (_, mut op) = self.balance(rpm, 0.5 * (lo + hi));
        op.thrust = op.mass_flow * (op.exit_velocity - self.airspeed);
        Some(op)
    }
}

impl Motor {
    /// Torque constant, N·m per ampere.
    fn kt(&self) -> f64 {
        60.0 / (2.0 * PI * self.kv)
    }
    /// Current and voltage that turn the shaft at `rpm` against `torque`.
    fn drive(&self, rpm: f64, torque: f64) -> (f64, f64) {
        let current = torque / self.kt() + self.no_load_current;
        (current, rpm / self.kv + current * self.resistance)
    }
    /// The torque the motor gives at `rpm` on `voltage`.
    fn torque_at(&self, rpm: f64, voltage: f64) -> f64 {
        ((voltage - rpm / self.kv) / self.resistance - self.no_load_current) * self.kt()
    }
}

fn point_json(fan: &Fan, rpm: f64, op: &Operating, motor: Option<&Motor>) -> Value {
    let mut warnings = Vec::new();
    for (i, s) in op.stages.iter().enumerate() {
        if s.stalled_fraction > 0.25 {
            warnings.push(format!("stage {} is stalled over {:.0} % of its span (lower beta or raise the flow)", i + 1, s.stalled_fraction * 100.0));
        }
        if s.alpha[1] < (-4f64).to_radians() {
            warnings.push(format!(
                "stage {} works at a negative angle of attack mid-span ({:.1}°): it brakes the air (raise beta)",
                i + 1,
                s.alpha[1].to_degrees()
            ));
        }
    }
    if op.thrust < op.blade_thrust * 0.8 {
        warnings.push("the duct and nozzle carry a large negative thrust: the exit is small for this flow".into());
    }
    // Against the ideal ducted actuator with the same exit area: `P = T^1.5 / (2 √(ρ A_e))`.
    let ideal_static = if fan.airspeed <= 1e-9 && op.power > 0.0 && op.thrust > 0.0 {
        Some(op.thrust.powf(1.5) / (2.0 * (fan.rho * fan.exit_area).sqrt()) / op.power)
    } else {
        None
    };
    let efficiency = if fan.airspeed > 1e-9 && op.power > 0.0 { Some(op.thrust * fan.airspeed / op.power) } else { None };
    let motor_json = motor.map(|m| {
        let (current, voltage) = m.drive(rpm, op.torque);
        let electrical = current * voltage;
        json!({"rpm": rpm, "current_a": current, "voltage_v": voltage, "electrical_power_w": electrical,
            "efficiency": if electrical > 0.0 { op.power / electrical } else { 0.0 }})
    });
    json!({
        "rpm": rpm,
        "airspeed_m_s": fan.airspeed,
        "thrust_n": op.thrust,
        "blade_thrust_n": op.blade_thrust,
        "duct_thrust_n": op.thrust - op.blade_thrust,
        "torque_nm": op.torque,
        "shaft_power_w": op.power,
        "mass_flow_kg_s": op.mass_flow,
        "exit_velocity_m_s": op.exit_velocity,
        "exit_swirl_m_s": op.exit_swirl,
        "pressure_rise_pa": op.pressure_rise,
        "figure_of_merit": ideal_static,
        "efficiency": efficiency,
        "stages": op.stages.iter().zip(&fan.stages).map(|(s, g)| json!({
            "diameter_mm": g.radius * 2000.0,
            "axial_velocity_m_s": s.axial_velocity,
            "flow_coefficient": s.flow_coefficient,
            "thrust_n": s.thrust,
            "torque_nm": s.torque,
            "pressure_rise_pa": s.pressure_rise,
            "alpha_deg": {"root": s.alpha[0].to_degrees(), "mid": s.alpha[1].to_degrees(), "tip": s.alpha[2].to_degrees()},
            "stalled_fraction": s.stalled_fraction,
        })).collect::<Vec<_>>(),
        "motor": motor_json,
        "warnings": warnings,
    })
}

/// A finite number in `lo..=hi`.
fn bounded(p: &Value, k: &str, lo: f64, hi: f64, cmd: &str) -> Result<Option<f64>> {
    match p.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_f64()
            .filter(|x| x.is_finite() && *x >= lo && *x <= hi)
            .map(Some)
            .ok_or_else(|| bad(cmd, format!("`{k}` must be a number {lo}…{hi}"))),
    }
}

fn parse_fan(p: &Value, cmd: &str) -> Result<Fan> {
    let diameter = bounded(p, "diameter", 1.0, 20_000.0, cmd)?.ok_or_else(|| bad(cmd, "`diameter` (mm) is required"))?;
    let hub = bounded(p, "hub_diameter", 0.0, 20_000.0, cmd)?.unwrap_or(0.3 * diameter);
    let blades = bounded(p, "blades", 1.0, 100.0, cmd)?.unwrap_or(2.0).round();
    let chord_of = |v: Option<&Value>, d: f64| -> Result<(f64, f64)> {
        let ok = |x: f64| x.is_finite() && x > 0.0 && x <= d;
        match v {
            None | Some(Value::Null) => Ok((0.12 * d, 0.12 * d)),
            Some(Value::Number(n)) => {
                n.as_f64().filter(|x| ok(*x)).map(|c| (c, c)).ok_or_else(|| bad(cmd, "`chord` must be a positive length up to the diameter"))
            }
            Some(Value::Array(a)) if a.len() == 2 => {
                let r = a.first().and_then(Value::as_f64).filter(|x| ok(*x));
                let t = a.get(1).and_then(Value::as_f64).filter(|x| ok(*x));
                r.zip(t).ok_or_else(|| bad(cmd, "`chord` must be [root, tip] positive lengths"))
            }
            _ => Err(bad(cmd, "`chord` must be a length or [root, tip]")),
        }
    };
    let chord = chord_of(p.get("chord"), diameter)?;
    let beta_of = |v: &Value| {
        v.as_f64()
            .filter(|b| b.is_finite() && *b > 0.0 && *b < 90.0)
            .map(f64::to_radians)
            .ok_or_else(|| bad(cmd, "`beta_deg` must be an angle between 0 and 90 degrees"))
    };
    let mut stages = Vec::new();
    match (p.get("stages"), p.get("beta_deg")) {
        (Some(Value::Array(list)), _) => {
            if list.is_empty() || list.len() > MAX_STAGES {
                return Err(bad(cmd, format!("`stages` must list 1…{MAX_STAGES} stages")));
            }
            for st in list {
                let d = bounded(st, "diameter", 1.0, 20_000.0, cmd)?.unwrap_or(diameter);
                stages.push(Stage {
                    radius: d / 2000.0,
                    blades: bounded(st, "blades", 1.0, 100.0, cmd)?.unwrap_or(blades).round(),
                    chord: {
                        // A stage without its own chord takes the fan's.
                        let (r, t) = chord_of(st.get("chord").or_else(|| p.get("chord")), d)?;
                        (r / 1000.0, t / 1000.0)
                    },
                    beta_ref: beta_of(st.get("beta_deg").ok_or_else(|| bad(cmd, "each stage needs `beta_deg`"))?)?,
                });
            }
        }
        (Some(_), _) => return Err(bad(cmd, "`stages` must be a list")),
        (None, Some(Value::Array(list))) => {
            if list.is_empty() || list.len() > MAX_STAGES {
                return Err(bad(cmd, format!("`beta_deg` must list 1…{MAX_STAGES} angles")));
            }
            for b in list {
                stages.push(Stage { radius: diameter / 2000.0, blades, chord: (chord.0 / 1000.0, chord.1 / 1000.0), beta_ref: beta_of(b)? });
            }
        }
        (None, Some(b)) => {
            stages.push(Stage { radius: diameter / 2000.0, blades, chord: (chord.0 / 1000.0, chord.1 / 1000.0), beta_ref: beta_of(b)? })
        }
        (None, None) => return Err(bad(cmd, "`beta_deg` (or `stages`) is required")),
    }
    let hub_radius = hub / 2000.0;
    if let Some(s) = stages.iter().find(|s| s.radius <= hub_radius * 1.01) {
        return Err(bad(cmd, format!("the hub ({hub} mm) leaves no blade on a {:.1} mm rotor", s.radius * 2000.0)));
    }
    let constant_pitch = match str_(p, "twist").map(str::to_ascii_lowercase).as_deref() {
        None | Some("constant_pitch") => true,
        Some("none") => false,
        Some(o) => return Err(bad(cmd, format!("unknown twist `{o}` (constant_pitch or none)"))),
    };
    let stators = if bool_(p, "stators").unwrap_or(true) { Some(bounded(p, "stator_recovery", 0.0, 1.0, cmd)?.unwrap_or(0.7)) } else { None };
    let last = stages.last().map(|s| s.radius).unwrap_or(hub_radius);
    let exit_area = match bounded(p, "exit_diameter", 1.0, 20_000.0, cmd)? {
        Some(d) => PI * (d / 2000.0) * (d / 2000.0),
        None => PI * (last * last - hub_radius * hub_radius),
    };
    let polar = match p.get("polar") {
        None | Some(Value::Null) => Polar::default(),
        Some(pp) if pp.is_object() => {
            let d = Polar::default();
            Polar {
                cl_alpha: bounded(pp, "cl_alpha", 0.1, 20.0, cmd)?.unwrap_or(d.cl_alpha),
                alpha0: bounded(pp, "alpha0_deg", -20.0, 20.0, cmd)?.map(f64::to_radians).unwrap_or(d.alpha0),
                cd0: bounded(pp, "cd0", 0.0, 2.0, cmd)?.unwrap_or(d.cd0),
                k: bounded(pp, "k", 0.0, 2.0, cmd)?.unwrap_or(d.k),
                alpha_stall: bounded(pp, "alpha_stall_deg", 1.0, 45.0, cmd)?.map(f64::to_radians).unwrap_or(d.alpha_stall),
            }
        }
        Some(_) => return Err(bad(cmd, "`polar` must be an object")),
    };
    Ok(Fan {
        hub_radius,
        stages,
        constant_pitch,
        stators,
        exit_area,
        duct_loss: bounded(p, "duct_loss", 0.0, 10.0, cmd)?.unwrap_or(0.15),
        airspeed: bounded(p, "airspeed", 0.0, 340.0, cmd)?.unwrap_or(0.0),
        rho: bounded(p, "air_density", 0.01, 20.0, cmd)?.unwrap_or(1.225),
        polar,
        stations: bounded(p, "stations", 4.0, MAX_STATIONS as f64, cmd)?.map(|n| n.round() as usize).unwrap_or(24),
    })
}

fn parse_motor(p: &Value, cmd: &str) -> Result<Option<Motor>> {
    match p.get("motor") {
        None | Some(Value::Null) => Ok(None),
        Some(m) if m.is_object() => Ok(Some(Motor {
            kv: bounded(m, "kv", 1.0, 100_000.0, cmd)?.ok_or_else(|| bad(cmd, "`motor.kv` (rpm per volt) is required"))?,
            resistance: bounded(m, "resistance", 1e-4, 1000.0, cmd)?.ok_or_else(|| bad(cmd, "`motor.resistance` (ohm) is required"))?,
            no_load_current: bounded(m, "no_load_current", 0.0, 1000.0, cmd)?.unwrap_or(0.0),
            voltage: bounded(m, "voltage", 0.1, 10_000.0, cmd)?,
        })),
        Some(_) => Err(bad(cmd, "`motor` must be an object")),
    }
}

fn ducted_fan(_s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "analysis.ducted_fan";
    let fan = parse_fan(p, cmd)?;
    let motor = parse_motor(p, cmd)?;
    let no_flow = || bad(cmd, "the blades move no air at this setting (beta too low, or the exit too small)");
    if let Some(Value::Array(sweep)) = p.get("rpm_sweep") {
        let get = |i: usize| sweep.get(i).and_then(Value::as_f64).filter(|x| x.is_finite());
        let (lo, hi, count) =
            get(0).zip(get(1)).zip(get(2)).map(|((a, b), c)| (a, b, c)).ok_or_else(|| bad(cmd, "`rpm_sweep` must be [min, max, count]"))?;
        if !(1.0..=500_000.0).contains(&lo) || !(lo..=500_000.0).contains(&hi) || !(2.0..=MAX_SWEEP as f64).contains(&count) {
            return Err(bad(cmd, format!("`rpm_sweep` must be [min ≥ 1, max ≥ min, 2…{MAX_SWEEP} points]")));
        }
        let count = count.round() as usize;
        let mut points = Vec::with_capacity(count);
        for i in 0..count {
            let rpm = lo + (hi - lo) * i as f64 / (count - 1) as f64;
            if let Some(op) = fan.operate(rpm) {
                points.push(point_json(&fan, rpm, &op, motor.as_ref()));
            }
        }
        if points.is_empty() {
            return Err(no_flow());
        }
        return Ok(json!({"sweep": "rpm", "points": points}));
    }
    let rpm = match (bounded(p, "rpm", 1.0, 500_000.0, cmd)?, motor.as_ref().and_then(|m| m.voltage)) {
        (Some(r), _) => r,
        (None, Some(v)) => {
            // Motor torque falls with rpm, fan torque rises: bisect where they meet.
            let Some(m) = motor.as_ref() else { return Err(no_flow()) };
            let (mut lo, mut hi) = (1.0, (m.kv * v).max(2.0));
            for _ in 0..60 {
                let mid = 0.5 * (lo + hi);
                let fan_torque = fan.operate(mid).map(|op| op.torque).unwrap_or(0.0);
                if m.torque_at(mid, v) > fan_torque { lo = mid } else { hi = mid }
            }
            0.5 * (lo + hi)
        }
        (None, None) => return Err(bad(cmd, "`rpm`, `rpm_sweep` or `motor.voltage` is required")),
    };
    let op = fan.operate(rpm).ok_or_else(no_flow)?;
    Ok(point_json(&fan, rpm, &op, motor.as_ref()))
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use crate::Session;

    fn run(p: Value) -> Value {
        let mut s = Session::default();
        match s.execute("analysis.ducted_fan", &p) {
            Ok(v) => v,
            Err(e) => panic!("{p}: {e}"),
        }
    }

    fn f(v: &Value, k: &str) -> f64 {
        v[k].as_f64().unwrap_or(f64::NAN)
    }

    #[test]
    fn static_thrust_stays_within_momentum_theory() {
        // Zero-drag blades, perfect stators, no duct loss: the figure of merit must not beat the
        // ideal actuator disk, and a lightly loaded fan should get most of the way there.
        let r = run(json!({"diameter": 100, "hub_diameter": 30, "blades": 4, "beta_deg": 25, "rpm": 12000,
            "duct_loss": 0, "stator_recovery": 1, "polar": {"cd0": 0, "k": 0}}));
        let fm = f(&r, "figure_of_merit");
        assert!(fm > 0.6 && fm <= 1.0 + 1e-9, "{r:#}");
        assert!(f(&r, "thrust_n") > 0.0 && f(&r, "shaft_power_w") > 0.0);
        // The jet carries what the blades push plus what the duct adds; both finite.
        assert!((f(&r, "blade_thrust_n") + f(&r, "duct_thrust_n") - f(&r, "thrust_n")).abs() < 1e-9);
    }

    #[test]
    fn thrust_and_power_follow_fan_laws() {
        let at = |rpm: f64| run(json!({"diameter": 96, "hub_diameter": 34, "blades": 6, "chord": 12, "beta_deg": [30, 34, 38], "rpm": rpm}));
        let (a, b) = (at(8000.0), at(16000.0));
        let t = f(&b, "thrust_n") / f(&a, "thrust_n");
        let p = f(&b, "shaft_power_w") / f(&a, "shaft_power_w");
        assert!((t - 4.0).abs() < 0.25, "thrust ratio {t}");
        assert!((p - 8.0).abs() < 0.6, "power ratio {p}");
        assert_eq!(b["stages"].as_array().map(Vec::len), Some(3));
        assert!(b["warnings"].as_array().map(Vec::is_empty).unwrap_or(false), "{b:#}");
    }

    #[test]
    fn more_stages_more_pressure_and_a_nozzle_speeds_the_jet() {
        let one = run(json!({"diameter": 96, "hub_diameter": 34, "blades": 6, "beta_deg": 30, "rpm": 15000}));
        let three = run(json!({"diameter": 96, "hub_diameter": 34, "blades": 6, "beta_deg": [30, 34, 38], "rpm": 15000}));
        // Three stages give well over one stage's pressure, though less than three times it:
        // the faster flow they drive lowers every blade's angle of attack.
        assert!(
            f(&three, "pressure_rise_pa") > 1.5 * f(&one, "pressure_rise_pa"),
            "{} {}",
            f(&one, "pressure_rise_pa"),
            f(&three, "pressure_rise_pa")
        );
        assert!(f(&three, "thrust_n") > f(&one, "thrust_n"));
        let nozzle = run(json!({"diameter": 96, "hub_diameter": 34, "blades": 6, "beta_deg": [30, 34, 38], "rpm": 15000, "exit_diameter": 73}));
        assert!(f(&nozzle, "exit_velocity_m_s") > f(&three, "exit_velocity_m_s"));
        assert!(f(&nozzle, "mass_flow_kg_s") < f(&three, "mass_flow_kg_s"));
    }

    #[test]
    fn stages_take_the_fan_chord_and_wider_blades_push_harder() {
        let narrow = run(
            json!({"diameter": 96, "hub_diameter": 34, "blades": 6, "chord": 10, "stages": [{"beta_deg": 30}, {"beta_deg": 34, "diameter": 91}], "rpm": 15000}),
        );
        let wide = run(
            json!({"diameter": 96, "hub_diameter": 34, "blades": 6, "chord": 18, "stages": [{"beta_deg": 30}, {"beta_deg": 34, "diameter": 91}], "rpm": 15000}),
        );
        let own = run(
            json!({"diameter": 96, "hub_diameter": 34, "blades": 6, "chord": 10, "stages": [{"beta_deg": 30, "chord": 18}, {"beta_deg": 34, "diameter": 91, "chord": 18}], "rpm": 15000}),
        );
        // Not in proportion: the wider blades also drive more flow, which lowers their angle of attack.
        assert!(f(&wide, "thrust_n") > 1.1 * f(&narrow, "thrust_n"), "{} {}", f(&narrow, "thrust_n"), f(&wide, "thrust_n"));
        assert!((f(&own, "thrust_n") - f(&wide, "thrust_n")).abs() < 1e-9);
    }

    #[test]
    fn stators_recover_swirl() {
        let with = run(json!({"diameter": 96, "hub_diameter": 34, "blades": 6, "beta_deg": [35, 35, 35], "rpm": 15000}));
        let without = run(json!({"diameter": 96, "hub_diameter": 34, "blades": 6, "beta_deg": [35, 35, 35], "rpm": 15000, "stators": false}));
        assert!(f(&without, "exit_swirl_m_s") > 1.0 && f(&with, "exit_swirl_m_s") == 0.0);
        assert!(f(&with, "thrust_n") > f(&without, "thrust_n"), "{} {}", f(&with, "thrust_n"), f(&without, "thrust_n"));
    }

    #[test]
    fn stall_is_reported() {
        let r = run(json!({"diameter": 96, "hub_diameter": 34, "blades": 6, "beta_deg": 55, "rpm": 15000}));
        assert!(f(&r["stages"][0], "stalled_fraction") > 0.5, "{r:#}");
        assert!(r["warnings"].as_array().map(|w| !w.is_empty()).unwrap_or(false));
        // Flat to the flow, the blades move no air at all: an error, not a number.
        let mut s = Session::default();
        assert!(s.execute("analysis.ducted_fan", &json!({"diameter": 96, "hub_diameter": 34, "blades": 6, "beta_deg": 89, "rpm": 15000})).is_err());
    }

    #[test]
    fn motor_match_balances_torque() {
        let motor = json!({"kv": 900, "resistance": 0.04, "no_load_current": 1.2, "voltage": 22.2});
        let m = run(json!({"diameter": 96, "hub_diameter": 34, "blades": 6, "chord": 12, "beta_deg": [30, 34, 38], "motor": motor}));
        let rpm = f(&m, "rpm");
        assert!(rpm > 1000.0 && rpm < 900.0 * 22.2, "{m:#}");
        // At that rpm the motor needs the given voltage.
        assert!((f(&m["motor"], "voltage_v") - 22.2).abs() < 0.05, "{m:#}");
        assert!(f(&m["motor"], "efficiency") > 0.3 && f(&m["motor"], "efficiency") < 1.0);
        // Given the rpm instead, the same motor answers the same voltage.
        let at = run(json!({"diameter": 96, "hub_diameter": 34, "blades": 6, "chord": 12, "beta_deg": [30, 34, 38], "rpm": rpm, "motor": motor}));
        assert!((f(&at["motor"], "voltage_v") - 22.2).abs() < 0.05);
    }

    #[test]
    fn rpm_sweep_answers_points() {
        let r = run(json!({"diameter": 96, "hub_diameter": 34, "beta_deg": 30, "rpm_sweep": [5000, 20000, 4]}));
        let pts = r["points"].as_array().cloned().unwrap_or_default();
        assert_eq!(pts.len(), 4);
        assert!(f(&pts[3], "thrust_n") > f(&pts[0], "thrust_n"));
    }

    #[test]
    fn forward_flight_has_an_efficiency() {
        let r = run(json!({"diameter": 96, "hub_diameter": 34, "blades": 6, "beta_deg": [30, 34, 38], "rpm": 15000, "airspeed": 20}));
        assert!(f(&r, "efficiency") > 0.0 && f(&r, "efficiency") < 1.0, "{r:#}");
        assert!(r["figure_of_merit"].is_null());
    }

    #[test]
    fn hostile_params_never_panic() {
        let mut s = Session::default();
        let bad = [
            json!({}),
            json!({"diameter": 96}),
            json!({"diameter": -1, "beta_deg": 30, "rpm": 1000}),
            json!({"diameter": 96, "hub_diameter": 96, "beta_deg": 30, "rpm": 1000}),
            json!({"diameter": 96, "beta_deg": 0, "rpm": 1000}),
            json!({"diameter": 96, "beta_deg": 90, "rpm": 1000}),
            json!({"diameter": 96, "beta_deg": "30", "rpm": 1000}),
            json!({"diameter": 96, "beta_deg": [], "rpm": 1000}),
            json!({"diameter": 96, "beta_deg": 30, "rpm": 1e12}),
            json!({"diameter": 96, "beta_deg": 30, "rpm_sweep": [1, 2]}),
            json!({"diameter": 96, "beta_deg": 30, "rpm_sweep": [100, 10, 5]}),
            json!({"diameter": 96, "beta_deg": 30, "rpm_sweep": [100, 1000, 1e9]}),
            json!({"diameter": 96, "beta_deg": 30, "rpm": 1000, "chord": 0}),
            json!({"diameter": 96, "beta_deg": 30, "rpm": 1000, "chord": [1]}),
            json!({"diameter": 96, "beta_deg": 30, "rpm": 1000, "stations": 1e9}),
            json!({"diameter": 96, "beta_deg": 30, "rpm": 1000, "motor": 5}),
            json!({"diameter": 96, "beta_deg": 30, "rpm": 1000, "motor": {"kv": 0}}),
            json!({"diameter": 96, "beta_deg": 30, "motor": {"kv": 900, "resistance": 0.04}}),
            json!({"diameter": 96, "beta_deg": 30, "rpm": 1000, "twist": "spiral"}),
            json!({"diameter": 96, "beta_deg": 30, "rpm": 1000, "polar": {"cl_alpha": 1e9}}),
            json!({"diameter": 96, "stages": [{}], "rpm": 1000}),
            json!({"diameter": 96, "stages": "x", "rpm": 1000}),
            json!({"diameter": 96, "beta_deg": 1, "rpm": 1, "exit_diameter": 1}),
        ];
        for p in bad {
            match s.execute("analysis.ducted_fan", &p) {
                Ok(_) => {}
                Err(e) => assert!(!matches!(e, crate::EngineError::Internal(..)), "{p}: {e}"),
            }
        }
    }
}
