#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
mod app;
mod args;
mod config;
mod jobs;
mod project;
mod report;

fn main() {
    match args::opts::parse().and_then(app::run) {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("vex: {error:#}");
            std::process::exit(2);
        }
    }
}
