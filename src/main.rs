//! bft is a binary crate that serves as the entry point for the Brainfuck interpreter.
//! It handles command-line input to load and execute Brainfuck programs using the [`bft_types`]
//! and [`bft_interp`] crates.

mod cli;
use bft_interp::BFInterpreter;
use bft_types::BFProgram;
use clap::Parser;
use cli::CLIArgs;
use std::error::Error;
use std::io::{self, Write};
use std::num::NonZeroUsize;

/// Optional: Type alias for the Result type used in main
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// A writer that appends a newline to the output if it doesn't end with one.
pub struct NewlineEnsuringWriter<W: Write> {
    inner: W,
    last_byte_was_newline: bool,
}

impl<W: Write> NewlineEnsuringWriter<W> {
    /// Constructs a new `NewlineEnsuringWriter` wrapping an existing writer.
    pub fn new(writer: W) -> Self {
        NewlineEnsuringWriter {
            inner: writer,
            last_byte_was_newline: false,
        }
    }
}

impl<W: Write> Write for NewlineEnsuringWriter<W> {
    /// Implements [`Write`] trait, tracking the last byte to ensure newline at end.
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write_all(buf)?;
        self.last_byte_was_newline = buf.ends_with(&[b'\n']);
        Ok(buf.len()) // Return the length of the buffer since all bytes were written
    }

    /// Flushes the inner writer.
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<W: Write> Drop for NewlineEnsuringWriter<W> {
    /// Ensures appending a newline character on drop if the last byte wasn't a newline.
    fn drop(&mut self) {
        if !self.last_byte_was_newline {
            let _ = self.inner.write_all(b"\n");
            let _ = self.inner.flush();
        }
    }
}

/// Executes the Brainfuck interpreter with the provided CLI arguments.
///
/// This function takes the parsed CLI arguments, loads and checks the Brainfuck program,
/// and then runs the interpreter using standard input for reading and [`NewlineEnsuringWriter`]
/// wrapped standard output for writing, ensuring the output is neatly formatted.
///
/// # Arguments
///
/// * `args` - The parsed command-line arguments.
///
/// # Returns
///
/// This function returns a `Result` that is `Ok(())` if execution succeeds, or an `Error` if it fails.
fn run_bft(args: &CLIArgs) -> Result<()> {
    // Load the Brainfuck program
    let mut bf_program = BFProgram::from_file(&args.program_path)?;

    // Check for balanced brackets
    bf_program
        .check_brackets()
        .map_err(|e| format!("Bracket Error: {}", e))?;

    // Construct the virtual machine
    let tape_size = NonZeroUsize::new(args.tape_size).expect("Tape size must not be zero");
    let mut vm = BFInterpreter::<u8>::new(&bf_program, tape_size, args.extensible);

    // Use standard input for the program's IO
    let stdin = io::stdin();
    let mut input_handle = stdin.lock();

    // Use standard output, wrapped in NewlineEnsuringWriter
    let stdout = io::stdout();
    let output_handle = stdout.lock();
    let mut newline_ensuring_output = NewlineEnsuringWriter::new(output_handle);

    // Interpret and execute the program
    vm.interpret(&mut input_handle, &mut newline_ensuring_output)
        .map_err(|e| format!("Runtime Error: {}", e))?;

    Ok(())
}

/// The main entry point for the Brainfuck interpreter.
///
/// This function parses the command-line arguments and then calls `run_bft` to execute the program.
/// If an error occurs, it prints a formatted error message and exits with a failure code.
fn main() {
    let args = CLIArgs::parse();
    if let Err(e) = run_bft(&args) {
        eprintln!("bft: Error: {}", e);
        std::process::exit(1);
    }
}
