//! petlauncher: an application launcher for GNOME. `docs/program.md` in the
//! repository describes the command line, the windows and the threads.

mod app;
mod backend;
mod cli;
mod desktop;
mod dictionary_window;
mod global_key;
mod keys;
mod log;
mod model;
mod notices;
mod rows;
mod selection;
mod settings_window;
mod stop;
mod visibility;
mod window;
mod worker;

use std::process::ExitCode;

use cli::Command;

fn main() -> ExitCode {
    let command = match cli::parse(std::env::args_os().skip(1)) {
        Ok(command) => command,
        Err(error) => {
            eprint!("{error}");
            return ExitCode::from(2);
        }
    };
    match command {
        Command::Version => {
            println!("petlauncher {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Command::Help => {
            print!("{}", cli::USAGE);
            ExitCode::SUCCESS
        }
        Command::ConvertShortcuts { force } => {
            match petlauncher_config::convert::convert_shortcuts(force) {
                Ok(summary) => {
                    print!("{summary}");
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("petlauncher: {error}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Start { .. } | Command::Toggle => {
            // `cli::parse` accepted the arguments, so they are UTF-8.
            let args: Vec<String> = std::env::args().collect();
            app::run(&args).into()
        }
    }
}
