use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const TOOL_CATALOG: &str = include_str!("../../../../tools/catalog.json");

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlatformInfo {
    pub os: String,
    pub family: String,
    pub arch: String,
    pub supported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolCheck {
    pub id: String,
    pub display_name: String,
    pub category: String,
    pub available: bool,
    pub command: Option<String>,
    pub version: Option<String>,
    pub discovered_outside_path: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuditSummary {
    pub available: usize,
    pub missing: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EnvironmentAudit {
    pub platform: PlatformInfo,
    pub tools: Vec<ToolCheck>,
    pub execution_backends: ExecutionBackendAudit,
    pub conda: Option<CondaAudit>,
    pub summary: AuditSummary,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExecutionBackendAudit {
    pub policy: String,
    pub required_any_of: Vec<String>,
    pub available: Vec<String>,
    pub ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CondaAudit {
    pub command: String,
    pub distribution: String,
    pub version: Option<String>,
    pub root_prefix: Option<String>,
    pub channels: Vec<String>,
    pub bioconda_configured: bool,
    pub bioconda_native_supported: bool,
    pub channel_order_valid: bool,
    pub strict_channel_priority: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlanActionState {
    Available,
    Install,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstallAction {
    pub tool_id: String,
    pub display_name: String,
    pub state: PlanActionState,
    pub execution_provider: Option<String>,
    pub strategy: Option<String>,
    pub package: Option<String>,
    pub source_url: Option<String>,
    pub resolved_source_url: Option<String>,
    pub requires_admin: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EnvironmentPlan {
    pub profile: String,
    pub description: String,
    pub platform: PlatformInfo,
    pub github_proxy: Option<String>,
    pub actions: Vec<InstallAction>,
    pub requires_confirmation: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub struct EnvironmentError(String);

impl Display for EnvironmentError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for EnvironmentError {}

#[derive(Debug, Deserialize)]
struct ToolCatalog {
    schema_version: String,
    profiles: Vec<ToolProfile>,
    tools: Vec<ToolDefinition>,
}

#[derive(Debug, Deserialize)]
struct ToolProfile {
    id: String,
    description: String,
    tools: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ToolDefinition {
    id: String,
    display_name: String,
    category: String,
    #[serde(default)]
    status: ToolStatus,
    #[serde(default)]
    platforms: Vec<String>,
    probes: Vec<ToolProbe>,
    install: BTreeMap<String, InstallSpec>,
}

#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum ToolStatus {
    #[default]
    Active,
    Planned,
}

#[derive(Debug, Deserialize)]
struct ToolProbe {
    program: String,
    args: Vec<String>,
    output_contains: Option<String>,
}

#[derive(Debug, Deserialize)]
struct InstallSpec {
    strategy: String,
    package: String,
    source_url: String,
    requires_admin: bool,
}

pub fn audit_environment() -> Result<EnvironmentAudit, EnvironmentError> {
    let catalog = load_catalog()?;
    let platform = current_platform();
    let wsl_distributions = if platform.family == "windows" {
        probe_wsl_distributions()
    } else {
        None
    };
    let mut tools = catalog
        .tools
        .iter()
        .filter(|tool| tool.status == ToolStatus::Active)
        .filter(|tool| tool_applies_to_platform(tool, &platform.family))
        .map(|tool| probe_tool(tool, &platform, wsl_distributions.as_deref()))
        .collect::<Vec<_>>();
    let conda = inspect_conda(&tools, &platform);
    apply_conda_distribution(&mut tools, conda.as_ref());
    let available = tools.iter().filter(|tool| tool.available).count();
    let execution_backends = summarize_execution_backends(&platform, &tools);
    let mut warnings = Vec::new();
    if !platform.supported {
        warnings.push(format!(
            "platform family '{}' is not in the Windows, Debian, or Arch support matrix",
            platform.family
        ));
    }
    for tool in tools
        .iter()
        .filter(|tool| tool.available && tool.discovered_outside_path)
    {
        warnings.push(format!(
            "{} was discovered at {} but is not available through the current PATH",
            tool.display_name,
            tool.command.as_deref().unwrap_or("an unknown path")
        ));
    }
    if !execution_backends.ready {
        warnings.push(format!(
            "no supported execution backend was found; install or configure one of: {}",
            execution_backends.required_any_of.join(", ")
        ));
    }
    if conda.as_ref().is_some_and(|configuration| {
        !configuration.bioconda_configured
            || !configuration.channel_order_valid
            || !configuration.strict_channel_priority
    }) {
        warnings.push(
            "Conda/Bioconda configuration is incomplete; keep conda-forge ahead of bioconda and use strict channel priority"
                .to_owned(),
        );
    }
    if conda.as_ref().is_some_and(|configuration| {
        configuration.bioconda_configured && !configuration.bioconda_native_supported
    }) {
        warnings.push(
            "Bioconda does not publish native Windows packages; run Bioconda environments through WSL Arch, WSL Debian, or another supported Linux backend"
                .to_owned(),
        );
    }

    Ok(EnvironmentAudit {
        platform,
        execution_backends,
        conda,
        summary: AuditSummary {
            available,
            missing: tools.len().saturating_sub(available),
        },
        tools,
        warnings,
    })
}

pub fn plan_environment(
    profile_id: &str,
    audit: &EnvironmentAudit,
) -> Result<EnvironmentPlan, EnvironmentError> {
    let catalog = load_catalog()?;
    let profile = catalog
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| EnvironmentError(format!("unknown environment profile: {profile_id}")))?;
    let definitions = catalog
        .tools
        .iter()
        .map(|tool| (tool.id.as_str(), tool))
        .collect::<BTreeMap<_, _>>();
    let checks = audit
        .tools
        .iter()
        .map(|tool| (tool.id.as_str(), tool))
        .collect::<BTreeMap<_, _>>();
    let github_proxy = env::var("GITHUB_PROXY")
        .ok()
        .or_else(|| env::var("LINXIRA_GITHUB_PROXY").ok())
        .filter(|value| !value.trim().is_empty());
    let mut actions = Vec::new();
    let mut warnings = Vec::new();

    for tool_id in &profile.tools {
        let definition = definitions.get(tool_id.as_str()).ok_or_else(|| {
            EnvironmentError(format!(
                "profile {profile_id} references unknown tool {tool_id}"
            ))
        })?;
        if !tool_applies_to_platform(definition, &audit.platform.family) {
            continue;
        }
        let available = checks
            .get(tool_id.as_str())
            .is_some_and(|tool| tool.available);
        let platform_install = definition.install.get(&audit.platform.family);
        let (install, execution_provider) =
            resolve_install_spec(definition, platform_install, audit);
        let state = if available {
            PlanActionState::Available
        } else if platform_install.is_some() {
            PlanActionState::Install
        } else {
            PlanActionState::Unsupported
        };
        let source_url = install.map(|spec| spec.source_url.clone());
        let resolved_source_url = source_url
            .as_deref()
            .map(|url| apply_github_proxy(url, github_proxy.as_deref()));

        if state == PlanActionState::Unsupported && profile.id != "containers" {
            warnings.push(format!(
                "{} has no registered installation strategy for {}",
                definition.display_name, audit.platform.family
            ));
        }

        actions.push(InstallAction {
            tool_id: definition.id.clone(),
            display_name: definition.display_name.clone(),
            state,
            strategy: install.map(|spec| {
                execution_provider
                    .map(|provider| format!("{provider}/{}", spec.strategy))
                    .unwrap_or_else(|| spec.strategy.clone())
            }),
            execution_provider: execution_provider.map(str::to_owned),
            package: install.map(|spec| spec.package.clone()),
            source_url,
            resolved_source_url,
            requires_admin: install.is_some_and(|spec| spec.requires_admin),
        });
    }

    let uses_wsl = actions
        .iter()
        .any(|action| action.strategy.as_deref() == Some("wsl-provider"));
    if uses_wsl {
        warnings.push(
            "the Windows plan requires a configured WSL Arch or WSL Debian provider for Unix-native tools"
                .to_owned(),
        );
    }

    if profile.id == "containers" {
        if audit.execution_backends.ready {
            actions.retain(|action| {
                action.state == PlanActionState::Available
                    && audit.execution_backends.available.contains(&action.tool_id)
            });
        } else {
            warnings.push(
                "execution backend actions are alternatives; choose one provider rather than installing every listed option"
                    .to_owned(),
            );
        }
    }

    let required_tools = profile.tools.iter().cloned().collect::<BTreeSet<_>>();
    if required_tools.len() != profile.tools.len() {
        warnings.push("the selected profile contains duplicate tool identifiers".to_owned());
    }

    Ok(EnvironmentPlan {
        profile: profile.id.clone(),
        description: profile.description.clone(),
        platform: audit.platform.clone(),
        github_proxy,
        requires_confirmation: actions
            .iter()
            .any(|action| action.state == PlanActionState::Install),
        actions,
        warnings,
    })
}

pub fn apply_github_proxy(url: &str, proxy: Option<&str>) -> String {
    let Some(proxy) = proxy.filter(|value| !value.trim().is_empty()) else {
        return url.to_owned();
    };
    if !url.starts_with("https://github.com/") {
        return url.to_owned();
    }
    format!("{}/{}", proxy.trim_end_matches('/'), url)
}

fn load_catalog() -> Result<ToolCatalog, EnvironmentError> {
    let catalog: ToolCatalog = serde_json::from_str(TOOL_CATALOG)
        .map_err(|error| EnvironmentError(format!("invalid embedded tool catalog: {error}")))?;
    if catalog.schema_version != "1" {
        return Err(EnvironmentError(format!(
            "unsupported tool catalog schema: {}",
            catalog.schema_version
        )));
    }
    Ok(catalog)
}

fn current_platform() -> PlatformInfo {
    let os = env::consts::OS.to_owned();
    let family = match os.as_str() {
        "windows" => "windows".to_owned(),
        "linux" => linux_family(),
        other => other.to_owned(),
    };
    let supported = matches!(family.as_str(), "windows" | "debian" | "arch");
    PlatformInfo {
        os,
        family,
        arch: env::consts::ARCH.to_owned(),
        supported,
    }
}

fn linux_family() -> String {
    let release = fs::read_to_string("/etc/os-release").unwrap_or_default();
    release
        .lines()
        .find_map(|line| line.strip_prefix("ID="))
        .map(|value| value.trim_matches('"').to_owned())
        .unwrap_or_else(|| "linux".to_owned())
}

fn probe_tool(
    definition: &ToolDefinition,
    platform: &PlatformInfo,
    wsl_distributions: Option<&str>,
) -> ToolCheck {
    if definition.id.starts_with("wsl-") {
        return probe_wsl_provider(definition, wsl_distributions);
    }
    if definition.id == "miniforge" {
        return unavailable_tool(definition);
    }
    for probe in &definition.probes {
        for (program, discovered_outside_path) in program_candidates(definition, probe, platform) {
            let Ok(output) = Command::new(&program).args(&probe.args).output() else {
                continue;
            };
            if !output.status.success() {
                continue;
            }
            let stdout = decode_output(&output.stdout);
            let stderr = decode_output(&output.stderr);
            if let Some(required) = &probe.output_contains
                && !format!("{stdout}\n{stderr}")
                    .to_lowercase()
                    .contains(&required.to_lowercase())
            {
                continue;
            }
            let version = probe
                .output_contains
                .as_deref()
                .and_then(|required| {
                    first_matching_output_line(&stdout, required)
                        .or_else(|| first_matching_output_line(&stderr, required))
                })
                .or_else(|| first_output_line(&stdout))
                .or_else(|| first_output_line(&stderr));
            return ToolCheck {
                id: definition.id.clone(),
                display_name: definition.display_name.clone(),
                category: definition.category.clone(),
                available: true,
                command: Some(program.to_string_lossy().into_owned()),
                version,
                discovered_outside_path,
            };
        }
    }

    unavailable_tool(definition)
}

fn probe_wsl_distributions() -> Option<String> {
    let output = Command::new("wsl.exe")
        .args(["--list", "--quiet"])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| decode_output(&output.stdout))
}

fn probe_wsl_provider(definition: &ToolDefinition, distributions: Option<&str>) -> ToolCheck {
    let Some(required) = definition
        .probes
        .first()
        .and_then(|probe| probe.output_contains.as_deref())
    else {
        return unavailable_tool(definition);
    };
    let Some(distribution) =
        distributions.and_then(|output| first_matching_output_line(output, required))
    else {
        return unavailable_tool(definition);
    };

    ToolCheck {
        id: definition.id.clone(),
        display_name: definition.display_name.clone(),
        category: definition.category.clone(),
        available: true,
        command: Some("wsl.exe".to_owned()),
        version: Some(distribution),
        discovered_outside_path: false,
    }
}

fn unavailable_tool(definition: &ToolDefinition) -> ToolCheck {
    ToolCheck {
        id: definition.id.clone(),
        display_name: definition.display_name.clone(),
        category: definition.category.clone(),
        available: false,
        command: None,
        version: None,
        discovered_outside_path: false,
    }
}

fn apply_conda_distribution(tools: &mut [ToolCheck], conda: Option<&CondaAudit>) {
    let Some(conda) = conda.filter(|configuration| configuration.distribution == "miniforge")
    else {
        return;
    };
    if let Some(tool) = tools.iter_mut().find(|tool| tool.id == "miniforge") {
        tool.available = true;
        tool.command = Some(conda.command.clone());
        tool.version = conda.version.clone();
        tool.discovered_outside_path = Path::new(&conda.command).is_absolute();
    }
}

fn tool_applies_to_platform(definition: &ToolDefinition, family: &str) -> bool {
    definition.platforms.is_empty() || definition.platforms.iter().any(|item| item == family)
}

fn resolve_install_spec<'a>(
    definition: &'a ToolDefinition,
    platform_install: Option<&'a InstallSpec>,
    audit: &EnvironmentAudit,
) -> (Option<&'a InstallSpec>, Option<&'static str>) {
    if audit.platform.family != "windows"
        || !platform_install.is_some_and(|install| install.strategy == "wsl-provider")
    {
        return (platform_install, None);
    }

    for (provider, family) in [("wsl-arch", "arch"), ("wsl-debian", "debian")] {
        if audit
            .tools
            .iter()
            .any(|tool| tool.id == provider && tool.available)
            && let Some(install) = definition.install.get(family)
        {
            return (Some(install), Some(provider));
        }
    }

    (platform_install, None)
}

fn program_candidates(
    definition: &ToolDefinition,
    probe: &ToolProbe,
    platform: &PlatformInfo,
) -> Vec<(PathBuf, bool)> {
    let mut candidates = vec![(PathBuf::from(&probe.program), false)];
    if platform.family != "windows" {
        return candidates;
    }

    if definition.id == "r"
        && let Some(root) = windows_r_install_root()
    {
        candidates.push((
            root.join("bin").join(format!("{}.exe", probe.program)),
            true,
        ));
    }
    if matches!(definition.id.as_str(), "conda" | "miniforge") {
        for root in windows_conda_roots() {
            candidates.push((root.join("Scripts").join("conda.exe"), true));
            candidates.push((root.join("condabin").join("conda.bat"), true));
        }
    }
    candidates
}

fn windows_r_install_root() -> Option<PathBuf> {
    for key in [
        r"HKLM\SOFTWARE\R-core\R",
        r"HKLM\SOFTWARE\WOW6432Node\R-core\R",
        r"HKCU\SOFTWARE\R-core\R",
    ] {
        let Ok(output) = Command::new("reg.exe")
            .args(["query", key, "/v", "InstallPath"])
            .output()
        else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        let stdout = decode_output(&output.stdout);
        if let Some(path) = stdout.lines().find_map(|line| {
            line.split_once("REG_SZ")
                .map(|(_, value)| value.trim())
                .filter(|value| !value.is_empty())
        }) {
            return Some(PathBuf::from(path));
        }
    }
    None
}

fn windows_conda_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for variable in ["CONDA_ROOT", "CONDA_PREFIX"] {
        if let Ok(value) = env::var(variable)
            && !value.trim().is_empty()
        {
            roots.push(PathBuf::from(value));
        }
    }
    if let Ok(profile) = env::var("USERPROFILE") {
        roots.push(Path::new(&profile).join("miniforge3"));
        roots.push(Path::new(&profile).join("miniconda3"));
    }
    roots
}

fn summarize_execution_backends(
    platform: &PlatformInfo,
    tools: &[ToolCheck],
) -> ExecutionBackendAudit {
    let (policy, required_any_of) = match platform.family.as_str() {
        "windows" => (
            "Windows requires WSL Arch, WSL Debian, or Docker for Unix/container workflows",
            vec!["wsl-arch", "wsl-debian", "docker"],
        ),
        "debian" | "arch" => (
            "Linux checks both Docker and Podman; either provides a local container backend",
            vec!["docker", "podman"],
        ),
        _ => ("No execution backend policy is registered", Vec::new()),
    };
    let available = required_any_of
        .iter()
        .filter(|id| tools.iter().any(|tool| tool.id == **id && tool.available))
        .map(|id| (*id).to_owned())
        .collect::<Vec<_>>();
    let ready = required_any_of.is_empty() || !available.is_empty();
    ExecutionBackendAudit {
        policy: policy.to_owned(),
        required_any_of: required_any_of.into_iter().map(str::to_owned).collect(),
        available,
        ready,
    }
}

fn inspect_conda(tools: &[ToolCheck], platform: &PlatformInfo) -> Option<CondaAudit> {
    let command = tools
        .iter()
        .find(|tool| tool.id == "conda" && tool.available)?
        .command
        .as_deref()?;
    let output = Command::new(command)
        .args(["info", "--json"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let info: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let root_prefix = info
        .get("root_prefix")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    let channels = info
        .get("channels")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let priority = Command::new(command)
        .args(["config", "--show", "channel_priority", "--json"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| serde_json::from_slice::<serde_json::Value>(&output.stdout).ok())
        .and_then(|value| {
            value
                .get("channel_priority")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        });
    let distribution = if root_prefix
        .as_deref()
        .is_some_and(|path| path.to_lowercase().contains("miniforge"))
    {
        "miniforge"
    } else {
        "conda"
    };

    let bioconda_position = channels
        .iter()
        .position(|channel| channel.to_lowercase().contains("bioconda"));

    Some(CondaAudit {
        command: command.to_owned(),
        distribution: distribution.to_owned(),
        version: info
            .get("conda_version")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        root_prefix,
        bioconda_configured: bioconda_position.is_some(),
        bioconda_native_supported: matches!(platform.family.as_str(), "debian" | "arch"),
        channel_order_valid: conda_channel_order_valid(&channels),
        channels,
        strict_channel_priority: priority.as_deref() == Some("strict"),
    })
}

fn conda_channel_order_valid(channels: &[String]) -> bool {
    let conda_forge = channels
        .iter()
        .position(|channel| channel.to_lowercase().contains("conda-forge"));
    let bioconda = channels
        .iter()
        .position(|channel| channel.to_lowercase().contains("bioconda"));
    conda_forge
        .zip(bioconda)
        .is_some_and(|(conda_forge, bioconda)| conda_forge < bioconda)
}

fn first_output_line(output: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_owned)
}

fn first_matching_output_line(output: &str, required: &str) -> Option<String> {
    let required = required.to_lowercase();
    output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && line.to_lowercase().contains(&required))
        .map(str::to_owned)
}

fn decode_output(bytes: &[u8]) -> String {
    let nul_count = bytes.iter().filter(|byte| **byte == 0).count();
    if bytes.len() >= 2 && (bytes.starts_with(&[0xff, 0xfe]) || nul_count >= 2) {
        let words = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .filter(|word| *word != 0xfeff)
            .collect::<Vec<_>>();
        return String::from_utf16_lossy(&words);
    }
    String::from_utf8_lossy(bytes).into_owned()
}

#[cfg(test)]
mod tests {
    use super::{
        AuditSummary, EnvironmentAudit, ExecutionBackendAudit, PlanActionState, PlatformInfo,
        ToolCheck, apply_github_proxy, conda_channel_order_valid, decode_output,
        first_matching_output_line, load_catalog, plan_environment, probe_wsl_provider,
        summarize_execution_backends,
    };

    #[test]
    fn catalog_profiles_reference_registered_tools() {
        let catalog = load_catalog().expect("valid embedded catalog");
        let tool_ids = catalog
            .tools
            .iter()
            .map(|tool| tool.id.as_str())
            .collect::<std::collections::BTreeSet<_>>();

        for profile in catalog.profiles {
            for tool in profile.tools {
                assert!(tool_ids.contains(tool.as_str()), "unknown tool {tool}");
                assert_eq!(
                    catalog
                        .tools
                        .iter()
                        .find(|definition| definition.id == tool)
                        .map(|definition| &definition.status),
                    Some(&super::ToolStatus::Active),
                    "planned tool {tool} must not appear in an executable profile"
                );
            }
        }
    }

    #[test]
    fn linxira_wsl_is_reserved_but_not_audited_as_active() {
        let catalog = load_catalog().expect("valid embedded catalog");
        let linxira = catalog
            .tools
            .iter()
            .find(|tool| tool.id == "wsl-linxira")
            .expect("planned Linxira WSL provider");

        assert_eq!(linxira.status, super::ToolStatus::Planned);
        assert!(linxira.install.is_empty());
        assert!(
            catalog
                .profiles
                .iter()
                .all(|profile| !profile.tools.contains(&linxira.id))
        );
    }

    #[test]
    fn platform_specific_tools_are_filtered() {
        let catalog = load_catalog().expect("valid embedded catalog");
        let wsl_arch = catalog
            .tools
            .iter()
            .find(|tool| tool.id == "wsl-arch")
            .expect("WSL Arch tool");
        let wsl_debian = catalog
            .tools
            .iter()
            .find(|tool| tool.id == "wsl-debian")
            .expect("WSL Debian tool");
        let podman = catalog
            .tools
            .iter()
            .find(|tool| tool.id == "podman")
            .expect("Podman tool");

        assert!(super::tool_applies_to_platform(wsl_arch, "windows"));
        assert!(super::tool_applies_to_platform(wsl_debian, "windows"));
        assert!(!super::tool_applies_to_platform(wsl_arch, "debian"));
        assert!(!super::tool_applies_to_platform(podman, "windows"));
        assert!(super::tool_applies_to_platform(podman, "debian"));
        assert!(super::tool_applies_to_platform(podman, "arch"));
    }

    #[test]
    fn validates_bioconda_channel_order() {
        let correct = vec![
            "https://conda.anaconda.org/conda-forge/win-64".to_owned(),
            "https://conda.anaconda.org/bioconda/win-64".to_owned(),
        ];
        let reversed = vec![correct[1].clone(), correct[0].clone()];

        assert!(conda_channel_order_valid(&correct));
        assert!(!conda_channel_order_valid(&reversed));
        assert!(!conda_channel_order_valid(&correct[..1]));
    }

    #[test]
    fn plans_missing_windows_sequence_search_tools() {
        let audit = EnvironmentAudit {
            platform: PlatformInfo {
                os: "windows".to_owned(),
                family: "windows".to_owned(),
                arch: "x86_64".to_owned(),
                supported: true,
            },
            tools: vec![ToolCheck {
                id: "wsl-debian".to_owned(),
                display_name: "WSL Debian".to_owned(),
                category: "execution".to_owned(),
                available: false,
                command: None,
                version: None,
                discovered_outside_path: false,
            }],
            execution_backends: ExecutionBackendAudit {
                policy: "test".to_owned(),
                required_any_of: vec![
                    "wsl-arch".to_owned(),
                    "wsl-debian".to_owned(),
                    "docker".to_owned(),
                ],
                available: Vec::new(),
                ready: false,
            },
            conda: None,
            summary: AuditSummary {
                available: 0,
                missing: 1,
            },
            warnings: Vec::new(),
        };

        let plan = plan_environment("sequence-search", &audit).expect("valid plan");
        assert_eq!(plan.actions.len(), 2);
        assert!(
            plan.actions
                .iter()
                .all(|action| action.state == PlanActionState::Install)
        );
        assert!(plan.requires_confirmation);
    }

    #[test]
    fn rewrites_only_github_urls() {
        let proxy = Some("https://gh.927223.xyz/");
        assert_eq!(
            apply_github_proxy("https://github.com/example/tool/releases", proxy),
            "https://gh.927223.xyz/https://github.com/example/tool/releases"
        );
        assert_eq!(
            apply_github_proxy("https://example.org/tool", proxy),
            "https://example.org/tool"
        );
    }

    #[test]
    fn decodes_utf16_probe_output() {
        let bytes = "Debian\r\n"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();

        assert_eq!(decode_output(&bytes), "Debian\r\n");
    }

    #[test]
    fn selects_the_matching_wsl_distribution() {
        let output = "arch-linux-current\r\nDebian\r\n";
        assert_eq!(
            first_matching_output_line(output, "debian"),
            Some("Debian".to_owned())
        );
    }

    #[test]
    fn detects_debian_and_arch_from_one_wsl_listing() {
        let catalog = load_catalog().expect("valid embedded catalog");
        let output = "arch-linux-current\r\ndebian-bookworm\r\n";
        let arch = probe_wsl_provider(
            catalog
                .tools
                .iter()
                .find(|tool| tool.id == "wsl-arch")
                .expect("WSL Arch tool"),
            Some(output),
        );
        let debian = probe_wsl_provider(
            catalog
                .tools
                .iter()
                .find(|tool| tool.id == "wsl-debian")
                .expect("WSL Debian tool"),
            Some(output),
        );

        assert!(arch.available);
        assert_eq!(arch.version.as_deref(), Some("arch-linux-current"));
        assert!(debian.available);
        assert_eq!(debian.version.as_deref(), Some("debian-bookworm"));
    }

    #[test]
    fn windows_genomics_plan_uses_an_existing_arch_provider() {
        let audit = windows_audit_with_tools(vec![available_tool("wsl-arch")]);
        let plan = plan_environment("genomics-cli", &audit).expect("valid plan");

        assert!(plan.actions.iter().all(|action| {
            action.execution_provider.as_deref() == Some("wsl-arch")
                && action
                    .strategy
                    .as_deref()
                    .is_some_and(|strategy| strategy == "wsl-arch/pacman")
        }));
        assert!(plan.actions.iter().all(|action| {
            action
                .source_url
                .as_deref()
                .is_some_and(|url| url.contains("archlinux.org"))
        }));
    }

    #[test]
    fn windows_genomics_plan_falls_back_to_existing_debian_provider() {
        let audit = windows_audit_with_tools(vec![available_tool("wsl-debian")]);
        let plan = plan_environment("genomics-cli", &audit).expect("valid plan");

        assert!(plan.actions.iter().all(|action| {
            action.execution_provider.as_deref() == Some("wsl-debian")
                && action
                    .strategy
                    .as_deref()
                    .is_some_and(|strategy| strategy == "wsl-debian/apt")
        }));
    }

    #[test]
    fn containers_plan_does_not_propose_extra_backends_when_one_is_ready() {
        let audit = windows_audit_with_tools(vec![available_tool("wsl-debian")]);
        let plan = plan_environment("containers", &audit).expect("valid plan");

        assert_eq!(plan.actions.len(), 1);
        assert_eq!(plan.actions[0].tool_id, "wsl-debian");
        assert_eq!(plan.actions[0].state, PlanActionState::Available);
        assert!(!plan.requires_confirmation);
        assert!(plan.warnings.is_empty());
    }

    #[test]
    fn windows_accepts_either_wsl_or_docker_backend() {
        let platform = PlatformInfo {
            os: "windows".to_owned(),
            family: "windows".to_owned(),
            arch: "x86_64".to_owned(),
            supported: true,
        };
        let tools = vec![ToolCheck {
            id: "docker".to_owned(),
            display_name: "Docker".to_owned(),
            category: "execution".to_owned(),
            available: true,
            command: Some("docker".to_owned()),
            version: Some("Docker version test".to_owned()),
            discovered_outside_path: false,
        }];

        let summary = summarize_execution_backends(&platform, &tools);
        assert!(summary.ready);
        assert_eq!(summary.available, ["docker"]);
        assert_eq!(
            summary.required_any_of,
            ["wsl-arch", "wsl-debian", "docker"]
        );
    }

    #[test]
    fn linux_policy_checks_docker_and_podman() {
        let platform = PlatformInfo {
            os: "linux".to_owned(),
            family: "debian".to_owned(),
            arch: "x86_64".to_owned(),
            supported: true,
        };

        let summary = summarize_execution_backends(&platform, &[]);
        assert!(!summary.ready);
        assert_eq!(summary.required_any_of, ["docker", "podman"]);
    }

    fn available_tool(id: &str) -> ToolCheck {
        ToolCheck {
            id: id.to_owned(),
            display_name: id.to_owned(),
            category: "execution".to_owned(),
            available: true,
            command: Some("test".to_owned()),
            version: Some("test".to_owned()),
            discovered_outside_path: false,
        }
    }

    fn windows_audit_with_tools(tools: Vec<ToolCheck>) -> EnvironmentAudit {
        let execution_backends = summarize_execution_backends(
            &PlatformInfo {
                os: "windows".to_owned(),
                family: "windows".to_owned(),
                arch: "x86_64".to_owned(),
                supported: true,
            },
            &tools,
        );
        EnvironmentAudit {
            platform: PlatformInfo {
                os: "windows".to_owned(),
                family: "windows".to_owned(),
                arch: "x86_64".to_owned(),
                supported: true,
            },
            summary: AuditSummary {
                available: tools.len(),
                missing: 0,
            },
            tools,
            execution_backends,
            conda: None,
            warnings: Vec::new(),
        }
    }
}
