//! What draws a headless picture: every adapter the platform's native backends offer is
//! classified as hardware (a discrete, integrated or virtual GPU) or software (a GPU driver that
//! runs on the CPU, such as llvmpipe, lavapipe or WARP), and a [`Policy`] picks one. The last
//! tier, below any adapter, is SolidCraft's own CPU rasterizer (no surface analysis, highlights
//! or legends): `auto` lands there only when no adapter qualifies, `cpu` forces it, `hardware`
//! and `software` never use it. macOS uses Metal only, and has no software adapter. `SOLIDCRAFT_GPU`
//! (`auto`, `hardware`, `software`, `cpu`) sets the policy from the environment; `WGPU_BACKEND`
//! still narrows the backends.

use serde::Serialize;

/// Whether an adapter is a GPU or a software rasterizer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Hardware,
    Software,
}

/// Which adapters a headless device may open.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Policy {
    /// The best hardware adapter, else a software one.
    #[default]
    Auto,
    /// A hardware adapter or nothing.
    Hardware,
    /// A software adapter or nothing.
    Software,
    /// The CPU rasterizer, whatever adapters there are.
    Cpu,
}

/// Where a policy lands: an adapter of a tier, or the CPU rasterizer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Landing {
    Hardware,
    Software,
    Cpu,
}

impl Landing {
    /// What a picture's reply names as its renderer: `gpu`, `gpu-software` or `cpu`.
    pub fn renderer(self) -> &'static str {
        match self {
            Landing::Hardware => "gpu",
            Landing::Software => "gpu-software",
            Landing::Cpu => "cpu",
        }
    }
}

impl From<Tier> for Landing {
    fn from(t: Tier) -> Self {
        match t {
            Tier::Hardware => Landing::Hardware,
            Tier::Software => Landing::Software,
        }
    }
}

impl Policy {
    /// `auto`, `hardware`, `software` or `cpu` (case-insensitive).
    pub fn parse(s: &str) -> Option<Policy> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Policy::Auto),
            "hardware" | "hw" | "gpu" => Some(Policy::Hardware),
            "software" | "sw" => Some(Policy::Software),
            "cpu" => Some(Policy::Cpu),
            _ => None,
        }
    }

    /// The policy `SOLIDCRAFT_GPU` names; [`Policy::Auto`] when it is unset, and an error when
    /// it names none.
    pub fn from_env() -> Result<Policy, String> {
        match std::env::var("SOLIDCRAFT_GPU") {
            Ok(v) => Policy::parse(&v).ok_or_else(|| format!("SOLIDCRAFT_GPU must be auto, hardware, software or cpu, not `{v}`")),
            Err(_) => Ok(Policy::Auto),
        }
    }

    /// `auto`, `hardware`, `software` or `cpu`.
    pub fn name(self) -> &'static str {
        match self {
            Policy::Auto => "auto",
            Policy::Hardware => "hardware",
            Policy::Software => "software",
            Policy::Cpu => "cpu",
        }
    }

    pub(crate) fn admits(self, tier: Tier) -> bool {
        match self {
            Policy::Auto => true,
            Policy::Hardware => tier == Tier::Hardware,
            Policy::Software => tier == Tier::Software,
            Policy::Cpu => false,
        }
    }

    /// Where the policy lands when the best qualifying adapter is of tier `adapter` (`None`: no
    /// adapter qualifies): that adapter; else the CPU rasterizer under `auto` (and always under
    /// `cpu`); else nowhere, and a picture is an error.
    pub fn lands_on(self, adapter: Option<Tier>) -> Option<Landing> {
        match (self, adapter) {
            (Policy::Cpu, _) => Some(Landing::Cpu),
            (_, Some(t)) => Some(t.into()),
            (Policy::Auto, None) => Some(Landing::Cpu),
            (Policy::Hardware | Policy::Software, None) => None,
        }
    }
}

/// One adapter as the survey saw it.
#[derive(Clone, Debug, Serialize)]
pub struct AdapterReport {
    pub name: String,
    pub backend: String,
    pub device_type: String,
    pub driver: String,
    pub tier: Tier,
}

/// The backends a headless device looks at on this platform: Metal on macOS, Vulkan and D3D12
/// on Windows, Vulkan and GL elsewhere; `WGPU_BACKEND` narrows them further.
pub fn native_backends() -> wgpu::Backends {
    let native = if cfg!(target_os = "macos") {
        wgpu::Backends::METAL
    } else if cfg!(target_os = "windows") {
        wgpu::Backends::DX12 | wgpu::Backends::VULKAN
    } else {
        wgpu::Backends::VULKAN | wgpu::Backends::GL
    };
    match wgpu::Backends::from_env() {
        Some(env) if !(env & native).is_empty() => env & native,
        _ => native,
    }
}

pub(crate) fn tier_of(t: wgpu::DeviceType) -> Tier {
    match t {
        wgpu::DeviceType::Cpu => Tier::Software,
        wgpu::DeviceType::DiscreteGpu | wgpu::DeviceType::IntegratedGpu | wgpu::DeviceType::VirtualGpu | wgpu::DeviceType::Other => Tier::Hardware,
    }
}

/// Preference among adapters (higher first): a discrete GPU, an integrated one, a virtual one,
/// an unknown kind, a software rasterizer.
fn rank(t: wgpu::DeviceType) -> u8 {
    match t {
        wgpu::DeviceType::DiscreteGpu => 4,
        wgpu::DeviceType::IntegratedGpu => 3,
        wgpu::DeviceType::VirtualGpu => 2,
        wgpu::DeviceType::Other => 1,
        wgpu::DeviceType::Cpu => 0,
    }
}

fn report(info: &wgpu::AdapterInfo) -> AdapterReport {
    AdapterReport {
        name: info.name.clone(),
        backend: format!("{:?}", info.backend),
        device_type: format!("{:?}", info.device_type),
        driver: [info.driver.as_str(), info.driver_info.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" "),
        tier: tier_of(info.device_type),
    }
}

fn instance() -> wgpu::Instance {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
    desc.backends = native_backends();
    desc.backend_options.dx12.shader_compiler = viewport_dx12_compiler();
    wgpu::Instance::new(desc)
}

/// The native backends with D3D12 compiling through [`dxc`]; without DXC, no D3D12 at all.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn dxc_instance() -> wgpu::Instance {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
    desc.backends = native_backends();
    match dxc() {
        Some(compiler) => desc.backend_options.dx12.shader_compiler = compiler,
        None => desc.backends.remove(wgpu::Backends::DX12),
    }
    wgpu::Instance::new(desc)
}

/// Every adapter the native backends offer, best first.
#[cfg(not(target_arch = "wasm32"))]
fn adapters(instance: &wgpu::Instance) -> Vec<wgpu::Adapter> {
    let mut all = pollster::block_on(instance.enumerate_adapters(native_backends()));
    all.sort_by_key(|a| std::cmp::Reverse(rank(a.get_info().device_type)));
    all
}

/// What a survey found, which adapter the policy picks (`chosen` indexes `adapters`) and where
/// it lands (`None`: nowhere, a picture is an error).
#[derive(Clone, Debug, Serialize)]
pub struct Survey {
    pub backends: String,
    pub adapters: Vec<AdapterReport>,
    pub chosen: Option<usize>,
    pub lands_on: Option<Landing>,
}

/// Every adapter the native backends offer, best first, and which one `policy` picks.
#[cfg(not(target_arch = "wasm32"))]
pub fn survey(policy: Policy) -> Survey {
    let found: Vec<AdapterReport> = adapters(&instance()).iter().map(|a| report(&a.get_info())).collect();
    let chosen = found.iter().position(|r| policy.admits(r.tier));
    let lands_on = policy.lands_on(chosen.and_then(|i| found.get(i)).map(|r| r.tier));
    Survey { backends: format!("{:?}", native_backends()), adapters: found, chosen, lands_on }
}

/// The adapter `policy` picks, with its report; `None` when no adapter qualifies.
#[cfg(not(target_arch = "wasm32"))]
pub fn pick(policy: Policy) -> Option<(wgpu::Adapter, AdapterReport)> {
    pick_on(&instance(), policy, |_| true)
}

/// The adapter `policy` picks on `instance` among those `keep` accepts, with its report.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn pick_on(
    instance: &wgpu::Instance,
    policy: Policy,
    keep: impl Fn(&wgpu::AdapterInfo) -> bool,
) -> Option<(wgpu::Adapter, AdapterReport)> {
    adapters(instance)
        .into_iter()
        .filter(|a| keep(&a.get_info()))
        .map(|a| {
            let r = report(&a.get_info());
            (a, r)
        })
        .find(|(_, r)| policy.admits(r.tier))
}

/// The D3D12 shader compiler for the viewport and headless pictures: `WGPU_DX12_COMPILER`, else
/// `dxcompiler.dll` when it loads, else FXC. This is wgpu's own choice without the DXC that x64
/// Windows builds link in for the path tracer (#3217): linked-in DXC would also hide adapters
/// without shader model 6, which FXC still drives. Probed once.
pub fn viewport_dx12_compiler() -> wgpu::Dx12Compiler {
    static COMPILER: std::sync::OnceLock<wgpu::Dx12Compiler> = std::sync::OnceLock::new();
    COMPILER
        .get_or_init(|| match wgpu::Dx12Compiler::from_env() {
            Some(wgpu::Dx12Compiler::Auto) | None if cfg!(target_os = "windows") => {
                if dx12_loads(wgpu::Dx12Compiler::default_dynamic_dxc()) {
                    wgpu::Dx12Compiler::default_dynamic_dxc()
                } else {
                    wgpu::Dx12Compiler::Fxc
                }
            }
            chosen => chosen.unwrap_or_default(),
        })
        .clone()
}

/// DXC for large shaders such as the path tracer's: the one `WGPU_DX12_COMPILER` names, else
/// the one linked in (x64 Windows builds), else `dxcompiler.dll`. `None` leaves only FXC, which
/// needs about two hours for the path shader (#3217). Probed once.
pub fn dxc() -> Option<wgpu::Dx12Compiler> {
    static DXC: std::sync::OnceLock<Option<wgpu::Dx12Compiler>> = std::sync::OnceLock::new();
    DXC.get_or_init(|| {
        let candidates = match wgpu::Dx12Compiler::from_env() {
            _ if !cfg!(target_os = "windows") => vec![],
            Some(wgpu::Dx12Compiler::Fxc) => vec![],
            Some(wgpu::Dx12Compiler::Auto) | None => vec![wgpu::Dx12Compiler::StaticDxc, wgpu::Dx12Compiler::default_dynamic_dxc()],
            Some(named) => vec![named],
        };
        candidates.into_iter().find(|c| dx12_loads(c.clone()))
    })
    .clone()
}

/// A D3D12 instance restricted to `compiler` fails to start when that compiler doesn't load,
/// and then offers no adapters.
fn dx12_loads(compiler: wgpu::Dx12Compiler) -> bool {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::DX12;
    desc.flags = wgpu::InstanceFlags::empty();
    desc.backend_options.dx12.shader_compiler = compiler;
    !pollster::block_on(wgpu::Instance::new(desc).enumerate_adapters(wgpu::Backends::DX12)).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policies_parse_and_admit_their_tiers() {
        assert_eq!(Policy::parse(" Hardware "), Some(Policy::Hardware));
        assert_eq!(Policy::parse("sw"), Some(Policy::Software));
        assert_eq!(Policy::parse("metal"), None);
        assert_eq!(Policy::parse("CPU"), Some(Policy::Cpu));
        assert!(!Policy::Cpu.admits(Tier::Software) && !Policy::Cpu.admits(Tier::Hardware));
        assert!(Policy::Auto.admits(Tier::Software) && Policy::Auto.admits(Tier::Hardware));
        assert!(!Policy::Hardware.admits(Tier::Software));
        assert!(!Policy::Software.admits(Tier::Hardware));
    }

    /// Hardware GPU, then a software adapter, then the CPU rasterizer: `auto` walks all three,
    /// `hardware` and `software` never fall to the CPU, and `cpu` always lands there.
    #[test]
    fn policies_land_on_their_tiers() {
        use Landing as L;
        let (hw, sw) = (Some(Tier::Hardware), Some(Tier::Software));
        assert_eq!([hw, sw, None].map(|t| Policy::Auto.lands_on(t)), [Some(L::Hardware), Some(L::Software), Some(L::Cpu)]);
        assert_eq!(Policy::Hardware.lands_on(hw), Some(L::Hardware));
        assert_eq!(Policy::Hardware.lands_on(None), None);
        assert_eq!(Policy::Software.lands_on(sw), Some(L::Software));
        assert_eq!(Policy::Software.lands_on(None), None);
        assert_eq!([hw, sw, None].map(|t| Policy::Cpu.lands_on(t)), [Some(L::Cpu); 3]);
        assert_eq!([L::Hardware, L::Software, L::Cpu].map(L::renderer), ["gpu", "gpu-software", "cpu"]);
    }

    #[test]
    fn software_rasterizers_rank_last() {
        assert_eq!(tier_of(wgpu::DeviceType::Cpu), Tier::Software);
        assert!(rank(wgpu::DeviceType::IntegratedGpu) > rank(wgpu::DeviceType::Cpu));
        assert!(rank(wgpu::DeviceType::DiscreteGpu) > rank(wgpu::DeviceType::IntegratedGpu));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_surveys_metal_only() {
        assert_eq!(native_backends() & !wgpu::Backends::METAL, wgpu::Backends::empty());
        let s = survey(Policy::Auto);
        assert!(s.adapters.iter().all(|a| a.backend == "Metal"), "{s:?}");
    }
}
