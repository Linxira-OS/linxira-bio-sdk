use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
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
    pub summary: AuditSummary,
    pub warnings: Vec<String>,
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
    probes: Vec<ToolProbe>,
    install: BTreeMap<String, InstallSpec>,
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
    let tools = catalog.tools.iter().map(probe_tool).collect::<Vec<_>>();
    let available = tools.iter().filter(|tool| tool.available).count();
    let mut warnings = Vec::new();
    if !platform.supported {
        warnings.push(format!(
            "platform family '{}' is not in the Windows, Debian, or Arch support matrix",
            platform.family
        ));
    }

    Ok(EnvironmentAudit {
        platform,
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
        let available = checks
            .get(tool_id.as_str())
            .is_some_and(|tool| tool.available);
        let install = definition.install.get(&audit.platform.family);
        let state = if available {
            PlanActionState::Available
        } else if install.is_some() {
            PlanActionState::Install
        } else {
            PlanActionState::Unsupported
        };
        let source_url = install.map(|spec| spec.source_url.clone());
        let resolved_source_url = source_url
            .as_deref()
            .map(|url| apply_github_proxy(url, github_proxy.as_deref()));

        if state == PlanActionState::Unsupported {
            warnings.push(format!(
                "{} has no registered installation strategy for {}",
                definition.display_name, audit.platform.family
            ));
        }

        actions.push(InstallAction {
            tool_id: definition.id.clone(),
            display_name: definition.display_name.clone(),
            state,
            strategy: install.map(|spec| spec.strategy.clone()),
            package: install.map(|spec| spec.package.clone()),
            source_url,
            resolved_source_url,
            requires_admin: install.is_some_and(|spec| spec.requires_admin),
        });
    }

    let uses_wsl = actions
        .iter()
        .any(|action| action.strategy.as_deref() == Some("wsl-debian"));
    let wsl_available = checks.get("wsl").is_some_and(|tool| tool.available);
    if uses_wsl && !wsl_available {
        warnings.push(
            "the Windows plan requires a configured WSL Debian environment for Unix-native tools"
                .to_owned(),
        );
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

fn probe_tool(definition: &ToolDefinition) -> ToolCheck {
    for probe in &definition.probes {
        let Ok(output) = Command::new(&probe.program).args(&probe.args).output() else {
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
            command: Some(probe.program.clone()),
            version,
        };
    }

    ToolCheck {
        id: definition.id.clone(),
        display_name: definition.display_name.clone(),
        category: definition.category.clone(),
        available: false,
        command: None,
        version: None,
    }
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
        AuditSummary, EnvironmentAudit, PlanActionState, PlatformInfo, ToolCheck,
        apply_github_proxy, decode_output, first_matching_output_line, load_catalog,
        plan_environment,
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
            }
        }
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
                id: "wsl".to_owned(),
                display_name: "WSL".to_owned(),
                category: "execution".to_owned(),
                available: false,
                command: None,
                version: None,
            }],
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
}
