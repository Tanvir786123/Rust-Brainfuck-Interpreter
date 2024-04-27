//! Command Line Interface for the Brainfuck Interpreter
//!
//! Handles parsing and providing access to the command-line arguments passed to the interpreter.
//! Uses `clap` for argument parsing.

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about = "Brainfuck Interpreter", long_about = None)]
pub struct CLIArgs {
    /// Path to the Brainfuck program file
    #[arg(name = "PROGRAM")]
    pub program_path: PathBuf,

    /// Number of cells in the interpreter's tape.
    #[arg(short = 'c', long = "cells", default_value_t = 30000)]
    pub tape_size: usize,

    /// Flag to enable a dynamically growing tape.
    #[arg(short = 'e', long = "extensible")]
    pub extensible: bool,
}
