use eframe::egui;
use serde_json::Value;

const GREEN: egui::Color32 = egui::Color32::from_rgb(39, 124, 99);
const BLUE: egui::Color32 = egui::Color32::from_rgb(47, 104, 165);
const AMBER: egui::Color32 = egui::Color32::from_rgb(194, 126, 34);
const RED: egui::Color32 = egui::Color32::from_rgb(177, 67, 63);
const CYAN: egui::Color32 = egui::Color32::from_rgb(42, 143, 157);

#[derive(Clone)]
struct BarValue {
    label: String,
    value: f64,
}

#[derive(Clone)]
struct LineSeries {
    label: String,
    color: egui::Color32,
    points: Vec<(f64, f64)>,
}

#[derive(Clone)]
struct ScatterPoint {
    label: String,
    x: f64,
    y: f64,
}

enum ChartSpec {
    Bars {
        title: String,
        values: Vec<BarValue>,
        percent: bool,
    },
    Lines {
        title: String,
        series: Vec<LineSeries>,
        percent: bool,
    },
    Scatter {
        title: String,
        x_label: String,
        y_label: String,
        points: Vec<ScatterPoint>,
    },
    Heatmap {
        title: String,
        row_labels: Vec<String>,
        column_labels: Vec<String>,
        values: Vec<Vec<f64>>,
        minimum: f64,
        maximum: f64,
    },
}

/// Draws capability-aware previews and returns whether a chart was available.
pub fn show_analysis_charts(
    ui: &mut egui::Ui,
    payload: &Value,
    capability: Option<&str>,
    zh_cn: bool,
) -> bool {
    let charts = chart_specs(payload, capability, zh_cn);
    if charts.is_empty() {
        return false;
    }

    for (index, chart) in charts.iter().enumerate() {
        if index > 0 {
            ui.add_space(12.0);
        }
        match chart {
            ChartSpec::Bars {
                title,
                values,
                percent,
            } => show_bar_chart(ui, title, values, *percent),
            ChartSpec::Lines {
                title,
                series,
                percent,
            } => show_line_chart(ui, title, series, *percent, zh_cn),
            ChartSpec::Scatter {
                title,
                x_label,
                y_label,
                points,
            } => show_scatter_chart(ui, title, x_label, y_label, points),
            ChartSpec::Heatmap {
                title,
                row_labels,
                column_labels,
                values,
                minimum,
                maximum,
            } => show_heatmap(
                ui,
                title,
                row_labels,
                column_labels,
                values,
                *minimum,
                *maximum,
            ),
        }
    }
    true
}

fn chart_specs(payload: &Value, capability: Option<&str>, zh_cn: bool) -> Vec<ChartSpec> {
    match capability.unwrap_or_default() {
        "sequence.stats.v1" => sequence_charts(payload, zh_cn),
        "sequence.kmer.count.v1" => kmer_charts(payload, zh_cn),
        "primer.epcr.v1" => epcr_charts(payload, zh_cn),
        "fastq.qc.v1" => fastq_charts(payload, zh_cn),
        "fastq.trim.v1" | "fastq.adapter.v1" => fastq_transform_charts(payload, zh_cn),
        "alignment.qc.v1" => alignment_charts(payload, zh_cn),
        "annotation.gxf.stats.v1" => annotation_charts(payload, zh_cn),
        "annotation.go.normalize.v1" => go_annotation_map_charts(payload, zh_cn),
        "annotation.eggnog.normalize.v1" => eggnog_annotation_charts(payload, zh_cn),
        "enrichment.overrepresentation.v1" | "enrichment.go.v1" | "enrichment.kegg.v1" => {
            enrichment_charts(payload, zh_cn)
        }
        "genome.gene-density.v1" => gene_density_charts(payload, zh_cn),
        "interval.intersect.v1" => interval_charts(payload, zh_cn),
        "interval.merge.v1" => interval_merge_charts(payload, zh_cn),
        "interval.subtract.v1" => interval_subtract_charts(payload, zh_cn),
        "expression.matrix.qc.v1" => expression_charts(payload, zh_cn),
        "expression.normalize.v1" => expression_normalization_charts(payload, zh_cn),
        "expression.pca.v1" => expression_pca_charts(payload, zh_cn),
        "expression.cluster.v1" => expression_cluster_charts(payload, zh_cn),
        "expression.heatmap.v1" => expression_heatmap_charts(payload, zh_cn),
        "set.venn.v1" | "set.upset.v1" => set_analysis_charts(payload, zh_cn),
        "protein.properties.v1" => protein_properties_charts(payload, zh_cn),
        "similarity.blast.parse.v1" => blast_parse_charts(payload, zh_cn),
        "similarity.reciprocal.v1" => reciprocal_hit_charts(payload, zh_cn),
        "protein.domain.parse.v1" => protein_domain_charts(payload, zh_cn),
        "phylogeny.tree.transform.v1" => phylogeny_tree_charts(payload, zh_cn),
        "table.manipulate.v1" => table_manipulate_charts(payload, zh_cn),
        "variant.stats.v1" => variant_charts(payload, zh_cn),
        "variant.filter.v1" => variant_filter_charts(payload, zh_cn),
        "variant.normalize.v1" => variant_normalize_charts(payload, zh_cn),
        "structure.pdb.summary.v1" | "structure.mmcif.summary.v1" => {
            structure_charts(payload, zh_cn)
        }
        "structure.sequence.extract.v1" => structure_sequence_charts(payload, zh_cn),
        "structure.contact-map.v1" => structure_contact_charts(payload, zh_cn),
        "structure.geometry.v1" => structure_geometry_charts(payload, zh_cn),
        "structure.superpose.v1" => structure_superposition_charts(payload, zh_cn),
        _ if payload.get("per_cycle").is_some() => fastq_charts(payload, zh_cn),
        _ if payload.get("contig_counts").is_some() => variant_charts(payload, zh_cn),
        _ if payload.get("n50").is_some() => sequence_charts(payload, zh_cn),
        _ => Vec::new(),
    }
}

fn go_annotation_map_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    bar_specs(
        [(
            localized(zh_cn, "GO 注释映射摘要", "GO annotation mapping summary"),
            values_for_keys(
                payload,
                &[
                    ("input_row_count", localized(zh_cn, "输入行", "Input rows")),
                    ("gene_count", localized(zh_cn, "基因", "Genes")),
                    ("term_count", localized(zh_cn, "GO 条目", "GO terms")),
                    (
                        "association_count",
                        localized(zh_cn, "关联", "Associations"),
                    ),
                ],
            ),
        )],
        false,
    )
}

fn eggnog_annotation_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    bar_specs(
        [(
            localized(zh_cn, "eggNOG 注释摘要", "eggNOG annotation summary"),
            values_for_keys(
                payload,
                &[
                    ("input_row_count", localized(zh_cn, "输入行", "Input rows")),
                    ("query_count", localized(zh_cn, "查询序列", "Queries")),
                    (
                        "go_association_count",
                        localized(zh_cn, "GO 关联", "GO assignments"),
                    ),
                    (
                        "kegg_association_count",
                        localized(zh_cn, "KEGG 关联", "KEGG assignments"),
                    ),
                ],
            ),
        )],
        false,
    )
}

fn enrichment_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let mut significance = Vec::new();
    let mut fold_enrichment = Vec::new();
    for term in payload
        .get("terms")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let term_id = term
            .get("term_id")
            .and_then(Value::as_str)
            .unwrap_or("term");
        let label = term
            .get("term_name")
            .and_then(Value::as_str)
            .filter(|name| !name.trim().is_empty())
            .map(|name| format!("{name} ({term_id})"))
            .unwrap_or_else(|| term_id.to_owned());
        if let Some(adjusted) = number(term, "adjusted_p_value") {
            significance.push(BarValue {
                label: label.clone(),
                value: negative_log10_probability(adjusted),
            });
        }
        if let Some(fold) = number(term, "fold_enrichment").filter(|value| value.is_finite()) {
            fold_enrichment.push(BarValue { label, value: fold });
        }
    }
    sort_and_limit(&mut significance, 20);
    sort_and_limit(&mut fold_enrichment, 20);
    bar_specs(
        [
            (
                localized(
                    zh_cn,
                    "富集显著性（-log10 校正 P 值）",
                    "Enrichment significance (-log10 adjusted p-value)",
                ),
                significance,
            ),
            (
                localized(zh_cn, "富集倍数", "Fold enrichment"),
                fold_enrichment,
            ),
        ],
        false,
    )
}

fn negative_log10_probability(value: f64) -> f64 {
    const MIN_POSITIVE_PROBABILITY: f64 = 1e-300;
    if !value.is_finite() || value < 0.0 {
        return 0.0;
    }
    -value.clamp(MIN_POSITIVE_PROBABILITY, 1.0).log10()
}

fn annotation_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let mut features = payload
        .get("feature_type_counts")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(name, value)| {
            Some(BarValue {
                label: name.clone(),
                value: value.as_f64()?,
            })
        })
        .collect::<Vec<_>>();
    sort_and_limit(&mut features, 12);

    let mut sequences = payload
        .get("sequence_counts")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(name, value)| {
            Some(BarValue {
                label: name.clone(),
                value: value.as_f64()?,
            })
        })
        .collect::<Vec<_>>();
    sort_and_limit(&mut sequences, 12);

    let mut charts = Vec::new();
    if !features.is_empty() {
        charts.push(ChartSpec::Bars {
            title: localized(zh_cn, "注释特征类型", "Annotation feature types").to_owned(),
            values: features,
            percent: false,
        });
    }
    if !sequences.is_empty() {
        charts.push(ChartSpec::Bars {
            title: localized(zh_cn, "注释最多的序列", "Top annotated sequences").to_owned(),
            values: sequences,
            percent: false,
        });
    }
    charts
}

fn gene_density_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let mut by_sequence = std::collections::BTreeMap::<String, Vec<(f64, f64)>>::new();
    for bin in payload
        .get("bins")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(seqid) = bin.get("seqid").and_then(Value::as_str) else {
            continue;
        };
        let Some(start) = bin.get("start").and_then(Value::as_f64) else {
            continue;
        };
        let Some(end) = bin.get("end").and_then(Value::as_f64) else {
            continue;
        };
        let Some(density) = bin.get("features_per_megabase").and_then(Value::as_f64) else {
            continue;
        };
        by_sequence
            .entry(seqid.to_owned())
            .or_default()
            .push(((start + end) / 2.0, density));
    }
    let series = by_sequence
        .into_iter()
        .take(8)
        .enumerate()
        .map(|(index, (label, points))| LineSeries {
            label,
            color: chart_color(index),
            points,
        })
        .collect::<Vec<_>>();
    if series.is_empty() {
        Vec::new()
    } else {
        vec![ChartSpec::Lines {
            title: localized(zh_cn, "滑动窗口特征密度", "Sliding-window feature density")
                .to_owned(),
            series,
            percent: false,
        }]
    }
}

fn blast_parse_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let summary = values_for_keys(
        payload,
        &[
            ("record_count", localized(zh_cn, "命中", "Hits")),
            ("query_count", localized(zh_cn, "查询", "Queries")),
            ("subject_count", localized(zh_cn, "目标", "Subjects")),
        ],
    );
    let scores = payload
        .get("hits")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|hit| {
            Some(BarValue {
                label: format!(
                    "{} → {}",
                    hit.get("query_id")?.as_str()?,
                    hit.get("subject_id")?.as_str()?
                ),
                value: hit.get("bit_score")?.as_f64()?,
            })
        })
        .take(20)
        .collect::<Vec<_>>();
    bar_specs(
        [
            (
                localized(zh_cn, "相似性结果摘要", "Similarity result summary"),
                summary,
            ),
            (
                localized(zh_cn, "前 20 个 bit score", "First 20 bit scores"),
                scores,
            ),
        ],
        false,
    )
}

fn reciprocal_hit_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let values = payload
        .get("pairs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|pair| {
            Some(BarValue {
                label: format!(
                    "{} ↔ {}",
                    pair.get("left_id")?.as_str()?,
                    pair.get("right_id")?.as_str()?
                ),
                value: pair
                    .get("forward_identity_percent")?
                    .as_f64()?
                    .min(pair.get("reverse_identity_percent")?.as_f64()?),
            })
        })
        .take(30)
        .collect::<Vec<_>>();
    if values.is_empty() {
        Vec::new()
    } else {
        vec![ChartSpec::Bars {
            title: localized(
                zh_cn,
                "双向配对最低相似度",
                "Minimum identity of reciprocal pairs",
            )
            .to_owned(),
            values,
            percent: true,
        }]
    }
}

fn protein_domain_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let mut sources = payload
        .get("source_counts")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(source, count)| {
            Some(BarValue {
                label: source.clone(),
                value: count.as_f64()?,
            })
        })
        .collect::<Vec<_>>();
    sort_and_limit(&mut sources, 20);
    if sources.is_empty() {
        Vec::new()
    } else {
        vec![ChartSpec::Bars {
            title: localized(zh_cn, "结构域来源", "Domain sources").to_owned(),
            values: sources,
            percent: false,
        }]
    }
}

fn phylogeny_tree_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let values = values_for_keys(
        payload,
        &[
            ("leaf_count", localized(zh_cn, "叶节点", "Leaves")),
            (
                "internal_node_count",
                localized(zh_cn, "内部节点", "Internal nodes"),
            ),
            ("max_depth", localized(zh_cn, "最大深度", "Maximum depth")),
            (
                "relabeled_count",
                localized(zh_cn, "重命名节点", "Relabeled nodes"),
            ),
        ],
    );
    if values.is_empty() {
        Vec::new()
    } else {
        vec![ChartSpec::Bars {
            title: localized(zh_cn, "系统发育树摘要", "Phylogeny tree summary").to_owned(),
            values,
            percent: false,
        }]
    }
}

fn fastq_transform_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let reads = values_for_keys(
        payload,
        &[
            (
                "input_read_count",
                localized(zh_cn, "输入读段", "Input reads"),
            ),
            (
                "output_read_count",
                localized(zh_cn, "输出读段", "Output reads"),
            ),
            (
                "discarded_read_count",
                localized(zh_cn, "丢弃读段", "Discarded reads"),
            ),
            (
                "trimmed_read_count",
                localized(zh_cn, "被裁剪读段", "Trimmed reads"),
            ),
        ],
    );
    let bases = values_for_keys(
        payload,
        &[
            ("input_bases", localized(zh_cn, "输入碱基", "Input bases")),
            ("output_bases", localized(zh_cn, "输出碱基", "Output bases")),
            (
                "quality_trimmed_bases",
                localized(zh_cn, "质量裁剪碱基", "Quality-trimmed bases"),
            ),
            (
                "adapter_trimmed_bases",
                localized(zh_cn, "接头裁剪碱基", "Adapter-trimmed bases"),
            ),
        ],
    );
    bar_specs(
        [
            (
                localized(zh_cn, "FASTQ 读段处理摘要", "FASTQ read processing summary"),
                reads,
            ),
            (
                localized(zh_cn, "输入/输出/裁剪碱基", "Input/output/trimmed bases"),
                bases,
            ),
        ],
        false,
    )
}

fn alignment_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let mapping = values_for_keys(
        payload,
        &[
            ("mapped_record_count", localized(zh_cn, "已比对", "Mapped")),
            (
                "unmapped_record_count",
                localized(zh_cn, "未比对", "Unmapped"),
            ),
            (
                "secondary_record_count",
                localized(zh_cn, "次要", "Secondary"),
            ),
            (
                "supplementary_record_count",
                localized(zh_cn, "补充", "Supplementary"),
            ),
        ],
    );
    let flags = values_for_keys(
        payload,
        &[
            (
                "duplicate_record_count",
                localized(zh_cn, "重复", "Duplicate"),
            ),
            (
                "qc_fail_record_count",
                localized(zh_cn, "QC 失败", "QC fail"),
            ),
            ("zero_mapq_record_count", "MAPQ 0"),
            (
                "proper_pair_record_count",
                localized(zh_cn, "正确配对", "Proper pair"),
            ),
        ],
    );
    bar_specs(
        [
            (localized(zh_cn, "比对记录", "Alignment records"), mapping),
            (localized(zh_cn, "FLAG 指标", "FLAG metrics"), flags),
        ],
        false,
    )
}

fn interval_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let summary = values_for_keys(
        payload,
        &[
            ("left_interval_count", localized(zh_cn, "左侧", "Left")),
            ("right_interval_count", localized(zh_cn, "右侧", "Right")),
            (
                "overlap_pair_count",
                localized(zh_cn, "重叠对", "Overlap pairs"),
            ),
            (
                "left_overlapped_count",
                localized(zh_cn, "左侧已重叠", "Left overlapped"),
            ),
            (
                "right_overlapped_count",
                localized(zh_cn, "右侧已重叠", "Right overlapped"),
            ),
        ],
    );
    let mut contigs = payload
        .get("contigs")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(name, metrics)| {
            Some(BarValue {
                label: name.clone(),
                value: number(metrics, "total_overlap_bases")?,
            })
        })
        .collect::<Vec<_>>();
    sort_and_limit(&mut contigs, 12);
    bar_specs(
        [
            (
                localized(zh_cn, "区间相交摘要", "Interval overlap summary"),
                summary,
            ),
            (
                localized(zh_cn, "各区域重叠碱基", "Overlap bases by contig"),
                contigs,
            ),
        ],
        false,
    )
}

fn interval_merge_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let summary = values_for_keys(
        payload,
        &[
            (
                "input_interval_count",
                localized(zh_cn, "输入区间", "Input intervals"),
            ),
            (
                "output_interval_count",
                localized(zh_cn, "输出区间", "Output intervals"),
            ),
            (
                "merged_interval_count",
                localized(zh_cn, "被合并", "Merged"),
            ),
        ],
    );
    let bases = values_for_keys(
        payload,
        &[
            ("input_bases", localized(zh_cn, "输入碱基", "Input bases")),
            ("output_bases", localized(zh_cn, "输出碱基", "Output bases")),
        ],
    );
    let mut contigs = payload
        .get("contigs")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(name, metrics)| {
            Some(BarValue {
                label: name.clone(),
                value: number(metrics, "output_bases")?,
            })
        })
        .collect::<Vec<_>>();
    sort_and_limit(&mut contigs, 12);
    bar_specs(
        [
            (
                localized(zh_cn, "区间合并摘要", "Interval merge summary"),
                summary,
            ),
            (
                localized(zh_cn, "输入/输出碱基", "Input/output bases"),
                bases,
            ),
            (
                localized(zh_cn, "各区域输出碱基", "Output bases by contig"),
                contigs,
            ),
        ],
        false,
    )
}

fn interval_subtract_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let summary = values_for_keys(
        payload,
        &[
            (
                "left_interval_count",
                localized(zh_cn, "左侧区间", "Left intervals"),
            ),
            (
                "right_interval_count",
                localized(zh_cn, "右侧区间", "Right intervals"),
            ),
            (
                "output_interval_count",
                localized(zh_cn, "输出区间", "Output intervals"),
            ),
            (
                "affected_left_interval_count",
                localized(zh_cn, "受影响左侧", "Affected left"),
            ),
        ],
    );
    let bases = values_for_keys(
        payload,
        &[
            (
                "removed_bases",
                localized(zh_cn, "扣除碱基", "Removed bases"),
            ),
            ("output_bases", localized(zh_cn, "输出碱基", "Output bases")),
        ],
    );
    let mut contigs = payload
        .get("contigs")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(name, metrics)| {
            Some(BarValue {
                label: name.clone(),
                value: number(metrics, "output_bases")?,
            })
        })
        .collect::<Vec<_>>();
    sort_and_limit(&mut contigs, 12);
    bar_specs(
        [
            (
                localized(zh_cn, "区间扣除摘要", "Interval subtraction summary"),
                summary,
            ),
            (
                localized(zh_cn, "扣除/输出碱基", "Removed/output bases"),
                bases,
            ),
            (
                localized(zh_cn, "各区域输出碱基", "Output bases by contig"),
                contigs,
            ),
        ],
        false,
    )
}

fn expression_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let cells = values_for_keys(
        payload,
        &[
            ("numeric_value_count", localized(zh_cn, "数值", "Numeric")),
            ("zero_value_count", localized(zh_cn, "零值", "Zero")),
            ("missing_value_count", localized(zh_cn, "缺失", "Missing")),
            ("negative_value_count", localized(zh_cn, "负值", "Negative")),
        ],
    );
    let mut totals = payload
        .get("samples")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|sample| {
            Some(BarValue {
                label: sample.get("sample")?.as_str()?.to_owned(),
                value: number(sample, "total")?,
            })
        })
        .collect::<Vec<_>>();
    let signed_totals = totals.iter().any(|entry| entry.value < 0.0);
    if signed_totals {
        sort_and_limit_by_magnitude(&mut totals, 20);
    } else {
        sort_and_limit(&mut totals, 20);
    }
    bar_specs(
        [
            (localized(zh_cn, "矩阵单元格", "Matrix cells"), cells),
            (
                if signed_totals {
                    localized(zh_cn, "样本有符号总和", "Signed sample totals")
                } else {
                    localized(zh_cn, "样本总量", "Sample totals")
                },
                totals,
            ),
        ],
        false,
    )
}

fn expression_normalization_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let mut input_totals = Vec::new();
    let mut output_totals = Vec::new();
    let mut scale_factors = Vec::new();
    for sample in payload
        .get("samples")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(label) = sample.get("sample").and_then(Value::as_str) else {
            continue;
        };
        if let Some(value) = number(sample, "input_total") {
            input_totals.push(BarValue {
                label: label.to_owned(),
                value,
            });
        }
        if let Some(value) = number(sample, "output_total") {
            output_totals.push(BarValue {
                label: label.to_owned(),
                value,
            });
        }
        if let Some(value) = number(sample, "scale_factor") {
            scale_factors.push(BarValue {
                label: label.to_owned(),
                value,
            });
        }
    }
    sort_and_limit(&mut input_totals, 20);
    sort_and_limit(&mut output_totals, 20);
    sort_and_limit(&mut scale_factors, 20);
    bar_specs(
        [
            (
                localized(zh_cn, "标准化前样本总量", "Input sample totals"),
                input_totals,
            ),
            (
                localized(zh_cn, "标准化后样本总量", "Output sample totals"),
                output_totals,
            ),
            (
                localized(zh_cn, "样本缩放因子", "Sample scale factors"),
                scale_factors,
            ),
        ],
        false,
    )
}

fn expression_pca_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let mut charts = Vec::new();
    let variance = payload
        .get("components")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|component| {
            Some(BarValue {
                label: format!("PC{}", component.get("component")?.as_u64()?),
                value: number(component, "explained_variance_percent")?,
            })
        })
        .collect::<Vec<_>>();
    if !variance.is_empty() {
        charts.push(ChartSpec::Bars {
            title: localized(zh_cn, "主成分解释方差", "Explained variance by component").to_owned(),
            values: variance,
            percent: true,
        });
    }
    let points = payload
        .get("samples")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|sample| {
            let scores = sample.get("scores")?.as_array()?;
            Some(ScatterPoint {
                label: sample.get("sample")?.as_str()?.to_owned(),
                x: scores.first()?.as_f64()?,
                y: scores.get(1)?.as_f64()?,
            })
        })
        .collect::<Vec<_>>();
    if !points.is_empty() {
        charts.push(ChartSpec::Scatter {
            title: localized(zh_cn, "样本 PCA", "Sample PCA").to_owned(),
            x_label: "PC1".to_owned(),
            y_label: "PC2".to_owned(),
            points,
        });
    }
    charts
}

fn expression_cluster_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let cluster_sizes = |axis: &str| {
        payload
            .get(axis)
            .and_then(|axis| axis.get("cluster_sizes"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
            .filter_map(|(index, value)| {
                Some(BarValue {
                    label: format!("C{}", index + 1),
                    value: value.as_f64()?,
                })
            })
            .collect::<Vec<_>>()
    };
    bar_specs(
        [
            (
                localized(zh_cn, "样本簇大小", "Sample cluster sizes"),
                cluster_sizes("samples"),
            ),
            (
                localized(zh_cn, "特征簇大小", "Feature cluster sizes"),
                cluster_sizes("features"),
            ),
        ],
        false,
    )
}

fn expression_heatmap_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let row_labels = payload
        .get("row_labels")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    let column_labels = payload
        .get("column_labels")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    let values = payload
        .get("values")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|row| {
            row.as_array()?
                .iter()
                .map(Value::as_f64)
                .collect::<Option<Vec<_>>>()
        })
        .collect::<Vec<_>>();
    let minimum = number(payload, "minimum_value").unwrap_or(-1.0);
    let maximum = number(payload, "maximum_value").unwrap_or(1.0);
    if row_labels.is_empty()
        || column_labels.is_empty()
        || values.len() != row_labels.len()
        || values.iter().any(|row| row.len() != column_labels.len())
    {
        return Vec::new();
    }
    vec![ChartSpec::Heatmap {
        title: localized(zh_cn, "聚类表达热图", "Clustered expression heatmap").to_owned(),
        row_labels,
        column_labels,
        values,
        minimum,
        maximum,
    }]
}

fn table_manipulate_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let rows = values_for_keys(
        payload,
        &[
            ("input_rows", localized(zh_cn, "输入行", "Input rows")),
            ("skipped_rows", localized(zh_cn, "跳过行", "Skipped rows")),
            ("filtered_rows", localized(zh_cn, "过滤行", "Filtered rows")),
            ("output_rows", localized(zh_cn, "输出行", "Output rows")),
        ],
    );
    let columns = values_for_keys(
        payload,
        &[
            ("input_columns", localized(zh_cn, "输入列", "Input columns")),
            (
                "output_columns",
                localized(zh_cn, "输出列", "Output columns"),
            ),
        ],
    );
    bar_specs(
        [
            (localized(zh_cn, "表格行处理", "Table row handling"), rows),
            (
                localized(zh_cn, "表格列处理", "Table column handling"),
                columns,
            ),
        ],
        false,
    )
}

fn set_analysis_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let set_sizes = payload
        .get("set_sizes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            Some(BarValue {
                label: entry.get("name")?.as_str()?.to_owned(),
                value: entry.get("count")?.as_f64()?,
            })
        })
        .take(64)
        .collect::<Vec<_>>();
    let intersections = payload
        .get("intersections")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let label = entry
                .get("sets")?
                .as_array()?
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" ∩ ");
            Some(BarValue {
                label,
                value: entry.get("count")?.as_f64()?,
            })
        })
        .take(24)
        .collect::<Vec<_>>();
    bar_specs(
        [
            (localized(zh_cn, "集合大小", "Set sizes"), set_sizes),
            (
                localized(zh_cn, "精确交集大小", "Exact intersection sizes"),
                intersections,
            ),
        ],
        false,
    )
}

fn protein_properties_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let records = payload
        .get("records")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(24)
        .collect::<Vec<_>>();
    let lengths = records
        .iter()
        .filter_map(|entry| {
            Some(BarValue {
                label: entry.get("id")?.as_str()?.to_owned(),
                value: entry.get("length")?.as_f64()?,
            })
        })
        .collect::<Vec<_>>();
    let molecular_weights = records
        .iter()
        .filter_map(|entry| {
            Some(BarValue {
                label: entry.get("id")?.as_str()?.to_owned(),
                value: entry.get("molecular_weight_da")?.as_f64()?,
            })
        })
        .collect::<Vec<_>>();
    let isoelectric_points = records
        .iter()
        .filter_map(|entry| {
            Some(BarValue {
                label: entry.get("id")?.as_str()?.to_owned(),
                value: entry.get("isoelectric_point")?.as_f64()?,
            })
        })
        .collect::<Vec<_>>();
    bar_specs(
        [
            (localized(zh_cn, "蛋白长度", "Protein lengths"), lengths),
            (
                localized(zh_cn, "分子量（Da）", "Molecular weight (Da)"),
                molecular_weights,
            ),
            (
                localized(zh_cn, "等电点", "Isoelectric point"),
                isoelectric_points,
            ),
        ],
        false,
    )
}

fn structure_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let composition = values_for_keys(
        payload,
        &[
            ("chain_count", localized(zh_cn, "链", "Chains")),
            ("residue_count", localized(zh_cn, "残基", "Residues")),
            (
                "polymer_atom_count",
                localized(zh_cn, "聚合物原子", "Polymer atoms"),
            ),
            (
                "hetero_atom_count",
                localized(zh_cn, "异质原子", "Hetero atoms"),
            ),
        ],
    );
    let bands = payload
        .get("alphafold_confidence")
        .and_then(|confidence| confidence.get("bands"))
        .map(|bands| {
            values_for_keys(
                bands,
                &[
                    ("very_high_count", localized(zh_cn, "很高", "Very high")),
                    ("confident_count", localized(zh_cn, "可信", "Confident")),
                    ("low_count", localized(zh_cn, "较低", "Low")),
                    ("very_low_count", localized(zh_cn, "很低", "Very low")),
                ],
            )
        })
        .unwrap_or_default();
    bar_specs(
        [
            (
                localized(zh_cn, "结构组成", "Structure composition"),
                composition,
            ),
            (localized(zh_cn, "pLDDT 分段", "pLDDT bands"), bands),
        ],
        false,
    )
}

fn structure_sequence_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let chains = payload
        .get("chains")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|chain| {
            Some(BarValue {
                label: chain.get("chain_id")?.as_str()?.to_owned(),
                value: number(chain, "residue_count")?,
            })
        })
        .collect::<Vec<_>>();
    bar_specs(
        [(
            localized(zh_cn, "各链坐标残基数", "Coordinate residues by chain"),
            chains,
        )],
        false,
    )
}

fn structure_contact_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let mut distances = payload
        .get("contacts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|contact| {
            let left = contact.get("left")?;
            let right = contact.get("right")?;
            Some(BarValue {
                label: format!(
                    "{}:{}–{}:{}",
                    left.get("chain_id")?.as_str()?,
                    left.get("residue_id")?.as_str()?,
                    right.get("chain_id")?.as_str()?,
                    right.get("residue_id")?.as_str()?
                ),
                value: number(contact, "distance_angstrom")?,
            })
        })
        .collect::<Vec<_>>();
    distances.sort_by(|left, right| {
        left.value
            .total_cmp(&right.value)
            .then_with(|| left.label.cmp(&right.label))
    });
    distances.truncate(40);
    bar_specs(
        [(
            localized(
                zh_cn,
                "最近残基接触距离（埃）",
                "Nearest residue contacts (angstrom)",
            ),
            distances,
        )],
        false,
    )
}

fn structure_geometry_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let Some(value) = number(payload, "value") else {
        return Vec::new();
    };
    let label = payload
        .get("measurement")
        .and_then(Value::as_str)
        .unwrap_or("measurement");
    bar_specs(
        [(
            localized(zh_cn, "结构几何测量", "Structure geometry measurement"),
            vec![BarValue {
                label: label.to_owned(),
                value,
            }],
        )],
        false,
    )
}

fn structure_superposition_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let rmsd = values_for_keys(
        payload,
        &[
            (
                "rmsd_before_angstrom",
                localized(zh_cn, "拟合前 RMSD", "RMSD before"),
            ),
            (
                "rmsd_after_angstrom",
                localized(zh_cn, "拟合后 RMSD", "RMSD after"),
            ),
        ],
    );
    bar_specs(
        [(
            localized(
                zh_cn,
                "结构叠合 RMSD（埃）",
                "Structure superposition RMSD (angstrom)",
            ),
            rmsd,
        )],
        false,
    )
}

fn values_for_keys(payload: &Value, definitions: &[(&str, &str)]) -> Vec<BarValue> {
    definitions
        .iter()
        .filter_map(|(key, label)| {
            Some(BarValue {
                label: (*label).to_owned(),
                value: number(payload, key)?,
            })
        })
        .collect()
}

fn bar_specs<const N: usize>(groups: [(&str, Vec<BarValue>); N], percent: bool) -> Vec<ChartSpec> {
    groups
        .into_iter()
        .filter(|(_, values)| !values.is_empty())
        .map(|(title, values)| ChartSpec::Bars {
            title: title.to_owned(),
            values,
            percent,
        })
        .collect()
}

fn sort_and_limit(values: &mut Vec<BarValue>, limit: usize) {
    values.sort_by(|left, right| {
        right
            .value
            .total_cmp(&left.value)
            .then_with(|| left.label.cmp(&right.label))
    });
    values.truncate(limit);
}

fn sort_and_limit_by_magnitude(values: &mut Vec<BarValue>, limit: usize) {
    values.sort_by(|left, right| {
        right
            .value
            .abs()
            .total_cmp(&left.value.abs())
            .then_with(|| left.label.cmp(&right.label))
    });
    values.truncate(limit);
}

fn sequence_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let mut lengths = Vec::new();
    for (key, zh, en) in [
        ("min_length", "最短", "Minimum"),
        ("mean_length", "平均", "Mean"),
        ("n50", "N50", "N50"),
        ("max_length", "最长", "Maximum"),
    ] {
        if let Some(value) = number(payload, key) {
            lengths.push(BarValue {
                label: localized(zh_cn, zh, en).to_owned(),
                value,
            });
        }
    }

    let gc = number(payload, "gc_percent")
        .unwrap_or(0.0)
        .clamp(0.0, 100.0);
    let ambiguous = number(payload, "n_percent")
        .unwrap_or(0.0)
        .clamp(0.0, 100.0);
    let mut charts = Vec::new();
    if !lengths.is_empty() {
        charts.push(ChartSpec::Bars {
            title: localized(zh_cn, "序列长度概览", "Sequence length overview").to_owned(),
            values: lengths,
            percent: false,
        });
    }
    charts.push(ChartSpec::Bars {
        title: localized(zh_cn, "碱基比例指标", "Base percentage metrics").to_owned(),
        values: vec![
            BarValue {
                label: localized(zh_cn, "GC（标准碱基）", "GC (canonical bases)").to_owned(),
                value: gc,
            },
            BarValue {
                label: localized(zh_cn, "N（全部碱基）", "N (all bases)").to_owned(),
                value: ambiguous,
            },
        ],
        percent: true,
    });
    charts
}

fn fastq_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let Some(cycles) = payload.get("per_cycle").and_then(Value::as_array) else {
        return Vec::new();
    };
    if cycles.is_empty() {
        return Vec::new();
    }

    let definitions = [
        (
            "mean_quality",
            localized(zh_cn, "平均质量", "Mean quality"),
            GREEN,
        ),
        ("q20_percent", "Q20 %", BLUE),
        ("q30_percent", "Q30 %", AMBER),
    ];
    let mut series = Vec::new();
    for (key, label, color) in definitions {
        let points = cycles
            .iter()
            .filter_map(|cycle| {
                Some((
                    number(cycle, "cycle")?,
                    cycle.get(key).and_then(Value::as_f64)?,
                ))
            })
            .collect::<Vec<_>>();
        if !points.is_empty() {
            series.push(LineSeries {
                label: label.to_owned(),
                color,
                points,
            });
        }
    }

    let composition = [
        ("gc_percent", "GC %"),
        ("n_percent", "N %"),
        ("q20_percent", "Q20 %"),
        ("q30_percent", "Q30 %"),
    ]
    .into_iter()
    .filter_map(|(key, label)| {
        Some(BarValue {
            label: label.to_owned(),
            value: number(payload, key)?,
        })
    })
    .collect::<Vec<_>>();

    let mut charts = Vec::new();
    if !series.is_empty() {
        charts.push(ChartSpec::Lines {
            title: localized(zh_cn, "逐循环测序质量", "Per-cycle sequencing quality").to_owned(),
            series,
            percent: true,
        });
    }
    if !composition.is_empty() {
        charts.push(ChartSpec::Bars {
            title: localized(zh_cn, "整体质量指标", "Overall quality metrics").to_owned(),
            values: composition,
            percent: true,
        });
    }
    charts
}

fn variant_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let variants = [
        ("snp_count", "SNP"),
        ("indel_count", "Indel"),
        ("mnv_count", "MNV"),
        ("symbolic_count", localized(zh_cn, "符号", "Symbolic")),
    ]
    .into_iter()
    .filter_map(|(key, label)| {
        Some(BarValue {
            label: label.to_owned(),
            value: number(payload, key)?,
        })
    })
    .collect::<Vec<_>>();

    let mut contigs = payload
        .get("contig_counts")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(name, value)| {
            Some(BarValue {
                label: name.clone(),
                value: value.as_f64()?,
            })
        })
        .collect::<Vec<_>>();
    sort_and_limit(&mut contigs, 12);

    let mut charts = Vec::new();
    if !variants.is_empty() {
        charts.push(ChartSpec::Bars {
            title: localized(zh_cn, "变异类型", "Variant classes").to_owned(),
            values: variants,
            percent: false,
        });
    }
    if !contigs.is_empty() {
        charts.push(ChartSpec::Bars {
            title: localized(zh_cn, "变异最多的序列区域", "Top contigs by variants").to_owned(),
            values: contigs,
            percent: false,
        });
    }
    charts
}

fn kmer_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let values = payload
        .get("top_kmers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            Some(BarValue {
                label: entry.get("kmer")?.as_str()?.to_owned(),
                value: entry.get("count")?.as_f64()?,
            })
        })
        .take(20)
        .collect::<Vec<_>>();
    if values.is_empty() {
        Vec::new()
    } else {
        vec![ChartSpec::Bars {
            title: localized(zh_cn, "高频 k-mer", "Top k-mers").to_owned(),
            values,
            percent: false,
        }]
    }
}

fn epcr_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let values = values_for_keys(
        payload,
        &[
            (
                "primer_pair_count",
                localized(zh_cn, "输入引物对", "Primer pairs"),
            ),
            (
                "matched_primer_pair_count",
                localized(zh_cn, "命中引物对", "Matched pairs"),
            ),
            ("amplicon_count", localized(zh_cn, "扩增子", "Amplicons")),
        ],
    );
    if values.is_empty() {
        Vec::new()
    } else {
        vec![ChartSpec::Bars {
            title: localized(zh_cn, "电子 PCR 命中", "Electronic PCR hits").to_owned(),
            values,
            percent: false,
        }]
    }
}

fn variant_filter_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let values = values_for_keys(
        payload,
        &[
            ("output_records", localized(zh_cn, "保留", "Retained")),
            (
                "rejected_by_qual",
                localized(zh_cn, "QUAL 淘汰", "QUAL rejected"),
            ),
            (
                "rejected_by_filter",
                localized(zh_cn, "FILTER 淘汰", "FILTER rejected"),
            ),
            (
                "rejected_by_contig",
                localized(zh_cn, "染色体淘汰", "Contig rejected"),
            ),
            (
                "rejected_by_info_dp",
                localized(zh_cn, "DP 淘汰", "DP rejected"),
            ),
        ],
    );
    if values.is_empty() {
        Vec::new()
    } else {
        vec![ChartSpec::Bars {
            title: localized(zh_cn, "VCF 过滤结果", "VCF filter outcome").to_owned(),
            values,
            percent: false,
        }]
    }
}

fn variant_normalize_charts(payload: &Value, zh_cn: bool) -> Vec<ChartSpec> {
    let values = values_for_keys(
        payload,
        &[
            (
                "reference_validated_records",
                localized(zh_cn, "参考验证", "Reference validated"),
            ),
            (
                "changed_records",
                localized(zh_cn, "表示已改变", "Representation changed"),
            ),
            (
                "left_aligned_records",
                localized(zh_cn, "已左对齐", "Left aligned"),
            ),
        ],
    );
    if values.is_empty() {
        Vec::new()
    } else {
        vec![ChartSpec::Bars {
            title: localized(zh_cn, "VCF 规范化", "VCF normalization").to_owned(),
            values,
            percent: false,
        }]
    }
}

fn show_bar_chart(ui: &mut egui::Ui, title: &str, values: &[BarValue], percent: bool) {
    ui.label(egui::RichText::new(title).strong().size(14.0));
    let height = (values.len() as f32 * 27.0 + 18.0).clamp(94.0, 350.0);
    let width = ui.available_width().max(320.0);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let label_width = (rect.width() * 0.24).clamp(82.0, 150.0);
    let value_width = 72.0;
    let plot_left = rect.left() + label_width;
    let plot_right = (rect.right() - value_width).max(plot_left + 20.0);
    let (min_value, max_value) = bar_domain(values, percent);
    let value_span = (max_value - min_value).max(f64::EPSILON);
    let zero_fraction = ((0.0 - min_value) / value_span).clamp(0.0, 1.0) as f32;
    let zero_x = plot_left + (plot_right - plot_left) * zero_fraction;
    let row_height = (height - 10.0) / values.len().max(1) as f32;
    let text_color = ui.visuals().text_color();
    let faint = egui::Color32::from_rgb(229, 234, 232);

    for (index, entry) in values.iter().enumerate() {
        let center_y = rect.top() + 5.0 + row_height * (index as f32 + 0.5);
        painter.text(
            egui::pos2(rect.left(), center_y),
            egui::Align2::LEFT_CENTER,
            &entry.label,
            egui::FontId::proportional(12.0),
            text_color,
        );
        let track = egui::Rect::from_center_size(
            egui::pos2((plot_left + plot_right) * 0.5, center_y),
            egui::vec2(plot_right - plot_left, 10.0),
        );
        painter.rect_filled(track, 2.0, faint);
        let value = entry.value.clamp(min_value, max_value);
        let fraction = ((value - min_value) / value_span).clamp(0.0, 1.0) as f32;
        let value_x = plot_left + (plot_right - plot_left) * fraction;
        let fill = egui::Rect::from_min_max(
            egui::pos2(zero_x.min(value_x), track.top()),
            egui::pos2(zero_x.max(value_x), track.bottom()),
        );
        painter.rect_filled(
            fill,
            2.0,
            if entry.value < 0.0 {
                RED
            } else {
                chart_color(index)
            },
        );
        painter.text(
            egui::pos2(rect.right(), center_y),
            egui::Align2::RIGHT_CENTER,
            format_value(entry.value, percent),
            egui::FontId::monospace(11.0),
            text_color,
        );
    }
    if min_value < 0.0 {
        painter.line_segment(
            [
                egui::pos2(zero_x, rect.top() + 2.0),
                egui::pos2(zero_x, rect.bottom() - 2.0),
            ],
            egui::Stroke::new(1.0, egui::Color32::from_gray(120)),
        );
    }
    response.on_hover_cursor(egui::CursorIcon::Crosshair);
}

fn bar_domain(values: &[BarValue], percent: bool) -> (f64, f64) {
    if percent {
        return (0.0, 100.0);
    }
    let minimum = values
        .iter()
        .map(|entry| entry.value)
        .fold(0.0_f64, f64::min);
    let maximum = values
        .iter()
        .map(|entry| entry.value)
        .fold(0.0_f64, f64::max);
    if minimum == maximum {
        (minimum.min(0.0), maximum.max(1.0))
    } else {
        (minimum, maximum)
    }
}

fn show_line_chart(
    ui: &mut egui::Ui,
    title: &str,
    series: &[LineSeries],
    percent: bool,
    zh_cn: bool,
) {
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new(title).strong().size(14.0));
        for item in series {
            ui.colored_label(item.color, format!("● {}", item.label));
        }
    });
    let width = ui.available_width().max(360.0);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 245.0), egui::Sense::hover());
    let plot = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 46.0, rect.top() + 10.0),
        egui::pos2(rect.right() - 14.0, rect.bottom() - 28.0),
    );
    let painter = ui.painter_at(rect);
    let x_min = series
        .iter()
        .flat_map(|item| item.points.iter().map(|point| point.0))
        .fold(f64::INFINITY, f64::min);
    let x_max = series
        .iter()
        .flat_map(|item| item.points.iter().map(|point| point.0))
        .fold(f64::NEG_INFINITY, f64::max);
    let data_y_max = series
        .iter()
        .flat_map(|item| item.points.iter().map(|point| point.1))
        .fold(0.0_f64, f64::max);
    let y_max = if percent {
        100.0
    } else {
        ((data_y_max / 10.0).ceil() * 10.0).max(10.0)
    };
    let grid_color = egui::Color32::from_rgb(216, 223, 220);
    let text_color = ui.visuals().weak_text_color();

    for step in 0..=4 {
        let fraction = step as f32 / 4.0;
        let y = plot.bottom() - plot.height() * fraction;
        painter.line_segment(
            [egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)],
            egui::Stroke::new(1.0, grid_color),
        );
        painter.text(
            egui::pos2(plot.left() - 6.0, y),
            egui::Align2::RIGHT_CENTER,
            format!("{:.0}", y_max * f64::from(fraction)),
            egui::FontId::monospace(10.0),
            text_color,
        );
    }
    painter.text(
        egui::pos2(plot.left(), plot.bottom() + 8.0),
        egui::Align2::LEFT_TOP,
        format!("{x_min:.0}"),
        egui::FontId::monospace(10.0),
        text_color,
    );
    painter.text(
        egui::pos2(plot.right(), plot.bottom() + 8.0),
        egui::Align2::RIGHT_TOP,
        format!("{x_max:.0}"),
        egui::FontId::monospace(10.0),
        text_color,
    );
    painter.text(
        egui::pos2(plot.center().x, rect.bottom()),
        egui::Align2::CENTER_BOTTOM,
        localized(zh_cn, "循环", "Cycle"),
        egui::FontId::proportional(11.0),
        text_color,
    );

    let x_span = (x_max - x_min).max(1.0);
    for item in series {
        let points = item
            .points
            .iter()
            .map(|&(x, y)| {
                let screen_x = if (x_max - x_min).abs() < f64::EPSILON {
                    plot.center().x
                } else {
                    plot.left() + plot.width() * ((x - x_min) / x_span) as f32
                };
                egui::pos2(
                    screen_x,
                    plot.bottom() - plot.height() * (y / y_max).clamp(0.0, 1.0) as f32,
                )
            })
            .collect::<Vec<_>>();
        if let Some(shape) = line_series_shape(points, item.color) {
            painter.add(shape);
        }
    }

    if let Some(pointer) = response
        .hover_pos()
        .filter(|position| plot.contains(*position))
    {
        painter.line_segment(
            [
                egui::pos2(pointer.x, plot.top()),
                egui::pos2(pointer.x, plot.bottom()),
            ],
            egui::Stroke::new(1.0, egui::Color32::from_gray(130)),
        );
        let cycle = x_min + f64::from((pointer.x - plot.left()) / plot.width()) * x_span;
        let label = format!("{} {:.0}", localized(zh_cn, "循环", "Cycle"), cycle);
        painter.text(
            egui::pos2(pointer.x + 5.0, plot.top() + 5.0),
            egui::Align2::LEFT_TOP,
            label,
            egui::FontId::monospace(10.0),
            ui.visuals().text_color(),
        );
    }
    response.on_hover_cursor(egui::CursorIcon::Crosshair);
}

fn line_series_shape(points: Vec<egui::Pos2>, color: egui::Color32) -> Option<egui::Shape> {
    match points.as_slice() {
        [] => None,
        [point] => Some(egui::Shape::circle_filled(*point, 3.5, color)),
        _ => Some(egui::Shape::line(points, egui::Stroke::new(1.8, color))),
    }
}

fn show_scatter_chart(
    ui: &mut egui::Ui,
    title: &str,
    x_label: &str,
    y_label: &str,
    points: &[ScatterPoint],
) {
    ui.label(egui::RichText::new(title).strong());
    if points.is_empty() {
        return;
    }
    let desired = egui::vec2(ui.available_width().min(760.0), 340.0);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let plot = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 52.0, rect.top() + 16.0),
        egui::pos2(rect.right() - 16.0, rect.bottom() - 38.0),
    );
    let painter = ui.painter_at(rect);
    let x_min = points
        .iter()
        .map(|point| point.x)
        .min_by(f64::total_cmp)
        .unwrap_or(0.0);
    let x_max = points
        .iter()
        .map(|point| point.x)
        .max_by(f64::total_cmp)
        .unwrap_or(1.0);
    let y_min = points
        .iter()
        .map(|point| point.y)
        .min_by(f64::total_cmp)
        .unwrap_or(0.0);
    let y_max = points
        .iter()
        .map(|point| point.y)
        .max_by(f64::total_cmp)
        .unwrap_or(1.0);
    let x_padding = ((x_max - x_min).abs() * 0.08).max(1e-9);
    let y_padding = ((y_max - y_min).abs() * 0.08).max(1e-9);
    let x_low = x_min - x_padding;
    let x_high = x_max + x_padding;
    let y_low = y_min - y_padding;
    let y_high = y_max + y_padding;
    let grid = ui.visuals().widgets.noninteractive.bg_stroke.color;
    for step in 0..=4 {
        let fraction = step as f32 / 4.0;
        let x = egui::lerp(plot.left()..=plot.right(), fraction);
        let y = egui::lerp(plot.bottom()..=plot.top(), fraction);
        painter.line_segment(
            [egui::pos2(x, plot.top()), egui::pos2(x, plot.bottom())],
            egui::Stroke::new(0.7, grid),
        );
        painter.line_segment(
            [egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)],
            egui::Stroke::new(0.7, grid),
        );
    }
    painter.text(
        egui::pos2(plot.center().x, rect.bottom()),
        egui::Align2::CENTER_BOTTOM,
        x_label,
        egui::FontId::proportional(11.0),
        ui.visuals().text_color(),
    );
    painter.text(
        egui::pos2(rect.left(), plot.center().y),
        egui::Align2::LEFT_CENTER,
        y_label,
        egui::FontId::proportional(11.0),
        ui.visuals().text_color(),
    );

    let to_screen = |point: &ScatterPoint| {
        egui::pos2(
            plot.left() + plot.width() * ((point.x - x_low) / (x_high - x_low)) as f32,
            plot.bottom() - plot.height() * ((point.y - y_low) / (y_high - y_low)) as f32,
        )
    };
    let hover = response.hover_pos();
    let mut hovered = None;
    for point in points {
        let position = to_screen(point);
        painter.circle_filled(position, 4.2, GREEN);
        if hover.is_some_and(|hover| hover.distance(position) <= 8.0) {
            hovered = Some((point, position));
        }
    }
    if let Some((point, position)) = hovered {
        painter.text(
            position + egui::vec2(7.0, -7.0),
            egui::Align2::LEFT_BOTTOM,
            format!("{} ({:.3}, {:.3})", point.label, point.x, point.y),
            egui::FontId::monospace(10.0),
            ui.visuals().text_color(),
        );
    }
    response.on_hover_cursor(egui::CursorIcon::Crosshair);
}

#[allow(clippy::too_many_arguments)]
fn show_heatmap(
    ui: &mut egui::Ui,
    title: &str,
    row_labels: &[String],
    column_labels: &[String],
    values: &[Vec<f64>],
    minimum: f64,
    maximum: f64,
) {
    ui.label(egui::RichText::new(title).strong());
    if row_labels.is_empty() || column_labels.is_empty() {
        return;
    }
    let desired = egui::vec2(ui.available_width().min(900.0), 430.0);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let plot = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 105.0, rect.top() + 42.0),
        egui::pos2(rect.right() - 18.0, rect.bottom() - 18.0),
    );
    let painter = ui.painter_at(rect);
    let cell_width = plot.width() / column_labels.len() as f32;
    let cell_height = plot.height() / row_labels.len() as f32;
    for (row_index, row) in values.iter().enumerate() {
        for (column_index, value) in row.iter().enumerate() {
            let cell = egui::Rect::from_min_max(
                egui::pos2(
                    plot.left() + column_index as f32 * cell_width,
                    plot.top() + row_index as f32 * cell_height,
                ),
                egui::pos2(
                    plot.left() + (column_index + 1) as f32 * cell_width,
                    plot.top() + (row_index + 1) as f32 * cell_height,
                ),
            );
            painter.rect_filled(cell, 0.0, heatmap_color(*value, minimum, maximum));
        }
    }
    painter.rect_stroke(
        plot,
        0.0,
        egui::Stroke::new(1.0, ui.visuals().text_color()),
        egui::StrokeKind::Inside,
    );
    let row_step = row_labels.len().div_ceil(20).max(1);
    for (index, label) in row_labels.iter().enumerate().step_by(row_step) {
        painter.text(
            egui::pos2(
                plot.left() - 5.0,
                plot.top() + (index as f32 + 0.5) * cell_height,
            ),
            egui::Align2::RIGHT_CENTER,
            shorten_label(label, 15),
            egui::FontId::monospace(9.0),
            ui.visuals().text_color(),
        );
    }
    let column_step = column_labels.len().div_ceil(12).max(1);
    for (index, label) in column_labels.iter().enumerate().step_by(column_step) {
        painter.text(
            egui::pos2(
                plot.left() + (index as f32 + 0.5) * cell_width,
                plot.top() - 5.0,
            ),
            egui::Align2::CENTER_BOTTOM,
            shorten_label(label, 10),
            egui::FontId::monospace(9.0),
            ui.visuals().text_color(),
        );
    }
    if let Some(pointer) = response
        .hover_pos()
        .filter(|pointer| plot.contains(*pointer))
    {
        let column = (((pointer.x - plot.left()) / cell_width).floor() as usize)
            .min(column_labels.len() - 1);
        let row =
            (((pointer.y - plot.top()) / cell_height).floor() as usize).min(row_labels.len() - 1);
        painter.text(
            pointer + egui::vec2(8.0, -8.0),
            egui::Align2::LEFT_BOTTOM,
            format!(
                "{} × {}: {:.4}",
                row_labels[row], column_labels[column], values[row][column]
            ),
            egui::FontId::monospace(10.0),
            ui.visuals().text_color(),
        );
    }
    response.on_hover_cursor(egui::CursorIcon::Crosshair);
}

fn heatmap_color(value: f64, minimum: f64, maximum: f64) -> egui::Color32 {
    let negative_extent = minimum.abs().max(1e-12);
    let positive_extent = maximum.abs().max(1e-12);
    let intensity = if value < 0.0 {
        (value.abs() / negative_extent).clamp(0.0, 1.0)
    } else {
        (value / positive_extent).clamp(0.0, 1.0)
    } as f32;
    let white = egui::Rgba::from_rgb(0.97, 0.97, 0.96);
    let target = if value < 0.0 {
        egui::Rgba::from_rgb(0.18, 0.42, 0.72)
    } else {
        egui::Rgba::from_rgb(0.78, 0.23, 0.20)
    };
    egui::Color32::from(egui::lerp(white..=target, intensity))
}

fn shorten_label(label: &str, maximum: usize) -> String {
    if label.chars().count() <= maximum {
        label.to_owned()
    } else {
        let mut shortened = label
            .chars()
            .take(maximum.saturating_sub(1))
            .collect::<String>();
        shortened.push('…');
        shortened
    }
}

fn number(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(Value::as_f64)
}

fn localized<'a>(zh_cn: bool, zh: &'a str, en: &'a str) -> &'a str {
    if zh_cn { zh } else { en }
}

fn chart_color(index: usize) -> egui::Color32 {
    [GREEN, BLUE, AMBER, RED, CYAN][index % 5]
}

fn format_value(value: f64, percent: bool) -> String {
    if percent {
        format!("{value:.1}%")
    } else if value.abs() >= 1_000_000.0 {
        format!("{:.2}M", value / 1_000_000.0)
    } else if value.abs() >= 1_000.0 {
        format!("{:.2}K", value / 1_000.0)
    } else if value.fract().abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BarValue, ChartSpec, GREEN, bar_domain, chart_specs, line_series_shape,
        negative_log10_probability,
    };
    use eframe::egui;
    use serde_json::json;

    #[test]
    fn fastq_cycle_metrics_build_a_line_chart() {
        let payload = json!({
            "per_cycle": [
                {"cycle": 1, "mean_quality": 35.0, "q20_percent": 99.0, "q30_percent": 96.0},
                {"cycle": 2, "mean_quality": 34.0, "q20_percent": 98.0, "q30_percent": 94.0}
            ],
            "gc_percent": 52.0,
            "n_percent": 0.2
        });
        let charts = chart_specs(&payload, Some("fastq.qc.v1"), false);
        assert!(matches!(charts.first(), Some(ChartSpec::Lines { .. })));
    }

    #[test]
    fn annotation_statistics_build_feature_and_sequence_charts() {
        let payload = json!({
            "feature_type_counts": {"gene": 4, "mRNA": 3, "exon": 12},
            "sequence_counts": {"chr1": 10, "chr2": 9}
        });
        let charts = chart_specs(&payload, Some("annotation.gxf.stats.v1"), true);
        assert_eq!(charts.len(), 2);
        assert!(matches!(charts.first(), Some(ChartSpec::Bars { .. })));
    }

    #[test]
    fn functional_annotation_and_enrichment_results_build_native_charts() {
        let go = json!({
            "input_row_count": 5,
            "gene_count": 3,
            "term_count": 3,
            "association_count": 5
        });
        let eggnog = json!({
            "input_row_count": 2,
            "query_count": 2,
            "go_association_count": 4,
            "kegg_association_count": 3
        });
        let enrichment = json!({
            "terms": [
                {
                    "term_id": "GO:0000001",
                    "term_name": "Example process",
                    "adjusted_p_value": 0.0,
                    "fold_enrichment": 2.5
                },
                {
                    "term_id": "GO:0000002",
                    "term_name": null,
                    "adjusted_p_value": 0.05,
                    "fold_enrichment": 1.2
                }
            ]
        });

        assert_eq!(
            chart_specs(&go, Some("annotation.go.normalize.v1"), false).len(),
            1
        );
        assert_eq!(
            chart_specs(&eggnog, Some("annotation.eggnog.normalize.v1"), true).len(),
            1
        );
        let charts = chart_specs(&enrichment, Some("enrichment.go.v1"), false);
        assert_eq!(charts.len(), 2);
        let ChartSpec::Bars { values, .. } = &charts[0] else {
            panic!("expected enrichment significance bars");
        };
        assert!(values.iter().all(|value| value.value.is_finite()));
        assert_eq!(values[0].value, 300.0);
    }

    #[test]
    fn enrichment_probability_transform_is_finite_and_bounded() {
        assert_eq!(negative_log10_probability(0.0), 300.0);
        assert_eq!(negative_log10_probability(1.0), 0.0);
        assert_eq!(negative_log10_probability(-1.0), 0.0);
        assert_eq!(negative_log10_probability(f64::NAN), 0.0);
    }

    #[test]
    fn coordinate_structure_results_build_domain_charts() {
        let sequence = json!({
            "chains": [
                {"chain_id": "A", "residue_count": 4},
                {"chain_id": "B", "residue_count": 2}
            ]
        });
        let contacts = json!({
            "contacts": [{
                "left": {"chain_id": "A", "residue_id": "1"},
                "right": {"chain_id": "B", "residue_id": "2"},
                "distance_angstrom": 4.5
            }]
        });
        let geometry = json!({"measurement": "angle", "value": 90.0});
        let superposition = json!({
            "rmsd_before_angstrom": 8.0,
            "rmsd_after_angstrom": 0.4
        });

        assert_eq!(
            chart_specs(&sequence, Some("structure.sequence.extract.v1"), false).len(),
            1
        );
        assert_eq!(
            chart_specs(&contacts, Some("structure.contact-map.v1"), false).len(),
            1
        );
        assert_eq!(
            chart_specs(&geometry, Some("structure.geometry.v1"), false).len(),
            1
        );
        let charts = chart_specs(&superposition, Some("structure.superpose.v1"), false);
        let ChartSpec::Bars { values, .. } = &charts[0] else {
            panic!("expected superposition RMSD bars");
        };
        assert_eq!(values.len(), 2);
        assert_eq!(values[1].value, 0.4);
    }

    #[test]
    fn similarity_domain_density_and_tree_results_build_native_charts() {
        let density = json!({
            "bins": [
                {"seqid": "chr1", "start": 1, "end": 100, "features_per_megabase": 20_000.0},
                {"seqid": "chr1", "start": 101, "end": 200, "features_per_megabase": 10_000.0}
            ]
        });
        let density_charts = chart_specs(&density, Some("genome.gene-density.v1"), false);
        assert_eq!(density_charts.len(), 1);
        assert!(matches!(density_charts[0], ChartSpec::Lines { .. }));

        let blast = json!({
            "record_count": 2,
            "query_count": 1,
            "subject_count": 2,
            "hits": [
                {"query_id": "q1", "subject_id": "s1", "bit_score": 80.0},
                {"query_id": "q1", "subject_id": "s2", "bit_score": 60.0}
            ]
        });
        let blast_charts = chart_specs(&blast, Some("similarity.blast.parse.v1"), false);
        assert_eq!(blast_charts.len(), 2);

        let reciprocal = json!({
            "pairs": [{
                "left_id": "q1",
                "right_id": "s1",
                "forward_identity_percent": 95.0,
                "reverse_identity_percent": 92.0
            }]
        });
        let reciprocal_charts = chart_specs(&reciprocal, Some("similarity.reciprocal.v1"), false);
        let ChartSpec::Bars {
            values, percent, ..
        } = &reciprocal_charts[0]
        else {
            panic!("expected reciprocal identity bars");
        };
        assert!(*percent);
        assert_eq!(values[0].value, 92.0);

        let domains = json!({"source_counts": {"Pfam": 4, "SMART": 2}});
        let domain_charts = chart_specs(&domains, Some("protein.domain.parse.v1"), false);
        assert_eq!(domain_charts.len(), 1);

        let tree = json!({
            "leaf_count": 4,
            "internal_node_count": 3,
            "max_depth": 3,
            "relabeled_count": 1
        });
        let tree_charts = chart_specs(&tree, Some("phylogeny.tree.transform.v1"), true);
        let ChartSpec::Bars { values, .. } = &tree_charts[0] else {
            panic!("expected phylogeny summary bars");
        };
        assert_eq!(values.len(), 4);
    }

    #[test]
    fn a_single_fastq_cycle_is_drawn_as_a_point() {
        let shape = line_series_shape(vec![egui::pos2(12.0, 24.0)], GREEN)
            .expect("single point should produce a shape");
        assert!(matches!(shape, egui::Shape::Circle(_)));
    }

    #[test]
    fn sequence_percentages_keep_their_distinct_denominators() {
        let payload = json!({
            "n50": 10,
            "gc_percent": 60.0,
            "n_percent": 20.0
        });
        let charts = chart_specs(&payload, Some("sequence.stats.v1"), false);
        let ChartSpec::Bars {
            title,
            values,
            percent,
        } = charts.last().expect("percentage chart")
        else {
            panic!("expected percentage bars");
        };

        assert_eq!(title, "Base percentage metrics");
        assert!(*percent);
        assert_eq!(values.len(), 2);
        assert_eq!(values[0].value, 60.0);
        assert_eq!(values[1].value, 20.0);
    }

    #[test]
    fn variant_contigs_are_limited_to_twelve() {
        let contigs = (0..20)
            .map(|index| (format!("chr{index}"), json!(index)))
            .collect::<serde_json::Map<_, _>>();
        let payload = json!({"snp_count": 4, "contig_counts": contigs});
        let charts = chart_specs(&payload, Some("variant.stats.v1"), false);
        let contig_chart = charts.last().expect("contig chart");
        let ChartSpec::Bars { values, .. } = contig_chart else {
            panic!("expected bar chart");
        };
        assert_eq!(values.len(), 12);
        assert_eq!(values[0].value, 19.0);
    }

    #[test]
    fn interval_set_operations_build_output_charts() {
        let merge = json!({
            "input_interval_count": 3,
            "output_interval_count": 2,
            "merged_interval_count": 1,
            "input_bases": 27,
            "output_bases": 20,
            "contigs": {
                "chr1": {"output_bases": 13},
                "chr2": {"output_bases": 7}
            }
        });
        let merge_charts = chart_specs(&merge, Some("interval.merge.v1"), false);
        assert_eq!(merge_charts.len(), 3);
        let ChartSpec::Bars { values, .. } = &merge_charts[0] else {
            panic!("expected merge summary chart");
        };
        assert_eq!(values[0].label, "Input intervals");

        let subtract = json!({
            "left_interval_count": 3,
            "right_interval_count": 3,
            "output_interval_count": 3,
            "affected_left_interval_count": 3,
            "removed_bases": 12,
            "output_bases": 15,
            "contigs": {
                "chr1": {"output_bases": 10},
                "chr2": {"output_bases": 5}
            }
        });
        let subtract_charts = chart_specs(&subtract, Some("interval.subtract.v1"), false);
        assert_eq!(subtract_charts.len(), 3);
        let ChartSpec::Bars { values, .. } = &subtract_charts[1] else {
            panic!("expected subtraction bases chart");
        };
        assert_eq!(values[0].label, "Removed bases");
    }

    #[test]
    fn expression_samples_build_summary_charts() {
        let payload = json!({
            "numeric_value_count": 5,
            "zero_value_count": 2,
            "missing_value_count": 1,
            "negative_value_count": 0,
            "samples": [
                {"sample": "a", "total": 10.0},
                {"sample": "b", "total": 20.0}
            ]
        });
        let charts = chart_specs(&payload, Some("expression.matrix.qc.v1"), false);
        assert_eq!(charts.len(), 2);
        let ChartSpec::Bars { values, .. } = &charts[1] else {
            panic!("expected sample bar chart");
        };
        assert_eq!(values[0].label, "b");
    }

    #[test]
    fn expression_pca_and_heatmap_build_native_chart_specs() {
        let pca = json!({
            "components": [
                {"component": 1, "explained_variance_percent": 80.0},
                {"component": 2, "explained_variance_percent": 15.0}
            ],
            "samples": [
                {"sample": "a", "scores": [-2.0, 0.5]},
                {"sample": "b", "scores": [2.0, -0.5]}
            ]
        });
        let charts = chart_specs(&pca, Some("expression.pca.v1"), false);
        assert_eq!(charts.len(), 2);
        assert!(matches!(charts[1], ChartSpec::Scatter { .. }));

        let heatmap = json!({
            "row_labels": ["g1", "g2"],
            "column_labels": ["s1", "s2"],
            "values": [[-1.0, 1.0], [1.0, -1.0]],
            "minimum_value": -1.0,
            "maximum_value": 1.0
        });
        let charts = chart_specs(&heatmap, Some("expression.heatmap.v1"), true);
        assert_eq!(charts.len(), 1);
        assert!(matches!(charts[0], ChartSpec::Heatmap { .. }));
    }

    #[test]
    fn negative_expression_totals_use_a_signed_domain() {
        let payload = json!({
            "numeric_value_count": 4,
            "zero_value_count": 0,
            "missing_value_count": 0,
            "negative_value_count": 2,
            "samples": [
                {"sample": "negative", "total": -30.0},
                {"sample": "positive", "total": 20.0}
            ]
        });
        let charts = chart_specs(&payload, Some("expression.matrix.qc.v1"), false);
        let ChartSpec::Bars { title, values, .. } = &charts[1] else {
            panic!("expected sample bar chart");
        };

        assert_eq!(title, "Signed sample totals");
        assert_eq!(values[0].label, "negative");
        assert_eq!(bar_domain(values, false), (-30.0, 20.0));
    }

    #[test]
    fn table_manipulation_builds_row_and_column_charts() {
        let payload = json!({
            "input_rows": 4,
            "skipped_rows": 1,
            "filtered_rows": 1,
            "output_rows": 2,
            "input_columns": 4,
            "output_columns": 2
        });
        let charts = chart_specs(&payload, Some("table.manipulate.v1"), false);
        assert_eq!(charts.len(), 2);
        let ChartSpec::Bars { title, values, .. } = &charts[0] else {
            panic!("expected row handling bar chart");
        };

        assert_eq!(title, "Table row handling");
        assert_eq!(values[0].label, "Input rows");
        assert_eq!(values[3].value, 2.0);
    }

    #[test]
    fn set_analysis_builds_size_and_exact_intersection_charts() {
        let payload = json!({
            "set_sizes": [
                {"name": "control", "count": 3},
                {"name": "treated", "count": 4}
            ],
            "intersections": [
                {"sets": ["control", "treated"], "degree": 2, "count": 2},
                {"sets": ["treated"], "degree": 1, "count": 2}
            ]
        });
        let charts = chart_specs(&payload, Some("set.upset.v1"), false);
        assert_eq!(charts.len(), 2);
        let ChartSpec::Bars { title, values, .. } = &charts[1] else {
            panic!("expected exact-intersection bar chart");
        };
        assert_eq!(title, "Exact intersection sizes");
        assert_eq!(values[0].label, "control ∩ treated");
        assert_eq!(values[0].value, 2.0);
    }

    #[test]
    fn protein_properties_builds_length_mass_and_pi_charts() {
        let payload = json!({
            "records": [
                {
                    "id": "protein-a",
                    "length": 120,
                    "molecular_weight_da": 13500.0,
                    "isoelectric_point": 6.2
                },
                {
                    "id": "ambiguous",
                    "length": 20,
                    "molecular_weight_da": null,
                    "isoelectric_point": null
                }
            ]
        });
        let charts = chart_specs(&payload, Some("protein.properties.v1"), true);
        assert_eq!(charts.len(), 3);
        let ChartSpec::Bars { values, .. } = &charts[0] else {
            panic!("expected protein-length chart");
        };
        assert_eq!(values.len(), 2);
        let ChartSpec::Bars { values, .. } = &charts[1] else {
            panic!("expected protein-mass chart");
        };
        assert_eq!(values.len(), 1);
    }

    #[test]
    fn an_all_zero_bar_chart_has_a_nonzero_domain() {
        let values = vec![BarValue {
            label: "zero".to_owned(),
            value: 0.0,
        }];
        assert_eq!(bar_domain(&values, false), (0.0, 1.0));
    }
}
