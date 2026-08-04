use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    if matches!(arguments.first().and_then(|value| value.to_str()), Some("stats" | "coverage")) {
        println!("# linxira native-tool test report\nSN\traw total sequences:\t2");
        return ExitCode::SUCCESS;
    }
    if let Some(prefix) = value_after(&arguments, "-pre") {
        let output = PathBuf::from(format!("{}.treefile", prefix.to_string_lossy()));
        return write_output(&output, "(one:0.1,two:0.1);\n");
    }
    if let Some(directory) = value_after(&arguments, "-oc") {
        let directory = PathBuf::from(directory);
        if let Err(error) = fs::create_dir_all(&directory) {
            eprintln!("failed to create {}: {error}", directory.display());
            return ExitCode::from(3);
        }
        return write_output(&directory.join("meme.txt"), "MEME version 5\n\nALPHABET= ACGT\n");
    }
    if arguments.len() == 1 {
        let dataset = PathBuf::from(&arguments[0]);
        let gff = PathBuf::from(format!("{}.gff", dataset.to_string_lossy()));
        let blast = PathBuf::from(format!("{}.blast", dataset.to_string_lossy()));
        if gff.is_file() && blast.is_file() {
            let output = PathBuf::from(format!("{}.collinearity", dataset.to_string_lossy()));
            return write_output(
                &output,
                "## Alignment 0: score=100 e_value=1e-20 N=2 chr1&chrA plus\n  0-  0: gene1 geneA 1e-20\n  0-  1: gene2 geneB 1e-18\n",
            );
        }
    }
    let output = ["--domtblout", "-output", "--out", "-out", "-o", "--db", "--outFileName"]
        .into_iter()
        .find_map(|flag| value_after(&arguments, flag).map(PathBuf::from));
    let Some(output) = output else {
        eprintln!("stub did not receive a supported output flag");
        return ExitCode::from(2);
    };
    if let Some(method) = value_after(&arguments, "-m") {
        return write_output(
            &output,
            &format!(
                "Sequence\tMethod\tKa\tKs\tKa/Ks\nGene1&GeneA\t{}\t0.010\t0.100\t0.100\n",
                method.to_string_lossy()
            ),
        );
    }
    let content = if arguments.iter().any(|value| value == "-output") {
        ">one\nACGTNN\n>two\nGGGG\n>three\nAT\n"
    } else if arguments.iter().any(|value| value == "--domtblout") {
        "# target name accession tlen query name accession qlen E-value score bias\n"
    } else if arguments.iter().any(|value| value == "makedb")
        || arguments.iter().any(|value| value == "-dbtype")
    {
        "stub-database\n"
    } else {
        "query\treference\t100.0\t4\t0\t0\t1\t4\t1\t4\t1e-20\t50.0\n"
    };
    write_output(&output, content)
}

fn value_after<'a>(arguments: &'a [std::ffi::OsString], flag: &str) -> Option<&'a std::ffi::OsString> {
    arguments
        .iter()
        .position(|value| value == flag)
        .and_then(|index| arguments.get(index + 1))
}

fn write_output(output: &PathBuf, content: &str) -> ExitCode {
    match fs::write(output, content) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("failed to write {}: {error}", output.display());
            ExitCode::from(3)
        }
    }
}
