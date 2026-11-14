//! `feedrico-mcp --library <path>`: a read-only MCP server over stdio.

use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

use feedrico_mcp::{Server, open_library};

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let mut library: Option<PathBuf> = None;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--library") => library = args.next().map(PathBuf::from),
            Some("--help" | "-h") => {
                println!(
                    "feedrico-mcp --library <Library path>\n\n\
                     A read-only MCP server (stdio) for a Feedrico Library. See docs/mcp.md."
                );
                return ExitCode::SUCCESS;
            }
            _ => {
                eprintln!("feedrico-mcp: unexpected argument {arg:?} (usage: --library <path>)");
                return ExitCode::FAILURE;
            }
        }
    }
    let Some(library) = library else {
        eprintln!("feedrico-mcp: --library <path> is required");
        return ExitCode::FAILURE;
    };
    let server = match open_library(&library) {
        Ok(lib) => Server::new(lib),
        Err(message) => {
            eprintln!("feedrico-mcp: {message}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(e) = server.serve(io::stdin().lock(), io::stdout().lock()) {
        eprintln!("feedrico-mcp: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
