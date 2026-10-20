mod app;
mod content;
mod gameplay;
mod items;
mod progression;
mod scripting;
mod stats;
mod status_effects;
#[cfg(test)]
mod tests;

use std::process::ExitCode;

fn main() -> ExitCode {
    match app::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
