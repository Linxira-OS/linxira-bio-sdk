use flate2::read::{MultiGzDecoder, ZlibDecoder};
use serde::Serialize;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// Summary of an mzML mass-spectrometry analysis: spectrum metadata plus a
/// centroid peak table from local-maximum peak picking.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct MetabolomicsResult {
    pub spectrum_count: u64,
    pub ms1_count: u64,
    pub ms2_count: u64,
    pub peak_count: u64,
    pub peak_table: Vec<Peak>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Peak {
    pub spectrum_index: u64,
    pub retention_time_min: Option<f64>,
    pub mz: f64,
    pub intensity: f64,
}

#[derive(Debug)]
pub enum MetabolomicsError {
    Io(io::Error),
    MissingSpectra,
    MalformedSpectrum { message: String },
    InvalidBase64 { message: String },
}

impl Display for MetabolomicsError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "failed to read mzML: {error}"),
            Self::MissingSpectra => formatter.write_str("mzML contains no spectra"),
            Self::MalformedSpectrum { message } => {
                write!(formatter, "malformed mzML spectrum: {message}")
            }
            Self::InvalidBase64 { message } => {
                write!(formatter, "invalid mzML binary data: {message}")
            }
        }
    }
}

impl Error for MetabolomicsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for MetabolomicsError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Analyze an mzML file (optionally gzip-compressed).
pub fn metabolomics_path(path: impl AsRef<Path>) -> Result<MetabolomicsResult, MetabolomicsError> {
    let path = path.as_ref();
    let mut magic = [0_u8; 2];
    let mut probe = File::open(path)?;
    let magic_length = probe.read(&mut magic)?;
    let input: Box<dyn Read> = if magic_length == magic.len() && magic == [0x1f, 0x8b] {
        Box::new(MultiGzDecoder::new(File::open(path)?))
    } else {
        Box::new(File::open(path)?)
    };
    let mut text = String::new();
    let mut reader = io::BufReader::new(input);
    reader.read_to_string(&mut text)?;
    metabolomics(&text)
}

fn metabolomics(text: &str) -> Result<MetabolomicsResult, MetabolomicsError> {
    let mut result = MetabolomicsResult::default();
    let mut offset = 0_usize;
    let mut found = 0_u64;
    while let Some(relative) = text[offset..].find("<spectrum ") {
        let start = offset + relative;
        let end = text[start..].find("</spectrum>").ok_or_else(|| {
            MetabolomicsError::MalformedSpectrum {
                message: "spectrum element is not closed".to_owned(),
            }
        })? + start;
        found += 1;
        parse_spectrum(&text[start..end], &mut result)?;
        offset = end + "</spectrum>".len();
    }
    result.spectrum_count = found;
    if found == 0 {
        return Err(MetabolomicsError::MissingSpectra);
    }
    if result.peak_count == 0 {
        result
            .warnings
            .push("no peaks detected above the local-maximum threshold".to_owned());
    }
    Ok(result)
}

fn parse_spectrum(
    spectrum: &str,
    result: &mut MetabolomicsResult,
) -> Result<(), MetabolomicsError> {
    let index = spectrum
        .find("index=\"")
        .and_then(|position| {
            let tail = &spectrum[position + "index=\"".len()..];
            tail.find('"').map(|length| &tail[..length])
        })
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| MetabolomicsError::MalformedSpectrum {
            message: "missing spectrum index".to_owned(),
        })?;

    let ms_level = cv_param_value(spectrum, "MS:1000511")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(1);
    if ms_level == 1 {
        result.ms1_count += 1;
    } else if ms_level == 2 {
        result.ms2_count += 1;
    }
    let retention_time_min =
        cv_param_value(spectrum, "MS:1000016").and_then(|value| value.parse::<f64>().ok());

    // Binary arrays: m/z (MS:1000514) and intensity (MS:1000515).
    let mut mz: Option<Vec<f64>> = None;
    let mut intensity: Option<Vec<f64>> = None;
    let mut array_offset = 0_usize;
    while let Some(relative) = spectrum[array_offset..].find("<binaryDataArray ") {
        let start = array_offset + relative;
        let end = spectrum[start..]
            .find("</binaryDataArray>")
            .ok_or_else(|| MetabolomicsError::MalformedSpectrum {
                message: "binaryDataArray is not closed".to_owned(),
            })?
            + start;
        let array = &spectrum[start..end];
        let accession = cv_param_value(array, "MS:1000514")
            .map(|_| "mz")
            .or_else(|| cv_param_value(array, "MS:1000515").map(|_| "intensity"));
        if let Some(kind) = accession {
            let values = decode_binary_array(array)?;
            match kind {
                "mz" => mz = Some(values),
                _ => intensity = Some(values),
            }
        }
        array_offset = end + "</binaryDataArray>".len();
    }
    let (Some(mz), Some(intensity)) = (mz, intensity) else {
        return Ok(());
    };
    if mz.len() != intensity.len() {
        return Err(MetabolomicsError::MalformedSpectrum {
            message: format!(
                "spectrum {index} has mismatched m/z ({}) and intensity ({}) lengths",
                mz.len(),
                intensity.len()
            ),
        });
    }
    for peak in detect_peaks(&mz, &intensity) {
        result.peak_count += 1;
        result.peak_table.push(Peak {
            spectrum_index: index,
            retention_time_min,
            mz: peak.0,
            intensity: peak.1,
        });
    }
    Ok(())
}

/// Extract the value of a CV param by accession (`<cvParam accession="..." value="..." .../>`).
fn cv_param_value(xml: &str, accession: &str) -> Option<String> {
    let needle = format!("accession=\"{accession}\"");
    let position = xml.find(&needle)?;
    let tail = &xml[position + needle.len()..];
    let value_position = tail.find("value=\"")?;
    let value_start = value_position + "value=\"".len();
    let value = &tail[value_start..];
    let end = value.find('"')?;
    Some(value[..end].to_owned())
}

fn decode_binary_array(array: &str) -> Result<Vec<f64>, MetabolomicsError> {
    let is_64_bit = cv_param_value(array, "MS:1000521").is_some();
    let is_compressed = cv_param_value(array, "MS:1000574").is_some();
    let binary = array
        .find("<binary>")
        .and_then(|position| {
            let tail = &array[position + "<binary>".len()..];
            tail.find("</binary>").map(|end| &tail[..end])
        })
        .ok_or_else(|| MetabolomicsError::MalformedSpectrum {
            message: "binaryDataArray lacks a <binary> element".to_owned(),
        })?;
    let bytes = decode_base64(binary)?;
    let bytes = if is_compressed {
        let mut decoder = ZlibDecoder::new(&bytes[..]);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).map_err(|error| {
            MetabolomicsError::InvalidBase64 {
                message: format!("zlib decompression failed: {error}"),
            }
        })?;
        decompressed
    } else {
        bytes
    };
    let mut values = Vec::with_capacity(bytes.len() / if is_64_bit { 8 } else { 4 });
    if is_64_bit {
        for chunk in bytes.chunks_exact(8) {
            values.push(f64::from_le_bytes(chunk.try_into().expect("8 bytes")));
        }
    } else {
        for chunk in bytes.chunks_exact(4) {
            values.push(f32::from_le_bytes(chunk.try_into().expect("4 bytes")) as f64);
        }
    }
    Ok(values)
}

/// Find local-maximum peaks: points strictly greater than both neighbors with
/// positive intensity. Returns (mz, intensity) pairs.
fn detect_peaks(mz: &[f64], intensity: &[f64]) -> Vec<(f64, f64)> {
    let mut peaks = Vec::new();
    for index in 1..mz.len().saturating_sub(1) {
        let value = intensity[index];
        if value > 0.0 && value > intensity[index - 1] && value >= intensity[index + 1] {
            peaks.push((mz[index], value));
        }
    }
    peaks
}

fn decode_base64(input: &str) -> Result<Vec<u8>, MetabolomicsError> {
    let mut table = [255_u8; 256];
    for (index, byte) in b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
        .iter()
        .enumerate()
    {
        table[*byte as usize] = index as u8;
    }
    let mut output = Vec::with_capacity(input.len() / 4 * 3);
    let mut accumulator = 0_u32;
    let mut bits = 0_u32;
    for byte in input.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
        if byte == b'=' {
            break;
        }
        let value = table[byte as usize];
        if value == 255 {
            return Err(MetabolomicsError::InvalidBase64 {
                message: format!("invalid base64 character 0x{byte:02x}"),
            });
        }
        accumulator = (accumulator << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((accumulator >> bits) as u8);
        }
    }
    Ok(output)
}

/// Render the peak table as TSV (spectrum_index, retention_time_min, mz, intensity).
pub fn render_peak_table(result: &MetabolomicsResult) -> String {
    let mut table = String::from("spectrum_index\tretention_time_min\tmz\tintensity\n");
    for peak in &result.peak_table {
        table.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            peak.spectrum_index,
            peak.retention_time_min
                .map(|value| value.to_string())
                .unwrap_or_else(|| ".".to_owned()),
            peak.mz,
            peak.intensity
        ));
    }
    table
}

#[cfg(test)]
mod tests {
    use super::{decode_base64, metabolomics, render_peak_table};

    #[test]
    fn decodes_base64_into_bytes() {
        assert_eq!(decode_base64("AQIDBA==").expect("base64"), vec![1, 2, 3, 4]);
        assert_eq!(decode_base64("aGVsbG8=").expect("base64"), b"hello");
        assert!(decode_base64("a!b").is_err());
    }

    #[test]
    fn parses_mzml_and_detects_peaks() {
        let mz = vec![100.0_f32, 200.0, 300.0, 400.0, 500.0];
        let intensity = vec![10.0_f32, 500.0, 20.0, 800.0, 5.0];
        let mz_b64 = base64(&to_le_bytes_f32(&mz));
        let int_b64 = base64(&to_le_bytes_f32(&intensity));
        let mzml = format!(
            "<?xml version=\"1.0\"?><mzML><run><spectrumList count=\"1\">\
             <spectrum id=\"scan=1\" index=\"0\" defaultArrayLength=\"5\">\
             <cvParam accession=\"MS:1000511\" name=\"ms level\" value=\"1\"/>\
             <cvParam accession=\"MS:1000016\" name=\"scan start time\" value=\"2.5\" unitAccession=\"MS:1000038\" unitName=\"minute\"/>\
             <binaryDataArrayList count=\"2\">\
             <binaryDataArray encodedLength=\"20\">\
             <cvParam accession=\"MS:1000523\" name=\"32-bit float\" value=\"\"/>\
             <cvParam accession=\"MS:1000514\" name=\"m/z array\" value=\"\"/>\
             <binary>{mz_b64}</binary></binaryDataArray>\
             <binaryDataArray encodedLength=\"20\">\
             <cvParam accession=\"MS:1000523\" name=\"32-bit float\" value=\"\"/>\
             <cvParam accession=\"MS:1000515\" name=\"intensity array\" value=\"\"/>\
             <binary>{int_b64}</binary></binaryDataArray>\
             </binaryDataArrayList></spectrum></spectrumList></run></mzML>"
        );
        let result = metabolomics(&mzml).expect("parse mzML");
        assert_eq!(result.spectrum_count, 1);
        assert_eq!(result.ms1_count, 1);
        assert_eq!(result.peak_count, 2);
        assert_eq!(result.peak_table[0].mz, 200.0);
        assert_eq!(result.peak_table[0].intensity, 500.0);
        assert_eq!(result.peak_table[0].retention_time_min, Some(2.5));
        assert_eq!(result.peak_table[1].mz, 400.0);
        let table = render_peak_table(&result);
        assert!(table.starts_with("spectrum_index\tretention_time_min\tmz\tintensity\n"));
        assert!(table.contains("0\t2.5\t200\t500"));
    }

    #[test]
    fn rejects_mzml_without_spectra() {
        assert!(metabolomics("<mzML><run/></mzML>").is_err());
    }

    fn base64(bytes: &[u8]) -> String {
        const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut output = String::new();
        for chunk in bytes.chunks(3) {
            let mut buffer = [0_u8; 3];
            buffer[..chunk.len()].copy_from_slice(chunk);
            let value =
                (u32::from(buffer[0]) << 16) | (u32::from(buffer[1]) << 8) | u32::from(buffer[2]);
            output.push(TABLE[(value >> 18) as usize & 63] as char);
            output.push(TABLE[(value >> 12) as usize & 63] as char);
            output.push(if chunk.len() > 1 {
                TABLE[(value >> 6) as usize & 63] as char
            } else {
                '='
            });
            output.push(if chunk.len() > 2 {
                TABLE[value as usize & 63] as char
            } else {
                '='
            });
        }
        output
    }

    fn to_le_bytes_f32(values: &[f32]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(values.len() * 4);
        for value in values {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes
    }
}
