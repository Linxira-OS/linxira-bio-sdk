#![forbid(unsafe_code)]

use linxira_bio_worker::execute_path;
use std::env;
use std::error::Error;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(2)
        }
    }
}

fn run(arguments: Vec<String>) -> Result<(), Box<dyn Error + Send + Sync>> {
    let request_path = match arguments.as_slice() {
        [path] => Path::new(path),
        _ => return Err("usage: linxira-bio-worker <job-request.json>".into()),
    };

    println!("{}", execute_path(request_path)?);
    Ok(())
}
