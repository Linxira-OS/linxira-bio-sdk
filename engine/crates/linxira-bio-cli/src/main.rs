#![forbid(unsafe_code)]

use linxira_bio_core::environment::{
    EnvironmentAudit, EnvironmentPlan, PlanActionState, audit_environment, plan_environment,
};
use linxira_bio_core::runtime::{RuntimeProviderStatus, load_runtime_catalog};
use linxira_bio_core::sequence::{SequenceStats, fasta_stats};
use linxira_bio_protocol::{AnalysisResult, ExecutionMode};
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::process::ExitCode;

const CAPABILITY_CATALOG: &str = include_str!("../../../../capabilities/catalog.json");

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(2)
        }
    }
}

fn run(arguments: Vec<String>) -> Result<(), Box<dyn Error>> {
    match arguments.as_slice() {
        [command] if command == "capabilities" => print_capabilities(false),
        [command, json] if command == "capabilities" && json == "--json" => {
            print_capabilities(true)
        }
        [command] if command == "doctor" => print_doctor(false),
        [command, json] if command == "doctor" && json == "--json" => print_doctor(true),
        [environment, audit] if environment == "environment" && audit == "audit" => {
            print_environment_audit(false)
        }
        [environment, audit, json]
            if environment == "environment" && audit == "audit" && json == "--json" =>
        {
            print_environment_audit(true)
        }
        [environment, plan] if environment == "environment" && plan == "plan" => {
            print_environment_plan("full-local", false)
        }
        [environment, plan, json]
            if environment == "environment" && plan == "plan" && json == "--json" =>
        {
            print_environment_plan("full-local", true)
        }
        [environment, plan, profile] if environment == "environment" && plan == "plan" => {
            print_environment_plan(profile, false)
        }
        [environment, plan, profile, json]
            if environment == "environment" && plan == "plan" && json == "--json" =>
        {
            print_environment_plan(profile, true)
        }
        [runtime, catalog] if runtime == "runtime" && catalog == "catalog" => {
            print_runtime_catalog(false)
        }
        [runtime, catalog, json]
            if runtime == "runtime" && catalog == "catalog" && json == "--json" =>
        {
            print_runtime_catalog(true)
        }
        [sequence, stats, path] if sequence == "sequence" && stats == "stats" => {
            print_sequence_stats(path, false)
        }
        [sequence, stats, path, json]
            if sequence == "sequence" && stats == "stats" && json == "--json" =>
        {
            print_sequence_stats(path, true)
        }
        _ => Err(usage().into()),
    }
}

fn print_capabilities(json: bool) -> Result<(), Box<dyn Error>> {
    if json {
        println!("{CAPABILITY_CATALOG}");
    } else {
        let catalog: serde_json::Value = serde_json::from_str(CAPABILITY_CATALOG)?;
        println!("Available:");
        if let Some(capabilities) = catalog
            .get("capabilities")
            .and_then(serde_json::Value::as_array)
        {
            for capability in capabilities.iter().filter(|capability| {
                capability.get("status").and_then(serde_json::Value::as_str) == Some("available")
            }) {
                if let Some(id) = capability.get("id").and_then(serde_json::Value::as_str) {
                    println!("  {id}");
                }
            }
        }
        println!();
        println!("Run with --json for the complete catalog, including planned capabilities.");
    }
    Ok(())
}

fn print_runtime_catalog(json: bool) -> Result<(), Box<dyn Error>> {
    let catalog = load_runtime_catalog()?;
    if json {
        println!("{}", serde_json::to_string(&catalog)?);
        return Ok(());
    }

    println!("Managed runtime providers (read-only catalog):");
    for provider in catalog.providers {
        let state = match provider.status {
            RuntimeProviderStatus::Cataloged => "cataloged",
            RuntimeProviderStatus::Installable => "installable",
            RuntimeProviderStatus::Deprecated => "deprecated",
        };
        let default = if provider.default { " [default]" } else { "" };
        println!(
            "  {}: {} via {} ({state}){default}",
            provider.runtime, provider.display_name, provider.manager
        );
    }
    println!("Installation is not implemented; environment.apply.v1 remains planned.");
    Ok(())
}

fn print_doctor(json: bool) -> Result<(), Box<dyn Error>> {
    let audit = audit_environment()?;

    if json {
        let tools = [
            "rust",
            "uv",
            "pixi",
            "conda",
            "miniforge",
            "python",
            "r",
            "java",
            "samtools",
            "bcftools",
            "bedtools",
            "wsl-arch",
            "wsl-debian",
            "docker",
            "podman",
        ]
        .iter()
        .filter_map(|tool_id| audit.tools.iter().find(|tool| tool.id == *tool_id))
        .map(|tool| {
            let name = if tool.id == "rust" { "rustc" } else { &tool.id };
            serde_json::json!({
                "name": name,
                "available": tool.available,
                "version": tool.version,
            })
        })
        .collect::<Vec<_>>();
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "schema_version": "1",
                "product": "linxira-bio-sdk",
                "os": audit.platform.os,
                "arch": audit.platform.arch,
                "tools": tools,
            }))?
        );
    } else {
        print_audit_text("Linxira Bio SDK doctor", &audit);
    }
    Ok(())
}

fn print_environment_audit(json: bool) -> Result<(), Box<dyn Error>> {
    let audit = audit_environment()?;
    if json {
        print_analysis_json("environment-audit", "environment.audit.v1", audit)?;
    } else {
        print_audit_text("Linxira Bio environment audit", &audit);
    }
    Ok(())
}

fn print_environment_plan(profile: &str, json: bool) -> Result<(), Box<dyn Error>> {
    let audit = audit_environment()?;
    let plan = plan_environment(profile, &audit)?;
    if json {
        print_analysis_json("environment-plan", "environment.plan.v1", plan)?;
    } else {
        print_plan_text(&plan);
    }
    Ok(())
}

fn print_analysis_json<T>(job_id: &str, capability: &str, result: T) -> Result<(), Box<dyn Error>>
where
    T: serde::Serialize,
{
    let result = AnalysisResult::ok(job_id, capability, result, ExecutionMode::LocalCpu);
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}

fn print_audit_text(title: &str, audit: &EnvironmentAudit) {
    println!("{title}");
    println!(
        "platform: {} {} ({})",
        audit.platform.family, audit.platform.arch, audit.platform.os
    );
    for tool in &audit.tools {
        let state = if tool.available {
            "available"
        } else {
            "not found"
        };
        let version = tool
            .version
            .as_deref()
            .map(|value| format!(" - {value}"))
            .unwrap_or_default();
        println!("{}: {state}{version}", tool.display_name);
    }
}

fn print_plan_text(plan: &EnvironmentPlan) {
    println!("Environment profile: {}", plan.profile);
    println!("{}", plan.description);
    for action in &plan.actions {
        let state = match action.state {
            PlanActionState::Available => "available",
            PlanActionState::Install => "install",
            PlanActionState::Unsupported => "unsupported",
        };
        let method = action
            .strategy
            .as_deref()
            .map(|strategy| format!(" via {strategy}"))
            .unwrap_or_default();
        println!("{}: {state}{method}", action.display_name);
    }
    for warning in &plan.warnings {
        println!("warning: {warning}");
    }
    if plan.requires_confirmation {
        println!("No changes were applied. Installation requires explicit confirmation.");
    }
}

fn print_sequence_stats(path: &str, json: bool) -> Result<(), Box<dyn Error>> {
    let file = File::open(Path::new(path))?;
    let stats = fasta_stats(BufReader::new(file))?;

    if json {
        print_stats_json(&stats)?;
    } else {
        print_stats_text(&stats);
    }
    Ok(())
}

fn print_stats_text(stats: &SequenceStats) {
    println!("sequence_count\t{}", stats.sequence_count);
    println!("total_bases\t{}", stats.total_bases);
    println!("min_length\t{}", stats.min_length);
    println!("max_length\t{}", stats.max_length);
    println!("mean_length\t{:.6}", stats.mean_length);
    println!("n50\t{}", stats.n50);
    println!("l50\t{}", stats.l50);
    println!("au_n\t{:.6}", stats.au_n);
    println!("gc_percent\t{:.6}", stats.gc_percent);
    println!("n_count\t{}", stats.n_count);
    println!("n_percent\t{:.6}", stats.n_percent);
}

fn print_stats_json(stats: &SequenceStats) -> Result<(), Box<dyn Error>> {
    let result = AnalysisResult::ok("cli", "sequence.stats.v1", stats, ExecutionMode::LocalCpu);
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}

fn usage() -> &'static str {
    "usage:\n  linxira-bio capabilities [--json]\n  linxira-bio doctor [--json]\n  linxira-bio environment audit [--json]\n  linxira-bio environment plan [PROFILE] [--json]\n  linxira-bio runtime catalog [--json]\n  linxira-bio sequence stats <input.fasta> [--json]"
}
