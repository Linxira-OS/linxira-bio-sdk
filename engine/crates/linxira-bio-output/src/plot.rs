//! The unified plot parameter contract (M1 types; renderers arrive with the
//! matplotlib/ggplot2 packs).
//!
//! All three rendering backends consume the same [`PlotSpec`] JSON so a
//! single axis or palette change stays consistent across matplotlib,
//! ggplot2, and the zero-dependency Rust SVG fallback.

use serde::{Deserialize, Serialize};

/// Unified rendering parameters shared by every plot backend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlotSpec {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub subtitle: String,
    /// Defaults to a label derived from the input file name.
    #[serde(default)]
    pub x_label: Option<String>,
    #[serde(default)]
    pub y_label: Option<String>,
    #[serde(default)]
    pub x_range: Option<[f64; 2]>,
    #[serde(default)]
    pub y_range: Option<[f64; 2]>,
    #[serde(default)]
    pub x_log: bool,
    #[serde(default)]
    pub y_log: bool,
    #[serde(default = "default_true")]
    pub grid: bool,
    #[serde(default)]
    pub theme: PlotTheme,
    #[serde(default = "default_palette")]
    pub palette: String,
    #[serde(default = "default_true")]
    pub legend: bool,
    #[serde(default)]
    pub figure: PlotFigure,
    #[serde(default)]
    pub font: PlotFont,
    #[serde(default)]
    pub output: PlotOutput,
    /// The plot payload (point series, matrices, ...) assembled by the
    /// capability; backends treat it as read-only.
    pub data: serde_json::Value,
}

impl Default for PlotSpec {
    fn default() -> Self {
        serde_json::from_value(serde_json::json!({
            "data": {}
        }))
        .expect("the default PlotSpec carries an empty data object")
    }
}

fn default_true() -> bool {
    true
}

fn default_palette() -> String {
    "set2".to_owned()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlotTheme {
    Dark,
    #[default]
    Light,
    Publication,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlotFigure {
    pub width: u32,
    pub height: u32,
    pub dpi: u32,
}

impl Default for PlotFigure {
    fn default() -> Self {
        Self {
            width: 800,
            height: 600,
            dpi: 150,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlotFont {
    /// Empty means the platform default font.
    #[serde(default)]
    pub family: Option<String>,
    pub size: u32,
}

impl Default for PlotFont {
    fn default() -> Self {
        Self {
            family: None,
            size: 12,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlotOutput {
    pub format: PlotOutputFormat,
    #[serde(default)]
    pub interactive: bool,
}

impl Default for PlotOutput {
    fn default() -> Self {
        Self {
            format: PlotOutputFormat::Svg,
            interactive: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlotOutputFormat {
    #[default]
    Svg,
    Png,
    Pdf,
}

#[cfg(test)]
mod tests {
    use super::{PlotOutputFormat, PlotSpec, PlotTheme};

    #[test]
    fn partial_specs_fill_the_documented_defaults() {
        let spec: PlotSpec =
            serde_json::from_str(r#"{"data": {"points": [{"x": 1, "y": 2}]}}"#).expect("spec");
        assert_eq!(spec.title, "");
        assert!(spec.grid);
        assert!(spec.legend);
        assert_eq!(spec.theme, PlotTheme::Light);
        assert_eq!(spec.palette, "set2");
        assert_eq!(spec.figure.width, 800);
        assert_eq!(spec.figure.height, 600);
        assert_eq!(spec.figure.dpi, 150);
        assert_eq!(spec.font.size, 12);
        assert_eq!(spec.font.family, None);
        assert_eq!(spec.output.format, PlotOutputFormat::Svg);
        assert!(!spec.output.interactive);
    }

    #[test]
    fn data_is_required_and_overrides_survive_a_roundtrip() {
        assert!(serde_json::from_str::<PlotSpec>("{}").is_err());

        let spec: PlotSpec = serde_json::from_str(
            r#"{"data": {}, "theme": "publication", "output": {"format": "pdf"}}"#,
        )
        .expect("spec");
        assert_eq!(spec.theme, PlotTheme::Publication);
        assert_eq!(spec.output.format, PlotOutputFormat::Pdf);

        let json = serde_json::to_value(&spec).expect("serialize");
        let parsed: PlotSpec = serde_json::from_value(json).expect("deserialize");
        assert_eq!(parsed, spec);
    }
}
