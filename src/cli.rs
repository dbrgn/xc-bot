//! Ultra-simple CLI argument parsing.
//!
//! The CLI only supports passing a configfile path. It also prints usage text
//! with --help or if invalid arguments are passed in.

use std::path::PathBuf;

pub struct App<'a> {
    name: &'a str,
    version: &'a str,
    description: &'a str,
    author: &'a str,
    default_config_path: &'a str,
}

impl<'a> App<'a> {
    pub fn new(name: &'a str, version: &'a str, description: &'a str, author: &'a str, default_config_path: &'a str) -> Self {
        Self {
            name,
            version,
            description,
            author,
            default_config_path,
        }
    }

    fn print_help(&self) {
        eprintln!("{} {}", self.name, self.version);
        eprintln!("\n{}", self.description);
        eprintln!("Author: {}", self.author);
        eprintln!("\nUsage:");
        eprintln!("  -c, --config <PATH>  Path to config file (default: '{}')", self.default_config_path);
        eprintln!("  -v, --version        Return the version");
        eprintln!("  -h, --help           Print this information");
    }

    pub fn get_configfile(self) -> PathBuf {
        let args: Vec<String> = std::env::args().collect();

        // Handle -h / --help
        if args.iter().any(|arg| arg == "-h" || arg == "--help") {
            self.print_help();
            std::process::exit(0);
        }

        // Handle -v / --version
        if args.iter().any(|arg| arg == "-v" || arg == "--version") {
            eprintln!("{} {}", self.name, self.version);
            std::process::exit(0);
        }

        // Parse other args
        match args.len() {
            1 => PathBuf::from(self.default_config_path),
            3 if args[1] == "-c" || args[1] == "--config" => PathBuf::from(&args[2]),
            _ => {
                self.print_help();
                std::process::exit(1);
            }
        }
    }
}
