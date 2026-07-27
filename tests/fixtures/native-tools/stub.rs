use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    let output = ["--domtblout", "-output", "--out", "-out", "--db"]
        .into_iter()
        .find_map(|flag| {
            arguments
                .iter()
                .position(|value| value == flag)
                .and_then(|index| arguments.get(index + 1))
                .map(PathBuf::from)
        });
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
    match fs::write(&output, content) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("failed to write {}: {error}", output.display());
            ExitCode::from(3)
        }
    }
}
