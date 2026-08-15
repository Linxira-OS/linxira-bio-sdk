use flate2::read::MultiGzDecoder;
use serde::Serialize;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

/// Reference build for the built-in PGx allele table (GRCh38).
pub const PGX_REFERENCE_BUILD: &str = "GRCh38";

/// One pharmacogenomic allele rule matched by variant coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct PgxAlleleRule {
    pub chrom: &'static str,
    pub position: u64,
    pub reference: &'static str,
    pub alternate: &'static str,
    pub rsid: &'static str,
    pub gene: &'static str,
    pub allele: &'static str,
    pub consequence: &'static str,
    pub phenotype: &'static str,
    pub drugs: &'static [&'static str],
}

/// GRCh38 allele table. Research-use-only reference facts for common
/// pharmacogenomic star alleles; not a substitute for clinical genotyping.
#[rustfmt::skip]
pub const PGX_ALLELES: &[PgxAlleleRule] = &[
    PgxAlleleRule { chrom: "chr10", position: 96_541_605, reference: "G", alternate: "A", rsid: "rs4244285", gene: "CYP2C19", allele: "CYP2C19*2", consequence: "loss-of-function splice variant", phenotype: "reduced CYP2C19 activity (poor metabolizer when homozygous)", drugs: &["clopidogrel", "omeprazole"] },
    PgxAlleleRule { chrom: "chr10", position: 96_540_410, reference: "G", alternate: "A", rsid: "rs4986893", gene: "CYP2C19", allele: "CYP2C19*3", consequence: "loss-of-function stop-gain variant", phenotype: "reduced CYP2C19 activity (poor metabolizer when homozygous)", drugs: &["clopidogrel", "omeprazole"] },
    PgxAlleleRule { chrom: "chr10", position: 96_521_657, reference: "C", alternate: "T", rsid: "rs12248560", gene: "CYP2C19", allele: "CYP2C19*17", consequence: "increased-expression promoter variant", phenotype: "increased CYP2C19 activity (rapid/ultrarapid metabolizer)", drugs: &["clopidogrel"] },
    PgxAlleleRule { chrom: "chr22", position: 42_130_641, reference: "G", alternate: "A", rsid: "rs3892097", gene: "CYP2D6", allele: "CYP2D6*4", consequence: "loss-of-function splice variant", phenotype: "reduced CYP2D6 activity (poor metabolizer when homozygous)", drugs: &["codeine", "tramadol", "tamoxifen"] },
    PgxAlleleRule { chrom: "chr22", position: 42_127_943, reference: "C", alternate: "T", rsid: "rs1065852", gene: "CYP2D6", allele: "CYP2D6*10", consequence: "decreased-function missense variant", phenotype: "reduced CYP2D6 activity (intermediate metabolizer)", drugs: &["codeine", "tamoxifen"] },
    PgxAlleleRule { chrom: "chr12", position: 21_176_826, reference: "T", alternate: "C", rsid: "rs4149056", gene: "SLCO1B1", allele: "SLCO1B1*5", consequence: "decreased-function missense variant", phenotype: "reduced OATP1B1 transport; increased simvastatin myopathy risk", drugs: &["simvastatin"] },
    PgxAlleleRule { chrom: "chr16", position: 31_096_446, reference: "G", alternate: "A", rsid: "rs9923231", gene: "VKORC1", allele: "VKORC1 -1639G>A", consequence: "decreased-expression promoter variant", phenotype: "increased warfarin sensitivity", drugs: &["warfarin"] },
    PgxAlleleRule { chrom: "chr6", position: 18_143_954, reference: "G", alternate: "C", rsid: "rs1800462", gene: "TPMT", allele: "TPMT*2", consequence: "loss-of-function missense variant", phenotype: "reduced TPMT activity; increased thiopurine toxicity risk", drugs: &["azathioprine", "mercaptopurine", "thioguanine"] },
    PgxAlleleRule { chrom: "chr6", position: 18_139_228, reference: "G", alternate: "A", rsid: "rs1800460", gene: "TPMT", allele: "TPMT*3B", consequence: "loss-of-function missense variant", phenotype: "reduced TPMT activity; increased thiopurine toxicity risk", drugs: &["azathioprine", "mercaptopurine", "thioguanine"] },
    PgxAlleleRule { chrom: "chr6", position: 18_130_718, reference: "A", alternate: "G", rsid: "rs1142345", gene: "TPMT", allele: "TPMT*3C", consequence: "loss-of-function missense variant", phenotype: "reduced TPMT activity; increased thiopurine toxicity risk", drugs: &["azathioprine", "mercaptopurine", "thioguanine"] },
    PgxAlleleRule { chrom: "chr1", position: 97_515_789, reference: "C", alternate: "T", rsid: "rs3918290", gene: "DPYD", allele: "DPYD*2A", consequence: "loss-of-function splice variant", phenotype: "reduced DPD activity; increased fluoropyrimidine toxicity risk", drugs: &["fluorouracil", "capecitabine"] },
    PgxAlleleRule { chrom: "chr6", position: 31_353_884, reference: "G", alternate: "A", rsid: "rs2395029", gene: "HLA-B", allele: "HLA-B*57:01 tag", consequence: "tag variant in strong linkage with HLA-B*57:01", phenotype: "increased abacavir hypersensitivity risk", drugs: &["abacavir"] },
];

/// A VCF variant that matched a PGx allele rule.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PgxVariantMatch {
    pub chrom: String,
    pub position: u64,
    pub reference: String,
    pub alternate: String,
    pub rsid: Option<String>,
    pub gene: String,
    pub allele: String,
    pub consequence: String,
    pub phenotype: String,
    pub drugs: Vec<String>,
    pub genotype: Option<String>,
}

/// Research-use-only pharmacogenomic interpretation summary.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct PharmacogenomicsResult {
    pub reference_build: String,
    pub record_count: u64,
    pub matched_variant_count: u64,
    pub allele_count: usize,
    pub genes_affected: Vec<String>,
    pub variants: Vec<PgxVariantMatch>,
    pub combined_phenotypes: BTreeMap<String, String>,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum PgxError {
    Io(io::Error),
    ReadLine { line: usize, source: io::Error },
    MissingHeader,
    MalformedRecord { line: usize, message: String },
}

impl Display for PgxError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "failed to read VCF: {error}"),
            Self::ReadLine { line, source } => {
                write!(formatter, "failed to read VCF at line {line}: {source}")
            }
            Self::MissingHeader => formatter.write_str("VCF column header is missing"),
            Self::MalformedRecord { line, message } => {
                write!(formatter, "malformed VCF record at line {line}: {message}")
            }
        }
    }
}

impl Error for PgxError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::ReadLine { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<io::Error> for PgxError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Interpret PGx alleles in a (optionally gzip-compressed) VCF.
pub fn pharmacogenomics_path(path: impl AsRef<Path>) -> Result<PharmacogenomicsResult, PgxError> {
    let path = path.as_ref();
    let mut magic = [0_u8; 2];
    let magic_length = File::open(path)?.read(&mut magic)?;
    let input: Box<dyn Read> = if magic_length == magic.len() && magic == [0x1f, 0x8b] {
        Box::new(MultiGzDecoder::new(File::open(path)?))
    } else {
        Box::new(File::open(path)?)
    };

    pharmacogenomics(BufReader::new(input))
}

fn pharmacogenomics(mut reader: impl BufRead) -> Result<PharmacogenomicsResult, PgxError> {
    let mut result = PharmacogenomicsResult {
        reference_build: PGX_REFERENCE_BUILD.to_owned(),
        ..PharmacogenomicsResult::default()
    };
    let mut saw_header = false;
    let mut sample_columns = 0_usize;
    let mut line_number = 0_usize;
    let mut buffer = String::new();

    loop {
        line_number += 1;
        buffer.clear();
        let bytes_read = reader
            .read_line(&mut buffer)
            .map_err(|source| PgxError::ReadLine {
                line: line_number,
                source,
            })?;
        if bytes_read == 0 {
            break;
        }
        let line = buffer.trim_end_matches(['\r', '\n']);
        if line.starts_with("##") {
            continue;
        }
        if line.starts_with("#CHROM") {
            let columns = line.split('\t').count();
            sample_columns = columns.saturating_sub(9);
            saw_header = true;
            continue;
        }
        if line.is_empty() {
            continue;
        }
        if !saw_header {
            return Err(PgxError::MissingHeader);
        }
        match_pgx_record(&mut result, line, sample_columns, line_number)?;
    }

    if !saw_header {
        return Err(PgxError::MissingHeader);
    }
    if result.record_count == 0 {
        result
            .warnings
            .push("VCF contains no variant records".to_owned());
    }
    if result.matched_variant_count == 0 {
        result
            .warnings
            .push("no pharmacogenomic alleles from the built-in table were found".to_owned());
    }
    result.genes_affected = {
        let mut genes: Vec<String> = result
            .variants
            .iter()
            .map(|variant| variant.gene.clone())
            .collect();
        genes.sort();
        genes.dedup();
        genes
    };
    result.combined_phenotypes = combined_phenotypes(&result.variants);
    Ok(result)
}

fn match_pgx_record(
    result: &mut PharmacogenomicsResult,
    line: &str,
    sample_columns: usize,
    line_number: usize,
) -> Result<(), PgxError> {
    let mut columns = line.split('\t');
    let chrom = columns
        .next()
        .ok_or_else(|| malformed(line_number, "missing CHROM"))?;
    let position = columns
        .next()
        .ok_or_else(|| malformed(line_number, "missing POS"))?
        .parse::<u64>()
        .map_err(|_| malformed(line_number, "POS is not an integer"))?;
    let id = columns
        .next()
        .ok_or_else(|| malformed(line_number, "missing ID"))?;
    let reference = columns
        .next()
        .ok_or_else(|| malformed(line_number, "missing REF"))?;
    let alternates = columns
        .next()
        .ok_or_else(|| malformed(line_number, "missing ALT"))?;
    let sample_fields: Vec<&str> = columns.collect();
    let rsid = if id == "." || id.is_empty() {
        None
    } else {
        Some(id.to_owned())
    };
    result.record_count += 1;

    for (index, alternate) in alternates.split(',').enumerate() {
        let allele_index = index + 1;
        for allele in PGX_ALLELES {
            if allele.chrom == chrom
                && allele.position == position
                && allele.reference == reference
                && allele.alternate == alternate
            {
                let genotype = genotype_of(sample_columns, &sample_fields, allele_index);
                if genotype.as_deref() == Some("ref") {
                    continue;
                }
                result.variants.push(PgxVariantMatch {
                    chrom: chrom.to_owned(),
                    position,
                    reference: reference.to_owned(),
                    alternate: alternate.to_owned(),
                    rsid: rsid.clone(),
                    gene: allele.gene.to_owned(),
                    allele: allele.allele.to_owned(),
                    consequence: allele.consequence.to_owned(),
                    phenotype: allele.phenotype.to_owned(),
                    drugs: allele.drugs.iter().map(|drug| (*drug).to_owned()).collect(),
                    genotype,
                });
                result.matched_variant_count += 1;
                result.allele_count += 1;
            }
        }
    }
    Ok(())
}

fn genotype_of(
    sample_columns: usize,
    sample_fields: &[&str],
    allele_index: usize,
) -> Option<String> {
    if sample_columns == 0 || sample_fields.len() < 5 {
        return None;
    }
    // sample_fields[0..] are QUAL, FILTER, INFO, FORMAT, then sample columns.
    if sample_fields[3].split(':').next()? != "GT" {
        return None;
    }
    let sample = sample_fields[4];
    let genotype = sample.split(':').next()?;
    let alleles: Vec<&str> = genotype.split(['/', '|']).collect();
    let allele_index = allele_index.to_string();
    let count = alleles
        .iter()
        .filter(|allele| **allele == allele_index)
        .count();
    Some(
        match count {
            2 => "hom-alt",
            1 => "het-alt",
            _ => "ref",
        }
        .to_owned(),
    )
}

/// Infer a combined diplotype phenotype for genes with multiple matched
/// alleles (CYP2C19 and CYP2D6); other genes keep allele-level phenotypes.
fn combined_phenotypes(variants: &[PgxVariantMatch]) -> BTreeMap<String, String> {
    let mut phenotypes = BTreeMap::new();
    for gene in ["CYP2C19", "CYP2D6"] {
        let alleles: Vec<&str> = variants
            .iter()
            .filter(|variant| variant.gene == gene)
            .map(|variant| variant.allele.as_str())
            .collect();
        if alleles.is_empty() {
            continue;
        }
        let mut sorted = alleles.clone();
        sorted.sort_unstable();
        let key = sorted.join("+");
        let phenotype = match (gene, sorted.as_slice()) {
            ("CYP2C19", ["CYP2C19*17", "CYP2C19*17"]) => {
                "CYP2C19 ultrarapid metabolizer".to_owned()
            }
            ("CYP2C19", ["CYP2C19*17", _]) => "CYP2C19 rapid metabolizer".to_owned(),
            ("CYP2C19", ["CYP2C19*2", "CYP2C19*2"])
            | ("CYP2C19", ["CYP2C19*2", "CYP2C19*3"])
            | ("CYP2C19", ["CYP2C19*3", "CYP2C19*3"]) => "CYP2C19 poor metabolizer".to_owned(),
            ("CYP2C19", _) => "CYP2C19 intermediate metabolizer".to_owned(),
            ("CYP2D6", ["CYP2D6*4", "CYP2D6*4"]) => "CYP2D6 poor metabolizer".to_owned(),
            ("CYP2D6", _) => "CYP2D6 intermediate metabolizer".to_owned(),
            _ => continue,
        };
        phenotypes.insert(format!("{gene} ({key})"), phenotype);
    }
    phenotypes
}

fn malformed(line: usize, message: impl Into<String>) -> PgxError {
    PgxError::MalformedRecord {
        line,
        message: message.into(),
    }
}

/// Render matched variants as a TSV interpretation table.
pub fn render_pgx_table(result: &PharmacogenomicsResult) -> String {
    let mut table = String::from(
        "chrom\tposition\treference\talternate\trsid\tgene\tallele\tconsequence\tphenotype\tdrugs\tgenotype\n",
    );
    for variant in &result.variants {
        table.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            variant.chrom,
            variant.position,
            variant.reference,
            variant.alternate,
            variant.rsid.as_deref().unwrap_or("."),
            variant.gene,
            variant.allele,
            variant.consequence,
            variant.phenotype,
            variant.drugs.join(";"),
            variant.genotype.as_deref().unwrap_or("."),
        ));
    }
    table
}

#[cfg(test)]
mod tests {
    use super::{PGX_ALLELES, pharmacogenomics, render_pgx_table};

    #[test]
    fn matches_pgx_alleles_and_infers_combined_phenotypes() {
        let vcf = "\
##fileformat=VCFv4.2
#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\tFORMAT\tsample1
chr10\t96541605\trs4244285\tG\tA\t.\tPASS\t.\tGT\t1/1
chr10\t96540410\trs4986893\tG\tA\t.\tPASS\t.\tGT\t0/1
chr22\t42130641\trs3892097\tG\tA\t.\tPASS\t.\tGT\t0/0
chr12\t21176826\trs4149056\tT\tC\t.\tPASS\t.\tGT\t1/1
";
        let result = pharmacogenomics(vcf.as_bytes()).expect("parse PGx VCF");
        assert_eq!(result.record_count, 4);
        assert_eq!(
            result.matched_variant_count, 3,
            "homozygous-reference records must not count as allele matches"
        );
        assert_eq!(result.allele_count, 3);
        assert_eq!(result.genes_affected, vec!["CYP2C19", "SLCO1B1"]);
        assert_eq!(result.variants[0].genotype.as_deref(), Some("hom-alt"));
        assert_eq!(result.variants[1].genotype.as_deref(), Some("het-alt"));
        assert!(
            result
                .variants
                .iter()
                .all(|variant| variant.gene != "CYP2D6"),
            "CYP2D6 record with 0/0 genotype must be excluded"
        );
        let combined = &result.combined_phenotypes;
        assert_eq!(
            combined
                .get("CYP2C19 (CYP2C19*2+CYP2C19*3)")
                .map(String::as_str),
            Some("CYP2C19 poor metabolizer")
        );
        assert!(
            combined.get("SLCO1B1").is_none(),
            "non-diplotype genes keep allele-level phenotypes"
        );
    }

    #[test]
    fn renders_a_tabular_interpretation_table() {
        let vcf = "\
##fileformat=VCFv4.2
#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\tFORMAT\tsample1
chr10\t96521657\trs12248560\tC\tT\t.\tPASS\t.\tGT\t1/1
";
        let result = pharmacogenomics(vcf.as_bytes()).expect("parse PGx VCF");
        let table = render_pgx_table(&result);
        assert!(table.starts_with("chrom\tposition\treference\talternate\trsid"));
        assert!(table.contains("CYP2C19*17"));
        assert!(table.contains("clopidogrel"));
    }

    #[test]
    fn rule_table_positions_are_nonzero_and_unique_per_gene_allele() {
        use std::collections::HashSet;
        let mut seen = HashSet::new();
        for allele in PGX_ALLELES {
            assert!(allele.position > 0);
            assert!(seen.insert((
                allele.chrom,
                allele.position,
                allele.reference,
                allele.alternate
            )));
        }
    }
}
