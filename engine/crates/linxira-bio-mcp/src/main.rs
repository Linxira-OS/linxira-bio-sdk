#![forbid(unsafe_code)]

use linxira_bio_protocol::{ExecutionMode, ExecutionRequest, JobRequest};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Catalog (runtime-loaded with an embedded snapshot fallback)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct Catalog {
    #[serde(default)]
    capabilities: Vec<CapabilityEntry>,
}

/// Load the capability catalog from the runtime catalog root
/// (`LINXIRA_BIO_WORKFLOW_ROOT` when set, else `<cwd>/workflows`, mirroring the
/// CLI) and fall back to the embedded snapshot when the runtime file is absent.
fn load_capability_catalog() -> Catalog {
    let root = env::var_os("LINXIRA_BIO_WORKFLOW_ROOT")
        .map(PathBuf::from)
        .filter(|value| !value.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from("workflows"));
    let runtime_text = match fs::canonicalize(&root) {
        Ok(root) => {
            let candidate = root.join("capabilities").join("catalog.json");
            if candidate.is_file() {
                fs::read_to_string(&candidate).ok()
            } else {
                None
            }
        }
        Err(_) => None,
    };
    let text = runtime_text
        .unwrap_or_else(|| include_str!("../../../../capabilities/catalog.json").to_owned());
    serde_json::from_str(&text).unwrap_or_else(|error| {
        eprintln!("fatal: failed to parse catalog.json: {error}");
        std::process::exit(1);
    })
}

#[derive(Debug, Deserialize)]
struct CapabilityEntry {
    id: String,
    status: String,
    category: String,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    input_formats: Option<Vec<String>>,
    #[serde(default)]
    #[allow(dead_code)]
    output_formats: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// MCP JSON-RPC types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: Option<String>,
    id: Option<serde_json::Value>,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct InitializeParams {
    #[serde(default)]
    protocol_version: Option<String>,
    #[serde(default)]
    capabilities: serde_json::Value,
    #[serde(default)]
    client_info: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct ToolCallParams {
    name: String,
    #[serde(default)]
    arguments: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct ToolDefinition {
    name: String,
    description: String,
    #[serde(rename = "inputSchema")]
    input_schema: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct ToolCallContent {
    #[serde(rename = "type")]
    content_type: &'static str,
    text: String,
}

#[derive(Debug, Serialize)]
struct ToolCallResult {
    content: Vec<ToolCallContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_error: Option<bool>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_error(
    id: Option<serde_json::Value>,
    code: i32,
    message: impl Into<String>,
) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: None,
        error: Some(JsonRpcError {
            code,
            message: message.into(),
            data: None,
        }),
    }
}

fn make_result(id: Option<serde_json::Value>, result: serde_json::Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: Some(result),
        error: None,
    }
}

fn write_response(response: &JsonRpcResponse) {
    let mut stdout = io::stdout().lock();
    let line = serde_json::to_string(response).unwrap_or_else(|_| {
        r#"{"jsonrpc":"2.0","error":{"code":-32603,"message":"failed to serialize response"}}"#
            .to_owned()
    });
    let _ = writeln!(stdout, "{line}");
    let _ = stdout.flush();
}

/// Locate the `linxira-bio-worker` binary.
fn find_worker_binary() -> Option<PathBuf> {
    // CARGO_BIN_EXE_linxira-bio-worker is set by Cargo when this crate is a
    // dependency of the worker's package (it is not — they are siblings), but
    // it may be set by an external runner.  Try it anyway.
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_linxira-bio-worker") {
        let p = PathBuf::from(&path);
        if p.exists() {
            return Some(p);
        }
    }
    // Look next to the current executable.
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        for name in &["linxira-bio-worker", "linxira-bio-worker.exe"] {
            let candidate = dir.join(name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Parse a command template like `linxira-bio sequence stats <input.fasta> --json`
/// and return (positional_args, named_params).
/// positional_args: the text inside `<>`, e.g. `["input.fasta"]`
/// named_params:   `(name, default_value)` pairs, e.g. `[("threshold", "0.5")]`
fn parse_command_template(cmd: &str) -> (Vec<String>, Vec<(String, String)>) {
    let mut positional = Vec::new();
    let mut named = Vec::new();

    let tokens: Vec<&str> = cmd.split_whitespace().collect();
    let mut i = 0;
    while i < tokens.len() {
        let token = tokens[i];
        if token.starts_with('<') && token.ends_with('>') {
            positional.push(token[1..token.len() - 1].to_owned());
        } else if let Some(name) = token.strip_prefix("--") {
            let name = name.to_owned();
            // Check if the next token looks like a value (not a flag and not a positional)
            if i + 1 < tokens.len() {
                let next = tokens[i + 1];
                if !next.starts_with("--") && !(next.starts_with('<') && next.ends_with('>')) {
                    named.push((name, next.to_owned()));
                    i += 1; // consume the value
                } else {
                    // Boolean flag
                    named.push((name, "true".to_owned()));
                }
            } else {
                named.push((name, "true".to_owned()));
            }
        }
        i += 1;
    }

    (positional, named)
}

/// Derive an input role name from a positional argument like "input.fasta"
/// or "reference.fasta".
fn role_from_positional(arg: &str) -> String {
    // Strip extension
    let stem = match arg.rsplit_once('.') {
        Some((base, _ext)) => base,
        None => arg,
    };
    // Use the stem as the role, but collapse common prefixes
    stem.to_owned()
}

/// Build a JSON Schema for the tool arguments from the capability.
fn build_input_schema(cap: &CapabilityEntry) -> serde_json::Value {
    let mut props = serde_json::Map::new();

    // ---- inputs property ----
    let mut input_props = serde_json::Map::new();
    let mut input_required = Vec::new();

    if let Some(ref cmd) = cap.command {
        let (positional, _named) = parse_command_template(cmd);
        for arg in &positional {
            let role = role_from_positional(arg);
            input_props.insert(
                role.clone(),
                serde_json::json!({
                    "type": "string",
                    "description": format!("File path for: {arg}")
                }),
            );
            input_required.push(serde_json::Value::String(role));
        }
    }

    let inputs_schema = serde_json::json!({
        "type": "object",
        "properties": input_props,
        "required": input_required,
        "additionalProperties": true
    });

    props.insert("inputs".to_owned(), inputs_schema);

    // ---- parameters property ----
    let mut param_props = serde_json::Map::new();
    if let Some(ref cmd) = cap.command {
        let (_positional, named) = parse_command_template(cmd);
        for (name, default) in &named {
            param_props.insert(
                name.clone(),
                serde_json::json!({
                    "type": "string",
                    "description": format!("Parameter --{name}"),
                    "default": default
                }),
            );
        }
    }

    let params_schema = serde_json::json!({
        "type": "object",
        "properties": param_props,
        "additionalProperties": true
    });

    props.insert("parameters".to_owned(), params_schema);

    serde_json::json!({
        "type": "object",
        "properties": props,
        "required": ["inputs"]
    })
}

/// Convert a capability ID to a valid MCP tool name (dots → underscores).
fn tool_name(cap_id: &str) -> String {
    cap_id.replace('.', "_")
}

/// Generate a human-readable description for a capability.
fn describe_capability(cap: &CapabilityEntry) -> String {
    let status_mark = match cap.status.as_str() {
        "available" => "",
        "planned" => " [planned]",
        "experimental" => " [experimental]",
        other => &format!(" [{}]", other),
    };

    let category_desc = match cap.category.as_str() {
        "sequence-io" => "Sequence I/O and statistics",
        "sequence-algorithms" => "Sequence algorithms (ORF, k-mer, translation)",
        "read-processing" => "FASTQ read processing",
        "read-qc" => "FASTQ quality control",
        "read-alignment" => "Read alignment",
        "alignment-files" => "Alignment file analysis (SAM/BAM/CRAM)",
        "genome-annotation" => "Genome annotation (GFF3/GTF)",
        "genome-intervals" => "Genomic interval operations (BED)",
        "variant-calling" => "Variant calling and analysis (VCF)",
        "expression" => "Expression analysis",
        "functional-enrichment" => "Functional enrichment analysis",
        "sequence-similarity" => "Sequence similarity search",
        "multiple-sequence-alignment" => "Multiple sequence alignment",
        "phylogenetics" => "Phylogenetic analysis",
        "motif-analysis" => "Sequence motif discovery",
        "comparative-genomics" => "Comparative genomics",
        "protein-properties" => "Protein property analysis",
        "protein-structure" => "Protein structure analysis",
        "structure-analysis" => "3D structure analysis",
        "set-analysis" => "Set overlap analysis",
        "data-management" => "Data management and export",
        "visualization" => "Scientific visualization",
        "medical-ruo" => "Medical analysis (RUO)",
        "system" => "System management",
        other => other,
    };

    format!("{}: {}{}", cap.id, category_desc, status_mark)
}

/// Build the list of MCP tool definitions from the catalog.
fn build_tool_definitions(catalog: &Catalog) -> Vec<ToolDefinition> {
    catalog
        .capabilities
        .iter()
        .filter_map(|cap| {
            let cmd = cap.command.as_ref()?;
            if cmd.trim().is_empty() {
                return None;
            }
            let name = tool_name(&cap.id);
            let description = describe_capability(cap);
            let input_schema = build_input_schema(cap);
            Some(ToolDefinition {
                name,
                description,
                input_schema,
            })
        })
        .collect()
}

/// Generate a unique job ID.
fn generate_job_id() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("mcp-{:x}", ts.as_nanos())
}

// ---------------------------------------------------------------------------
// Method handlers
// ---------------------------------------------------------------------------

fn handle_initialize(id: Option<serde_json::Value>, _params: serde_json::Value) -> JsonRpcResponse {
    let result = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "serverInfo": {
            "name": "linxira-bio-mcp",
            "version": env!("CARGO_PKG_VERSION")
        },
        "capabilities": {
            "tools": {},
            "resources": {}
        }
    });
    make_result(id, result)
}

fn handle_tools_list(id: Option<serde_json::Value>, catalog: &Catalog) -> JsonRpcResponse {
    let tools = build_tool_definitions(catalog);
    let result = serde_json::json!({ "tools": tools });
    make_result(id, result)
}

fn handle_tools_call(
    id: Option<serde_json::Value>,
    params: serde_json::Value,
    catalog: &Catalog,
    worker_path: &PathBuf,
) -> JsonRpcResponse {
    let call_params: ToolCallParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => return make_error(id, -32602, format!("invalid params: {e}")),
    };

    // Map MCP tool name back to capability ID
    let capability_id = call_params.name.replace('_', ".");
    let cap = match catalog.capabilities.iter().find(|c| c.id == capability_id) {
        Some(c) => c,
        None => {
            return make_error(id, -32602, format!("unknown tool: {}", call_params.name));
        }
    };

    if cap.command.is_none() {
        return make_error(
            id,
            -32602,
            format!("tool '{}' has no executable command", call_params.name),
        );
    }

    // Extract inputs and parameters from the tool arguments
    let arguments = call_params.arguments;
    let inputs: BTreeMap<String, String> = arguments
        .get("inputs")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
                .collect()
        })
        .unwrap_or_default();

    if inputs.is_empty() {
        return make_error(
            id,
            -32602,
            format!(
                "tool '{}' requires at least one input file",
                call_params.name
            ),
        );
    }

    let parameters = arguments
        .get("parameters")
        .cloned()
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

    // Build a JobRequest
    let job_id = generate_job_id();
    let request = JobRequest {
        schema_version: linxira_bio_protocol::SCHEMA_VERSION.to_owned(),
        job_id: job_id.clone(),
        capability: capability_id.clone(),
        inputs,
        execution: ExecutionRequest {
            mode: ExecutionMode::LocalCpu,
        },
        parameters,
    };

    // Write the request to a temp file
    let d = std::env::temp_dir().join("linxira-bio-mcp").join(&job_id);
    let temp_dir = {
        let _ = std::fs::create_dir_all(&d);
        d
    };
    let request_path = temp_dir.join("job-request.json");
    if let Err(e) = std::fs::write(
        &request_path,
        serde_json::to_string_pretty(&request).unwrap_or_default(),
    ) {
        return make_error(id, -32603, format!("failed to write job request: {e}"));
    }

    // Execute the worker
    let child = match Command::new(worker_path)
        .arg(&request_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return make_error(
                id,
                -32603,
                format!("failed to spawn worker '{}': {e}", worker_path.display()),
            );
        }
    };

    let output = match child.wait_with_output() {
        Ok(o) => o,
        Err(e) => {
            return make_error(id, -32603, format!("worker process error: {e}"));
        }
    };

    // Clean up the temp directory
    let _ = std::fs::remove_dir_all(&temp_dir);

    let stdout_str = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr_str = String::from_utf8_lossy(&output.stderr).into_owned();

    if !output.status.success() {
        let err_text = if stderr_str.is_empty() {
            stdout_str
        } else {
            stderr_str
        };
        return make_error(
            id,
            -32603,
            format!("worker exited with {}: {err_text}", output.status),
        );
    }

    // Try to parse the worker output as JSON, otherwise return it as text
    let result_text = stdout_str.trim().to_owned();
    let result_json: serde_json::Value = match serde_json::from_str(&result_text) {
        Ok(v) => v,
        Err(_) => serde_json::json!({ "raw_output": result_text }),
    };

    let content = ToolCallContent {
        content_type: "text",
        text: serde_json::to_string_pretty(&result_json).unwrap_or(result_text),
    };

    let call_result = ToolCallResult {
        content: vec![content],
        is_error: None,
    };

    make_result(id, serde_json::to_value(call_result).unwrap_or_default())
}

fn handle_resources_list(id: Option<serde_json::Value>) -> JsonRpcResponse {
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut resources = Vec::new();

    // List files in the current working directory (non-recursive, limited)
    if let Ok(entries) = std::fs::read_dir(&cwd) {
        for entry in entries.flatten().take(200) {
            let path = entry.path();
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let is_dir = path.is_dir();

            let mime_type = if is_dir {
                "inode/directory"
            } else {
                guess_mime_type(file_name)
            };

            let uri = if is_dir {
                format!("file:///{}/", path.display())
            } else {
                format!("file:///{}", path.display())
            };

            let resource = serde_json::json!({
                "uri": uri,
                "name": file_name,
                "mimeType": mime_type,
            });
            resources.push(resource);
        }
    }

    make_result(id, serde_json::json!({ "resources": resources }))
}

fn handle_resources_read(
    id: Option<serde_json::Value>,
    params: serde_json::Value,
) -> JsonRpcResponse {
    let uri = match params.get("uri").and_then(|v| v.as_str()) {
        Some(u) => u,
        None => return make_error(id, -32602, "missing uri parameter"),
    };

    // Strip file:/// prefix
    let path_str = uri.strip_prefix("file:///").unwrap_or(uri);
    let path = std::path::Path::new(path_str);

    if !path.exists() {
        return make_error(id, -32602, format!("file not found: {path_str}"));
    }

    if path.is_dir() {
        // Return directory listing as a resource
        let mut entries_list = Vec::new();
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten().take(200) {
                let name = entry.file_name().to_str().unwrap_or("").to_owned();
                let entry_type = if entry.path().is_dir() { "dir" } else { "file" };
                entries_list.push(format!("[{entry_type}] {name}"));
            }
        }
        let content = entries_list.join("\n");
        let result = serde_json::json!({
            "contents": [{
                "uri": uri,
                "mimeType": "text/plain",
                "text": content
            }]
        });
        return make_result(id, result);
    }

    // Read file content (limit to text files under 10MB)
    let metadata = match path.metadata() {
        Ok(m) => m,
        Err(e) => return make_error(id, -32603, format!("cannot read metadata: {e}")),
    };

    if metadata.len() > 10 * 1024 * 1024 {
        return make_error(id, -32603, "file too large (>10MB)");
    }

    match std::fs::read_to_string(path) {
        Ok(text) => {
            let mime_type =
                guess_mime_type(path.file_name().and_then(|n| n.to_str()).unwrap_or(""));
            let result = serde_json::json!({
                "contents": [{
                    "uri": uri,
                    "mimeType": mime_type,
                    "text": text
                }]
            });
            make_result(id, result)
        }
        Err(_e) => {
            // Try reading as binary for a size report
            let size = metadata.len();
            let result = serde_json::json!({
                "contents": [{
                    "uri": uri,
                    "mimeType": "application/octet-stream",
                    "text": format!("[Binary file: {size} bytes]")
                }]
            });
            make_result(id, result)
        }
    }
}

fn guess_mime_type(file_name: &str) -> &'static str {
    let lower = file_name.to_lowercase();
    if lower.ends_with(".fasta") || lower.ends_with(".fa") || lower.ends_with(".fna") {
        "text/x-fasta"
    } else if lower.ends_with(".fastq") || lower.ends_with(".fq") {
        "text/x-fastq"
    } else if lower.ends_with(".vcf") {
        "text/x-vcf"
    } else if lower.ends_with(".gff") || lower.ends_with(".gff3") {
        "text/x-gff3"
    } else if lower.ends_with(".gtf") {
        "text/x-gtf"
    } else if lower.ends_with(".bed") {
        "text/x-bed"
    } else if lower.ends_with(".sam") {
        "text/x-sam"
    } else if lower.ends_with(".bam") {
        "application/octet-stream"
    } else if lower.ends_with(".pdb") {
        "chemical/x-pdb"
    } else if lower.ends_with(".cif") || lower.ends_with(".mmcif") {
        "chemical/x-cif"
    } else if lower.ends_with(".csv") {
        "text/csv"
    } else if lower.ends_with(".tsv") {
        "text/tab-separated-values"
    } else if lower.ends_with(".json") {
        "application/json"
    } else if lower.ends_with(".txt") {
        "text/plain"
    } else if lower.ends_with(".nwk") || lower.ends_with(".newick") {
        "text/x-newick"
    } else if lower.ends_with(".gz") {
        "application/gzip"
    } else {
        "text/plain"
    }
}

// ---------------------------------------------------------------------------
// Main loop
// ---------------------------------------------------------------------------

fn main() {
    let catalog: Catalog = load_capability_catalog();

    let worker_path = match find_worker_binary() {
        Some(p) => p,
        None => {
            eprintln!("fatal: linxira-bio-worker binary not found");
            std::process::exit(1);
        }
    };

    // Log server identity to stderr (MCP protocol uses stdout for JSON-RPC)
    eprintln!(
        "linxira-bio-mcp v{} started, worker at {}",
        env!("CARGO_PKG_VERSION"),
        worker_path.display()
    );

    let stdin = io::stdin().lock();
    let reader = BufReader::new(stdin);

    for line_result in reader.lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(e) => {
                eprintln!("stdin read error: {e}");
                break;
            }
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let response = make_error(None, -32700, format!("parse error: {e}"));
                write_response(&response);
                continue;
            }
        };

        let method = request.method.as_str();

        // notifications/initialized has no id — do not respond
        let is_notification = request.id.is_none();

        match method {
            "initialize" => {
                let response = handle_initialize(request.id, request.params);
                write_response(&response);
            }
            "notifications/initialized" => {
                // No response for notifications
            }
            "tools/list" => {
                if is_notification {
                    continue;
                }
                let response = handle_tools_list(request.id, &catalog);
                write_response(&response);
            }
            "tools/call" => {
                if is_notification {
                    continue;
                }
                let response =
                    handle_tools_call(request.id, request.params, &catalog, &worker_path);
                write_response(&response);
            }
            "resources/list" => {
                if is_notification {
                    continue;
                }
                let response = handle_resources_list(request.id);
                write_response(&response);
            }
            "resources/read" => {
                if is_notification {
                    continue;
                }
                let response = handle_resources_read(request.id, request.params);
                write_response(&response);
            }
            "ping" => {
                if is_notification {
                    continue;
                }
                let response = make_result(request.id, serde_json::json!({}));
                write_response(&response);
            }
            _ => {
                if is_notification {
                    // Silently ignore unknown notifications
                    continue;
                }
                let response =
                    make_error(request.id, -32601, format!("method not found: {method}"));
                write_response(&response);
            }
        }
    }
}
