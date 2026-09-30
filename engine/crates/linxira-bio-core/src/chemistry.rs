use serde::Serialize;

/// One scored pose from a completed AutoDock Vina run, ordered by ascending
/// predicted binding affinity (Vina always emits rank 1 first).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VinaDockMode {
    pub rank: u32,
    pub affinity_kcal_per_mol: f64,
    pub rmsd_lb: Option<f64>,
    pub rmsd_ub: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VinaDockSummary {
    pub modes: Vec<VinaDockMode>,
    pub best_affinity_kcal_per_mol: Option<f64>,
}

/// Parse the pose table that AutoDock Vina (1.x and 1.2.x) prints to stdout
/// after a docking run:
///
/// ```text
/// mode |   affinity | dist from best mode
///      | (kcal/mol) | rmsd l.b.| rmsd u.b.
/// -----+------------+----------+----------
///    1       -6.750          0.000      0.000
/// ```
///
/// Parsing starts at the header line containing both "mode" and "affinity",
/// skips the unit and separator lines, and stops at the first line that is
/// neither blank nor a four-number row.
pub fn parse_vina_pose_table(text: &str) -> VinaDockSummary {
    let mut modes = Vec::new();
    let mut in_table = false;
    let mut saw_row = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if !in_table {
            if trimmed.contains("mode") && trimmed.contains("affinity") {
                in_table = true;
            }
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with('-') {
            continue;
        }
        // Before the first data row the unit line "(kcal/mol)" and similar
        // decorations appear; after it, any non-row line ends the table.
        let Some(fields) = split_pose_row(trimmed) else {
            if saw_row {
                break;
            }
            continue;
        };
        saw_row = true;
        modes.push(fields);
    }
    let best_affinity_kcal_per_mol = modes.first().map(|mode| mode.affinity_kcal_per_mol);
    VinaDockSummary {
        modes,
        best_affinity_kcal_per_mol,
    }
}

/// Recognise a pose row (rank plus three numbers); anything else is None.
fn split_pose_row(trimmed: &str) -> Option<VinaDockMode> {
    let fields: Vec<&str> = trimmed.split_whitespace().collect();
    if fields.len() != 4 {
        return None;
    }
    let rank = fields[0].parse::<u32>().ok()?;
    let affinity = fields[1].parse::<f64>().ok()?;
    let rmsd_lb = fields[2].parse::<f64>().ok();
    let rmsd_ub = fields[3].parse::<f64>().ok();
    if rmsd_lb.is_none() && rmsd_ub.is_none() {
        return None;
    }
    Some(VinaDockMode {
        rank,
        affinity_kcal_per_mol: affinity,
        rmsd_lb,
        rmsd_ub,
    })
}

#[cfg(test)]
mod tests {
    use super::{VinaDockMode, parse_vina_pose_table};

    const VINA_OUTPUT: &str = "AutoDock Vina v1.2.5\n#################################################################\n# If you used AutoDock Vina in your work, please cite:           #\n#################################################################\n\nComputing Vina grid ... done.\nEstimated energy of the grid interaction per atom type:\n... table elided ...\n\nPerforming docking ...\nmode |   affinity | dist from best mode\n     | (kcal/mol) | rmsd l.b.| rmsd u.b.\n-----+------------+----------+----------\n   1       -6.750          0.000      0.000\n   2       -6.412          1.679      2.524\n   3       -6.288          1.992      2.907\n   4       -6.204          2.311      3.412\n";

    #[test]
    fn parses_vina_pose_table_with_best_mode_first() {
        let summary = parse_vina_pose_table(VINA_OUTPUT);
        assert_eq!(summary.modes.len(), 4);
        assert_eq!(summary.best_affinity_kcal_per_mol, Some(-6.750));
        assert_eq!(
            summary.modes[0],
            VinaDockMode {
                rank: 1,
                affinity_kcal_per_mol: -6.750,
                rmsd_lb: Some(0.000),
                rmsd_ub: Some(0.000),
            }
        );
        assert_eq!(summary.modes[3].rank, 4);
        assert_eq!(summary.modes[3].affinity_kcal_per_mol, -6.204);
    }

    #[test]
    fn handles_output_without_a_pose_table() {
        let summary = parse_vina_pose_table("AutoDock Vina v1.2.5\nPerforming scoring only ...\n");
        assert!(summary.modes.is_empty());
        assert_eq!(summary.best_affinity_kcal_per_mol, None);
    }

    #[test]
    fn stops_at_trailing_text_after_the_table() {
        let text = "mode |   affinity | dist from best mode\n     | (kcal/mol) | rmsd l.b.| rmsd u.b.\n-----+------------+----------+----------\n   1       -6.750          0.000      0.000\n\nReference RMSD: 1.234\n";
        let summary = parse_vina_pose_table(text);
        assert_eq!(summary.modes.len(), 1);
    }
}
