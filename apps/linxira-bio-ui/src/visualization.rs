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
        }
    }
    true
}

fn chart_specs(payload: &Value, capability: Option<&str>, zh_cn: bool) -> Vec<ChartSpec> {
    match capability.unwrap_or_default() {
        "sequence.stats.v1" => sequence_charts(payload, zh_cn),
        "fastq.qc.v1" => fastq_charts(payload, zh_cn),
        "alignment.qc.v1" => alignment_charts(payload, zh_cn),
        "interval.intersect.v1" => interval_charts(payload, zh_cn),
        "interval.merge.v1" => interval_merge_charts(payload, zh_cn),
        "interval.subtract.v1" => interval_subtract_charts(payload, zh_cn),
        "expression.matrix.qc.v1" => expression_charts(payload, zh_cn),
        "variant.stats.v1" => variant_charts(payload, zh_cn),
        "structure.pdb.summary.v1" => structure_charts(payload, zh_cn),
        _ if payload.get("per_cycle").is_some() => fastq_charts(payload, zh_cn),
        _ if payload.get("contig_counts").is_some() => variant_charts(payload, zh_cn),
        _ if payload.get("n50").is_some() => sequence_charts(payload, zh_cn),
        _ => Vec::new(),
    }
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
    use super::{BarValue, ChartSpec, GREEN, bar_domain, chart_specs, line_series_shape};
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
    fn an_all_zero_bar_chart_has_a_nonzero_domain() {
        let values = vec![BarValue {
            label: "zero".to_owned(),
            value: 0.0,
        }];
        assert_eq!(bar_domain(&values, false), (0.0, 1.0));
    }
}
