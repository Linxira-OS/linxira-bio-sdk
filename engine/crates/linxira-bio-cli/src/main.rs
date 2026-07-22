#![forbid(unsafe_code)]

use linxira_bio_core::dataset::{DatasetInspection, DatasetSupport, inspect_dataset};
use linxira_bio_core::environment::{
    EnvironmentAudit, EnvironmentMode, EnvironmentPlan, EnvironmentPlanOptions, PlanActionState,
    audit_environment, parse_environment_mode, plan_environment_with_options,
};
use linxira_bio_core::fastq::{FastqQcMetrics, FastqQcOptions, QualityEncodingMode, fastq_qc_path};
use linxira_bio_core::runtime::{RuntimeProviderStatus, load_runtime_catalog};
use linxira_bio_core::sequence::{SequenceStats, fasta_stats_path};
use linxira_bio_core::variant::{VcfStats, vcf_stats_path};
use linxira_bio_export::export_json_file;
use linxira_bio_protocol::{AnalysisResult, ExecutionMode};
use std::env;
use std::error::Error;
use std::path::{Path, PathBuf};
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
        [help] if matches!(help.as_str(), "-h" | "--help") => {
            println!("{}", usage());
            Ok(())
        }
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
        [environment, plan, arguments @ ..] if environment == "environment" && plan == "plan" => {
            print_environment_plan(arguments)
        }
        [fastq, qc, arguments @ ..] if fastq == "fastq" && qc == "qc" => print_fastq_qc(arguments),
        [runtime, catalog] if runtime == "runtime" && catalog == "catalog" => {
            print_runtime_catalog(false)
        }
        [runtime, catalog, json]
            if runtime == "runtime" && catalog == "catalog" && json == "--json" =>
        {
            print_runtime_catalog(true)
        }
        [dataset, inspect, path] if dataset == "dataset" && inspect == "inspect" => {
            print_dataset_inspection(path, false)
        }
        [dataset, inspect, path, json]
            if dataset == "dataset" && inspect == "inspect" && json == "--json" =>
        {
            print_dataset_inspection(path, true)
        }
        [export, table, input, output] if export == "export" && table == "table" => {
            print_table_export(input, output, false)
        }
        [export, table, input, output, json]
            if export == "export" && table == "table" && json == "--json" =>
        {
            print_table_export(input, output, true)
        }
        [sequence, stats, path] if sequence == "sequence" && stats == "stats" => {
            print_sequence_stats(path, false)
        }
        [sequence, stats, path, json]
            if sequence == "sequence" && stats == "stats" && json == "--json" =>
        {
            print_sequence_stats(path, true)
        }
        [variant, stats, path] if variant == "variant" && stats == "stats" => {
            print_variant_stats(path, false)
        }
        [variant, stats, path, json]
            if variant == "variant" && stats == "stats" && json == "--json" =>
        {
            print_variant_stats(path, true)
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

fn print_environment_plan(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let (profile, options, json) = parse_environment_plan_arguments(arguments)?;
    let audit = audit_environment()?;
    let plan = plan_environment_with_options(&profile, &audit, &options)?;
    if json {
        print_analysis_json("environment-plan", "environment.plan.v1", plan)?;
    } else {
        print_plan_text(&plan);
    }
    Ok(())
}

fn parse_environment_plan_arguments(
    arguments: &[String],
) -> Result<(String, EnvironmentPlanOptions, bool), Box<dyn Error>> {
    let mut profile = None;
    let mut mode = EnvironmentMode::ManagedUser;
    let mut project_root = None;
    let mut json = false;
    let mut index = 0;

    while index < arguments.len() {
        match arguments[index].as_str() {
            "--json" => json = true,
            "--mode" => {
                index += 1;
                let value = arguments.get(index).ok_or("--mode requires a value")?;
                mode = parse_environment_mode(value)?;
            }
            "--project-root" => {
                index += 1;
                let value = arguments
                    .get(index)
                    .ok_or("--project-root requires a path")?;
                project_root = Some(PathBuf::from(value));
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown environment plan option: {value}").into());
            }
            value if profile.is_none() => profile = Some(value.to_owned()),
            value => return Err(format!("unexpected environment plan argument: {value}").into()),
        }
        index += 1;
    }

    if mode != EnvironmentMode::ProjectIsolated && project_root.is_some() {
        return Err("--project-root is only valid with --mode project-isolated".into());
    }

    Ok((
        profile.unwrap_or_else(|| "full-local".to_owned()),
        EnvironmentPlanOptions { mode, project_root },
        json,
    ))
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
    println!("mode: {}", plan.mode.as_str());
    if let Some(target_root) = &plan.target_root {
        println!("target: {target_root}");
    }
    println!("{}", plan.description);
    for action in &plan.actions {
        let state = match action.state {
            PlanActionState::Available => "available",
            PlanActionState::Install => "install",
            PlanActionState::Alternative => "alternative",
            PlanActionState::Missing => "missing",
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
    for blocker in &plan.transaction.blockers {
        println!("blocked: {blocker}");
    }
    if plan.requires_confirmation {
        println!("No changes were applied. This is a transaction preview only.");
    }
}

fn print_sequence_stats(path: &str, json: bool) -> Result<(), Box<dyn Error>> {
    let stats = fasta_stats_path(Path::new(path))?;

    if json {
        print_stats_json(&stats)?;
    } else {
        print_stats_text(&stats);
    }
    Ok(())
}

fn print_fastq_qc(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut path = None;
    let mut json = false;
    let mut options = FastqQcOptions::default();
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--json" => json = true,
            "--max-cycles" => {
                index += 1;
                options.max_cycles = arguments
                    .get(index)
                    .ok_or("--max-cycles requires a value")?
                    .parse()?;
            }
            "--quality-encoding" => {
                index += 1;
                options.quality_encoding = match arguments
                    .get(index)
                    .ok_or("--quality-encoding requires a value")?
                    .as_str()
                {
                    "auto" => QualityEncodingMode::Auto,
                    "phred+33" => QualityEncodingMode::Phred33,
                    "phred+64" => QualityEncodingMode::Phred64,
                    value => return Err(format!("unsupported quality encoding: {value}").into()),
                };
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown FASTQ QC option: {value}").into());
            }
            value if path.is_none() => path = Some(value),
            value => return Err(format!("unexpected FASTQ QC argument: {value}").into()),
        }
        index += 1;
    }
    let path = path.ok_or("fastq qc requires an input path")?;
    let metrics = fastq_qc_path(Path::new(path), options)?;
    if json {
        print_analysis_json("fastq-qc", "fastq.qc.v1", metrics)?;
    } else {
        print_fastq_qc_text(&metrics);
    }
    Ok(())
}

fn print_fastq_qc_text(metrics: &FastqQcMetrics) {
    println!("read_count\t{}", metrics.read_count);
    println!("total_bases\t{}", metrics.total_bases);
    println!("mean_length\t{:.6}", metrics.mean_length);
    println!("gc_percent\t{:.6}", metrics.gc_percent);
    println!("mean_quality\t{:.6}", metrics.mean_quality);
    println!("q20_percent\t{:.6}", metrics.q20_percent);
    println!("q30_percent\t{:.6}", metrics.q30_percent);
    println!("quality_encoding\t{:?}", metrics.quality_encoding);
    for warning in &metrics.warnings {
        println!("warning\t{warning}");
    }
}

fn print_variant_stats(path: &str, json: bool) -> Result<(), Box<dyn Error>> {
    let stats = vcf_stats_path(Path::new(path))?;
    if json {
        print_analysis_json("variant-stats", "variant.stats.v1", stats)?;
    } else {
        print_variant_stats_text(&stats);
    }
    Ok(())
}

fn print_variant_stats_text(stats: &VcfStats) {
    println!("record_count\t{}", stats.record_count);
    println!("sample_count\t{}", stats.sample_count);
    println!("pass_record_count\t{}", stats.pass_record_count);
    println!("filtered_record_count\t{}", stats.filtered_record_count);
    println!("snp_count\t{}", stats.snp_count);
    println!("indel_count\t{}", stats.indel_count);
    println!(
        "multiallelic_record_count\t{}",
        stats.multiallelic_record_count
    );
    if let Some(ratio) = stats.ti_tv_ratio {
        println!("ti_tv_ratio\t{ratio:.6}");
    }
    for warning in &stats.warnings {
        println!("warning\t{warning}");
    }
}

fn print_dataset_inspection(path: &str, json: bool) -> Result<(), Box<dyn Error>> {
    let inspection = inspect_dataset(Path::new(path))?;
    if json {
        print_analysis_json("dataset-inspect", "dataset.inspect.v1", inspection)?;
    } else {
        print_inspection_text(&inspection);
    }
    Ok(())
}

fn print_table_export(input: &str, output: &str, json: bool) -> Result<(), Box<dyn Error>> {
    let receipt = export_json_file(Path::new(input), Path::new(output))?;
    if json {
        print_analysis_json("table-export", "table.export.v1", receipt)?;
    } else {
        println!("{}", receipt.output_path);
    }
    Ok(())
}

fn print_inspection_text(inspection: &DatasetInspection) {
    let support = match inspection.support {
        DatasetSupport::Supported => "supported",
        DatasetSupport::RecognizedUnsupported => "recognized, not yet supported",
        DatasetSupport::Unknown => "unknown",
    };
    println!("file\t{}", inspection.path.display());
    println!("format\t{}", inspection.format);
    println!("compression\t{:?}", inspection.compression);
    println!("support\t{support}");
    println!("size_bytes\t{}", inspection.size_bytes);
    if let Some(preview) = &inspection.preview {
        println!("preview_records\t{}", preview.records_shown);
        println!("preview_truncated\t{}", preview.truncated);
    }
    for warning in &inspection.warnings {
        println!("warning\t{}: {}", warning.code, warning.message);
    }
    for error in &inspection.errors {
        println!("error\t{}: {}", error.code, error.message);
    }
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
    "usage:\n  linxira-bio capabilities [--json]\n  linxira-bio doctor [--json]\n  linxira-bio environment audit [--json]\n  linxira-bio environment plan [PROFILE] [--mode MODE] [--project-root PATH] [--json]\n  linxira-bio runtime catalog [--json]\n  linxira-bio dataset inspect <input> [--json]\n  linxira-bio sequence stats <input.fasta[.gz]> [--json]\n  linxira-bio fastq qc <input.fastq[.gz]> [--quality-encoding MODE] [--max-cycles N] [--json]\n  linxira-bio variant stats <input.vcf[.gz]> [--json]\n  linxira-bio export table <input.json> <output.csv|tsv|json|jsonl|xlsx> [--json]"
}
