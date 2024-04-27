//! Utilities for working with Brainfuck programs.
//!
//! This includes representing Brainfuck instructions
//! and programs, and functions for parsing Brainfuck code from strings or files.

use std::collections::HashMap;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Defines errors for the Brainfuck Virtual Machine (VM).
///
/// This enum includes runtime errors encountered by the VM during Brainfuck program execution.
#[derive(Error, Debug)]
pub enum VMError {
    /// Error for invalid head position on the tape.
    ///
    /// Occurs when the head moves outside the tape bounds, with details about the erroneous position
    /// and the source code location causing the error (line and column).
    #[error(
        "Invalid head position: {position}, caused by instruction at line {line} column {column}"
    )]
    InvalidHeadPosition {
        position: usize,
        line: usize,
        column: usize,
    },

    /// I/O error during program execution.
    ///
    /// Occurs when an [`io::Error`] operation fails, caused by a specific
    /// Brainfuck [`Instruction`].
    #[error("I/O error: {source}, caused by: {instruction}")]
    IOError {
        source: io::Error,
        instruction: Instruction,
    },

    /// Error for unbalanced or unmatched brackets.
    ///
    /// Occurs when a loop start `[` or end `]` bracket does not have a matching counterpart.
    /// This includes details about the position of the unmatched bracket and its type (start or end).
    #[error("Unmatched bracket: type {bracket_type} at line {line} column {column}")]
    UnmatchedBracket {
        bracket_type: String,
        line: usize,
        column: usize,
    },
}

/// Custom error type for Brainfuck program processing.
///
/// This enum represents the various kinds of errors that can occur
/// while working with Brainfuck programs, such as parsing errors or
/// unbalanced brackets in the program's code.
#[derive(Error, Debug, PartialEq)]
pub enum BFError {
    #[error("Unbalanced closing bracket at position {0}")]
    UnbalancedClosingBracket(usize),

    #[error("Unbalanced opening bracket at position {0}")]
    UnbalancedOpeningBracket(usize),
}

/// Raw Brainfuck instruction
///
/// Each variant corresponds to a particular Brainfuck instruction.
#[derive(Debug, PartialEq, Clone)]
pub enum Instruction {
    /// Represents `>`
    Right,
    /// Represents `<`
    Left,
    /// Represents `+`
    Increment,
    /// Represents `-`
    Decrement,
    /// Represents `.`
    Output,
    /// Represents `,`
    Input,
    /// Represents `[`
    LoopStart,
    /// Represents `]`
    LoopEnd,
}

impl Instruction {
    /// Converts a character to the corresponding Brainfuck instruction, if valid.
    pub fn from_char(ch: char) -> Option<Instruction> {
        match ch {
            '>' => Some(Instruction::Right),
            '<' => Some(Instruction::Left),
            '+' => Some(Instruction::Increment),
            '-' => Some(Instruction::Decrement),
            '.' => Some(Instruction::Output),
            ',' => Some(Instruction::Input),
            '[' => Some(Instruction::LoopStart),
            ']' => Some(Instruction::LoopEnd),
            _ => None, // None for characters that are not Brainfuck instructions
        }
    }

    /// Returns the character representation of the instruction.
    pub fn symbol(&self) -> char {
        match self {
            Instruction::Right => '>',
            Instruction::Left => '<',
            Instruction::Increment => '+',
            Instruction::Decrement => '-',
            Instruction::Output => '.',
            Instruction::Input => ',',
            Instruction::LoopStart => '[',
            Instruction::LoopEnd => ']',
        }
    }
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let symbol = match self {
            Instruction::Right => '>',
            Instruction::Left => '<',
            Instruction::Increment => '+',
            Instruction::Decrement => '-',
            Instruction::Output => '.',
            Instruction::Input => ',',
            Instruction::LoopStart => '[',
            Instruction::LoopEnd => ']',
        };
        write!(f, "{}", symbol)
    }
}

/// An [`Instruction`] with line and column information.
#[derive(Debug)]
pub struct BFInstruction {
    instruction: Instruction,
    line_number: usize,
    char_column: usize,
}

impl BFInstruction {
    /// Returns a reference to the [`Instruction`].
    pub fn instruction(&self) -> &Instruction {
        &self.instruction
    }

    /// Returns a reference to the line number where this instruction appears in the source code.
    pub fn line_number(&self) -> &usize {
        &self.line_number
    }

    /// Returns a reference to the character column where this instruction appears in the source code.
    pub fn char_column(&self) -> &usize {
        &self.char_column
    }
}

/// A Brainfuck Program.
///
/// Each program stores the file it was loaded from as well as
/// a sequence of instructions along with their provenance.
#[derive(Debug)]
pub struct BFProgram {
    filename: PathBuf,
    instructions: Vec<BFInstruction>,
    bracket_pairs: HashMap<usize, usize>,
}

impl fmt::Display for BFProgram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Write the filename to the formatter
        write!(f, "{}", self.filename.display())
    }
}

impl BFProgram {
    /// Construct a new program with the given filename and content.
    ///
    /// This method processes the provided string representing the Brainfuck program's content,
    /// creating a new `BFProgram` instance. It parses the content into instructions, along with their
    /// line and column information. The `bracket_pairs` field, which will later store the indices of matching
    /// brackets for fast lookup, is initialised as empty and populated by the `check_brackets` method.
    ///
    /// # Arguments
    /// * `filename`: A path-like object representing the filename of the Brainfuck program.
    /// * `content`: A string slice representing the content of the Brainfuck program.
    ///
    /// # Returns
    /// Returns a new instance of `BFProgram` with parsed instructions and an initialised (but empty)
    /// `bracket_pairs` field.
    ///
    /// # Examples
    /// ```
    /// # use bft_types::{Instruction, BFProgram};
    /// # use std::path::Path;
    /// // Creating a new BFProgram instance
    /// let mut program = BFProgram::new("example.bf", ">+<-");
    /// assert_eq!(program.filename(), Path::new("example.bf"));
    /// assert!(matches!(program.instructions()[0].instruction(), Instruction::Right));
    ///
    /// // Checking brackets and using bracket pairs length getter
    /// // This step assumes the `check_brackets` method is called as in the `bft` crate
    /// program.check_brackets().unwrap();
    /// assert_eq!(program.bracket_pairs_len(), 0); // Check the number of bracket pairs
    ///
    /// // Demonstrating 'get_matching_bracket' with a program containing brackets
    /// // Reinitialize the program with brackets for demonstration
    /// let mut program_with_brackets = BFProgram::new("example_with_brackets.bf", "[>+<-]");
    /// program_with_brackets.check_brackets().unwrap();
    /// assert_eq!(program_with_brackets.get_matching_bracket(0), Some(5));
    /// assert_eq!(program_with_brackets.get_matching_bracket(5), Some(0));
    /// ```
    pub fn new<P: AsRef<Path>, S: AsRef<str>>(filename: P, content: S) -> BFProgram {
        let mut instructions = Vec::new();
        let mut line_number = 1;

        for line in content.as_ref().lines() {
            for (char_column, ch) in line.chars().enumerate() {
                let char_column = char_column + 1; // enumerate() starts counting from 0

                if let Some(instr) = Instruction::from_char(ch) {
                    instructions.push(BFInstruction {
                        instruction: instr,
                        line_number,
                        char_column,
                    });
                }
            }

            line_number += 1; // Increment line number after each line
        }

        BFProgram {
            filename: filename.as_ref().to_owned(),
            instructions,
            bracket_pairs: HashMap::new(),
        }
    }

    /// Load a program from file.
    ///
    /// * `path` - A path-like object representing the file for reading.
    ///
    /// # Errors
    ///
    /// Returns an `io::Result` with an error if the file can't opened or read.
    ///
    /// # Examples
    /// Read the given file from disk and construct a `BFProgram`.
    /// ```no_run
    /// # use bft_types::BFProgram;
    /// let program = BFProgram::from_file("example.bf").expect("Failed to load program");
    /// ```
    pub fn from_file<P: AsRef<Path>>(path: P) -> io::Result<BFProgram> {
        let contents = std::fs::read_to_string(&path)?;

        Ok(BFProgram::new(path, contents))
    }

    /// The filename from which the program was loaded
    pub fn filename(&self) -> &Path {
        &self.filename
    }

    /// The parsed instructions in the program
    pub fn instructions(&self) -> &[BFInstruction] {
        &self.instructions
    }

    /// Returns the number of bracket pairs stored in the program.
    pub fn bracket_pairs_len(&self) -> usize {
        self.bracket_pairs.len()
    }

    /// Returns the index of the matching bracket for a given index, if it exists.
    pub fn get_matching_bracket(&self, index: usize) -> Option<usize> {
        self.bracket_pairs.get(&index).copied()
    }

    /// Checks if the square brackets in the program are balanced and populates the `bracket_pairs` map.
    ///
    /// This method scans the program's instructions to find matching bracket pairs. If the brackets
    /// are balanced, it fills the `bracket_pairs` map with the indices of corresponding opening and
    /// closing brackets. The map is used for quick lookup of bracket pairs during program execution.
    ///
    /// # Returns
    /// - `Ok(())`: If all square brackets in the program are properly balanced.
    ///
    /// # Errors
    /// - [`BFError::UnbalancedClosingBracket`]: If an unbalanced closing bracket `']'` is found.
    /// - [`BFError::UnbalancedOpeningBracket`]: If an unbalanced opening bracket `'['` is found.
    ///
    /// # Examples
    /// ```
    /// # use bft_types::{BFProgram, BFError};
    /// let mut program = BFProgram::new("balanced.bf", "[+[-]+]");
    /// assert!(program.check_brackets().is_ok());
    ///
    /// let mut program = BFProgram::new("unbalanced.bf", "[+");
    /// assert!(matches!(program.check_brackets(), Err(BFError::UnbalancedOpeningBracket(_))));
    ///
    /// let mut program = BFProgram::new("unbalanced.bf", "+]");
    /// assert!(matches!(program.check_brackets(), Err(BFError::UnbalancedClosingBracket(_))));
    /// ```
    pub fn check_brackets(&mut self) -> Result<(), BFError> {
        let mut stack = Vec::new();

        for (index, instruction) in self.instructions.iter().enumerate() {
            match instruction.instruction {
                Instruction::LoopStart => stack.push(index),
                Instruction::LoopEnd => match stack.pop() {
                    Some(open_index) => {
                        self.bracket_pairs.insert(open_index, index);
                        self.bracket_pairs.insert(index, open_index);
                    }
                    None => return Err(BFError::UnbalancedClosingBracket(index)),
                },
                _ => {}
            }
        }

        if let Some(index) = stack.pop() {
            return Err(BFError::UnbalancedOpeningBracket(index));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    #[rstest]
    #[case('>', Instruction::Right)]
    #[case('<', Instruction::Left)]
    #[case(']', Instruction::LoopEnd)]
    // ... Similarly for the other 5 chars
    fn test_parse_instructions(
        #[case] input_char: char,
        #[case] expected_instruction: Instruction,
    ) {
        let program = BFProgram::new("", &input_char.to_string());
        assert_eq!(program.instructions.len(), 1);
        assert_eq!(program.instructions[0].instruction, expected_instruction);
        assert_eq!(program.instructions[0].line_number, 1);
        assert_eq!(program.instructions[0].char_column, 1);
    }

    #[rstest]
    #[case("++[--++]", Ok(()))]
    #[case("++[--", Err(BFError::UnbalancedOpeningBracket(2)))]
    #[case("++--]", Err(BFError::UnbalancedClosingBracket(4)))]
    #[case("[++[--]++]", Ok(()))]
    #[case("[[]]", Ok(()))]
    #[case("[]][", Err(BFError::UnbalancedClosingBracket(2)))]
    fn test_bracket_balancing(#[case] input: &str, #[case] expected: Result<(), BFError>) {
        let mut program = BFProgram::new("", input);
        assert_eq!(program.check_brackets(), expected);
    }
}
