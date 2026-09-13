"""enrichment.overrepresentation.v1 — Python pack implementation.

A faithful port of the engine's over-representation analysis
(engine/crates/linxira-bio-core/src/functional.rs): the association union is
the background, the query is intersected with it, per-term hypergeometric
upper-tail p-values use log-factorial point probabilities with the same
recurrence summation, and Benjamini-Hochberg uses the same tie-breaking and
running minimum. The engine, not an external library, is the specification.
"""
from __future__ import annotations

import csv
import gzip
import io
import math
import platform
from pathlib import Path

CAPABILITY = "enrichment.overrepresentation.v1"
INPUT_ROLES = ("genes", "associations")
PARAMETERS = ("min_overlap", "max_terms", "include_genes")


def software() -> list[dict]:
    return [
        {"name": "CPython", "version": platform.python_version()},
        {"name": "stdlib csv/gzip/math", "version": platform.python_version()},
    ]


MISSING = {"", "-", "na", "n/a", "none", "null", "."}


def read_text(path: str) -> str:
    data = Path(path).read_bytes()
    if data[:2] == b"\x1f\x8b":
        data = gzip.decompress(data)
    return data.decode("utf-8")


def delimiter_for(name: str, first_line: str) -> str:
    lowered = name.lower()
    if lowered.endswith((".csv", ".csv.gz")):
        return ","
    if lowered.endswith((".tsv", ".tab", ".tsv.gz")):
        return "\t"
    if "\t" in first_line:
        return "\t"
    if "," in first_line:
        return ","
    return "\t"


GENE_HEADERS = {"gene", "gene_id", "query", "query_id", "protein_id", "id"}


def read_gene_set(path: str) -> set[str]:
    lines = [
        line.strip()
        for line in read_text(path).splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    ]
    if not lines:
        raise ValueError("query gene list is empty")
    delimiter = "\t" if "\t" in lines[0] else ("," if "," in lines[0] else None)
    genes = set()
    for index, line in enumerate(lines):
        value = (line.split(delimiter)[0] if delimiter else line).strip()
        if index == 0 and value.lower() in GENE_HEADERS:
            continue
        if not value or value.lower() in MISSING:
            continue
        genes.add(value)
    if not genes:
        raise ValueError("query gene list is empty")
    return genes


def resolve_column(headers: list[str], candidates: list[str], role: str) -> int:
    lowered = [h.strip().lower() for h in headers]
    for candidate in candidates:
        if candidate in lowered:
            return lowered.index(candidate)
    raise ValueError(f"association table lacks the {role} column")


def optional_column(headers: list[str], candidates: list[str]) -> int | None:
    lowered = [h.strip().lower() for h in headers]
    for candidate in candidates:
        if candidate in lowered:
            return lowered.index(candidate)
    return None


def optional_value(record: list[str], column: int | None) -> str | None:
    if column is None or column >= len(record):
        return None
    value = record[column].strip()
    if not value or value.lower() in MISSING:
        return None
    return value


def read_associations(path: str) -> dict[str, dict]:
    text = read_text(path)
    rows = [line for line in text.splitlines() if line.strip() and not line.lstrip().startswith("#")]
    if not rows:
        raise ValueError("association table is empty")
    delimiter = delimiter_for(Path(path).name, rows[0])
    reader = csv.reader(io.StringIO("\n".join(rows)), delimiter=delimiter)
    headers = next(reader)
    gene_column = resolve_column(headers, ["gene_id", "gene", "query", "locus"], "gene")
    term_column = resolve_column(headers, ["term_id", "term", "go_id", "pathway_id", "category_id"], "term")
    name_column = optional_column(headers, ["term_name", "name", "description"])
    namespace_column = optional_column(headers, ["namespace", "category", "source"])
    terms: dict[str, dict] = {}
    for record in reader:
        gene = record[gene_column].strip() if gene_column < len(record) else ""
        term_id = record[term_column].strip() if term_column < len(record) else ""
        if not gene or gene.lower() in MISSING or not term_id or term_id.lower() in MISSING:
            continue
        term = terms.setdefault(
            term_id,
            {"name": optional_value(record, name_column), "namespace": optional_value(record, namespace_column), "genes": set()},
        )
        term["genes"].add(gene)
    if not terms:
        raise ValueError("no associations were found")
    return terms


def hypergeometric_upper_tail(population: int, successes: int, draws: int, observed: int,
                              log_factorials: list[float]) -> float:
    upper = min(successes, draws)
    if observed > upper:
        return 0.0

    def ln_choose(total: int, selected: int) -> float:
        if selected > total:
            return float("-inf")
        return (log_factorials[total] - log_factorials[selected]
                - log_factorials[total - selected])

    log_probability = (ln_choose(successes, observed)
                       + ln_choose(population - successes, draws - observed)
                       - ln_choose(population, draws))
    probability = math.exp(log_probability)
    total = probability
    current = observed
    while current < upper:
        numerator = (successes - current) * (draws - current)
        remaining_failures = population - successes
        sampled_failures = draws - current
        denominator = (current + 1) * (remaining_failures - sampled_failures + 1)
        if denominator <= 0:
            break
        probability *= numerator / denominator
        total += probability
        current += 1
    return min(max(total, 0.0), 1.0)


def adjust_benjamini_hochberg(terms: list[dict]) -> None:
    order = sorted(
        range(len(terms)),
        key=lambda index: (terms[index]["p_value"], terms[index]["term_id"]),
    )
    count = len(order)
    running = 1.0
    for reverse_index in range(count - 1, -1, -1):
        term_index = order[reverse_index]
        rank = reverse_index + 1
        adjusted = min(terms[term_index]["p_value"] * count / rank, 1.0)
        running = min(running, adjusted)
        terms[term_index]["adjusted_p_value"] = running


def run(inputs: dict, parameters: dict) -> dict:
    genes_path = inputs["genes"]
    associations_path = inputs["associations"]
    min_overlap = int(parameters.get("min_overlap", 1))
    max_terms = int(parameters.get("max_terms", 100))
    include_genes = bool(parameters.get("include_genes", False))
    if min_overlap < 1:
        raise ValueError("min_overlap must be >= 1")
    if max_terms < 1:
        raise ValueError("max_terms must be >= 1")

    query = read_gene_set(genes_path)
    terms = read_associations(associations_path)
    background = set().union(*(term["genes"] for term in terms.values()))
    if not background:
        raise ValueError("no associations were found")
    mapped_query = query & background
    if not mapped_query:
        raise ValueError("none of the query identifiers occur in the association universe")

    population = len(background)
    sample = len(mapped_query)
    log_factorials = [0.0] * (population + 1)
    for value in range(1, population + 1):
        log_factorials[value] = log_factorials[value - 1] + math.log(value)

    tested = []
    for term_id in sorted(terms):
        term = terms[term_id]
        overlap = sorted(term["genes"] & mapped_query)
        if len(overlap) < min_overlap:
            continue
        term_count = len(term["genes"])
        overlap_count = len(overlap)
        p_value = hypergeometric_upper_tail(population, term_count, sample, overlap_count, log_factorials)
        fold_enrichment = (overlap_count / sample) / (term_count / population)
        tested.append({
            "term_id": term_id,
            "term_name": term["name"],
            "namespace": term["namespace"],
            "overlap_count": overlap_count,
            "query_gene_count": sample,
            "background_term_count": term_count,
            "background_gene_count": population,
            "fold_enrichment": fold_enrichment,
            "p_value": p_value,
            "adjusted_p_value": 1.0,
            "overlap_genes": overlap if include_genes else [],
        })
    if not tested:
        raise ValueError(f"no terms meet min_overlap {min_overlap}")
    adjust_benjamini_hochberg(tested)
    tested.sort(key=lambda t: (
        t["adjusted_p_value"],
        t["p_value"],
        -t["overlap_count"],
        t["term_id"],
    ))
    tested_term_count = len(tested)
    reported = tested[:max_terms]
    warnings = []
    unmapped = len(query) - len(mapped_query)
    if unmapped > 0:
        warnings.append(f"{unmapped} query identifiers were absent from the association universe")
    return {
        "analysis_type": "custom",
        "query_input_count": len(query),
        "query_mapped_count": sample,
        "query_unmapped_count": unmapped,
        "background_gene_count": population,
        "tested_term_count": tested_term_count,
        "reported_term_count": len(reported),
        "omitted_term_count": tested_term_count - len(reported),
        "terms": reported,
        "warnings": warnings,
    }
