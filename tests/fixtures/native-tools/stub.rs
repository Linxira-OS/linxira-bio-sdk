use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
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
    let output = ["--domtblout", "-output", "--out", "-out", "-o", "--db"]
        .into_iter()
        .find_map(|flag| value_after(&arguments, flag).map(PathBuf::from));
    let Some(output) = output else {
        eprintln!("stub did not receive a supported output flag");
        return ExitCode::from(2);
    };
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
