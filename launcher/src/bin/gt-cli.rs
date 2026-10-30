//! GoatTech developer entry: the launcher's command line without the window. Same code as
//! `horizon-launcher <subcommand>`; links without the GUI platform libraries.
fn main() {
    std::process::exit(horizon_launcher::cli::run().unwrap_or(0));
}
