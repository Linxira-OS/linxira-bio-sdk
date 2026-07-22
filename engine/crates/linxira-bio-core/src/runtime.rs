use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{Display, Formatter};

const RUNTIME_CATALOG: &str = include_str!("../../../../runtimes/catalog.json");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeCatalog {
    pub schema_version: String,
    pub default_scope: String,
    pub providers: Vec<RuntimeProvider>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeProvider {
    pub id: String,
    pub runtime: String,
    pub display_name: String,
    pub status: RuntimeProviderStatus,
    pub manager: String,
    pub version_policy: String,
    pub default: bool,
    pub user_scoped: bool,
    pub platforms: Vec<String>,
    pub source_url: String,
    pub licenses: Vec<String>,
    pub health_checks: Vec<RuntimeHealthCheck>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeProviderStatus {
    Cataloged,
    Installable,
    Deprecated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeHealthCheck {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug)]
pub struct RuntimeCatalogError(String);

impl Display for RuntimeCatalogError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for RuntimeCatalogError {}

pub fn load_runtime_catalog() -> Result<RuntimeCatalog, RuntimeCatalogError> {
    let catalog: RuntimeCatalog = serde_json::from_str(RUNTIME_CATALOG).map_err(|error| {
        RuntimeCatalogError(format!("invalid embedded runtime catalog: {error}"))
    })?;
    validate_runtime_catalog(&catalog)?;
    Ok(catalog)
}

fn validate_runtime_catalog(catalog: &RuntimeCatalog) -> Result<(), RuntimeCatalogError> {
    if catalog.schema_version != "1" {
        return Err(RuntimeCatalogError(format!(
            "unsupported runtime catalog schema: {}",
            catalog.schema_version
        )));
    }
    if catalog.default_scope != "user" {
        return Err(RuntimeCatalogError(
            "managed runtimes must default to user scope".to_owned(),
        ));
    }

    let mut ids = BTreeSet::new();
    let mut defaults = BTreeMap::new();
    for provider in &catalog.providers {
        if !ids.insert(&provider.id) {
            return Err(RuntimeCatalogError(format!(
                "duplicate runtime provider: {}",
                provider.id
            )));
        }
        if !provider.user_scoped {
            return Err(RuntimeCatalogError(format!(
                "runtime provider {} is not user scoped",
                provider.id
            )));
        }
        if provider.default
            && defaults
                .insert(provider.runtime.as_str(), provider.id.as_str())
                .is_some()
        {
            return Err(RuntimeCatalogError(format!(
                "runtime {} has more than one default provider",
                provider.runtime
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{RuntimeProviderStatus, load_runtime_catalog};

    #[test]
    fn catalog_has_default_python_and_java_runtimes() {
        let catalog = load_runtime_catalog().expect("valid embedded runtime catalog");

        for runtime in ["python", "r", "java"] {
            assert!(
                catalog
                    .providers
                    .iter()
                    .any(|provider| provider.runtime == runtime && provider.default),
                "missing default provider for {runtime}"
            );
        }
    }

    #[test]
    fn catalog_does_not_claim_installation_is_implemented() {
        let catalog = load_runtime_catalog().expect("valid embedded runtime catalog");
        assert!(catalog.providers.iter().all(|provider| {
            provider.status == RuntimeProviderStatus::Cataloged && provider.user_scoped
        }));
    }
}
