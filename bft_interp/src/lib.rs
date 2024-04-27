//! This provides an interpreter for Brainfuck programs.
//!
//! It includes the [`BFInterpreter`] struct that simulates Brainfuck
//! environment with a customisable tape of cells.

use bft_types::BFProgram;
use bft_types::Instruction;
use bft_types::VMError;
use std::io::{self, Read, Write};
use std::num::NonZeroUsize;

/// Trait for Brainfuck interpreter cell types.
///
/// Imposes constraints such as requiring `Default` and supporting
/// in-place wrapping increment and decrement operations.
pub trait CellKind: Default + Copy {
    /// The associated type for the value stored in the cell.
    type Value;

    /// Performs an in-place wrapping increment operation on the cell value.
    ///
    /// If incrementing the cell value would cause overflow, it wraps around to the minimum value.
    fn increment(&mut self);

    /// Performs an in-place wrapping decrement operation on the cell value.
    ///
    /// If decrementing the cell value would cause underflow, it wraps around to the maximum value.
    fn decrement(&mut self);

    /// Converts the cell value to the associated type.
    fn to_value(&self) -> Self::Value;

    /// Converts a value of the associated type to the cell type.
    fn from_value(value: Self::Value) -> Self;
}

impl CellKind for u8 {
    type Value = u8;

    fn increment(&mut self) {
        *self = self.wrapping_add(1);
    }

    fn decrement(&mut self) {
        *self = self.wrapping_sub(1);
    }

    fn to_value(&self) -> u8 {
        *self
    }

    fn from_value(value: u8) -> Self {
        value
    }
}

/// A Brainfuck virtual machine (VM).
///
/// `BFInterpreter` simulates the Brainfuck execution environment. It maintains a tape of cells,
/// a head pointer for tape manipulation, a program counter for instruction tracking, and a reference
/// to the Brainfuck program being executed.
///
/// The tape's cell type (`T`) is constrained by the [`CellKind`] trait, which specifies operations
/// required for in-place wrapping increment and decrement. The tape can optionally be set to grow
/// dynamically, accommodating programs that require more memory than the initial allocation.
///
/// # Type Parameters
///
/// * `T` - The type of the cells on the tape. This type must implement `Default` and `Clone`.
/// * `'a` - The lifetime of the reference to the Brainfuck program.
///
/// # Fields
///
/// * `tape` - The memory tape used by the VM.
/// * `head` - The current position of the head on the tape.
/// * `can_grow` - Indicates whether the tape is allowed to grow dynamically.
/// * `program_counter` - Tracks the current execution point in the Brainfuck program.
/// * `program` - A reference to the Brainfuck program being executed.
///
/// # Examples
///
/// ```
/// # use bft_interp::BFInterpreter;
/// # use bft_types::BFProgram;
/// # use std::num::NonZeroUsize;
/// # let bf_program = BFProgram::new("example.bf", ">+<-");
/// let tape_size = NonZeroUsize::new(10_000).unwrap();
/// let interpreter = BFInterpreter::<u8>::new(&bf_program, tape_size, false);
/// ```
pub struct BFInterpreter<'a, T> {
    tape: Vec<T>,
    head: usize,
    can_grow: bool,
    program_counter: usize,
    program: &'a BFProgram,
}

impl<'a, T> BFInterpreter<'a, T>
where
    T: CellKind<Value = u8>, // Specify that T's associated type Value must be u8
{
    /// Interprets and executes the loaded Brainfuck program.
    ///
    /// This method processes each instruction in the program sequentially, handling input and output operations,
    /// and performing jumps for loop constructs. Execution continues until all instructions have been processed
    /// or an error occurs.
    ///
    /// # Arguments
    ///
    /// * `input` - A reader to provide input data for the program.
    /// * `output` - A writer to output data from the program.
    ///
    /// # Returns
    ///
    /// Returns [`Ok`] if the program executes successfully to completion.
    /// Returns [`VMError`] if an error occurs during execution, such as an invalid memory access or I/O error.
    ///
    /// # Example
    ///
    /// ```
    /// # use bft_interp::BFInterpreter;
    /// # use bft_types::BFProgram;
    /// # use std::io::{Cursor, Read, Write};
    /// # use std::num::NonZeroUsize;
    /// let program_str = "++++++++++[>+++++++>++++++++++>+++>+<<<<-]>++.>+.+++++++..+++.>++.<<+++++++++++++++.>.+++.------.--------.>+.>.";
    /// let mut bf_program = BFProgram::new("hello.bf", program_str);
    ///
    /// // Ensure brackets are balanced and populate the bracket_pairs hashmap
    /// bf_program.check_brackets().unwrap();
    ///
    /// let tape_size = NonZeroUsize::new(10_000).unwrap();
    /// let mut interpreter = BFInterpreter::<u8>::new(&bf_program, tape_size, false);
    ///
    /// let mut output_buffer = Cursor::new(Vec::new());
    ///
    /// interpreter.interpret(&mut Cursor::new(Vec::new()), &mut output_buffer).unwrap();
    ///
    /// let output = String::from_utf8(output_buffer.into_inner()).unwrap();
    /// assert_eq!(output, "Hello World!\n");
    /// ```
    pub fn interpret<R, W>(&mut self, mut input: R, mut output: W) -> Result<(), VMError>
    where
        R: Read,
        W: Write,
    {
        while self.program_counter < self.program.instructions().len() {
            let instruction = self.program.instructions()[self.program_counter].instruction();
            self.program_counter = match instruction {
                Instruction::Right => self.move_head_right()?,
                Instruction::Left => self.move_head_left()?,
                Instruction::Increment => self.increment_cell()?,
                Instruction::Decrement => self.decrement_cell()?,
                Instruction::Output => self.write_value(&mut output)?,
                Instruction::Input => self.read_value(&mut input)?,
                Instruction::LoopStart => self.jump_if_zero()?,
                Instruction::LoopEnd => self.jump_if_nonzero()?,
            };
        }
        Ok(())
    }

    /// Converts an `io::Error` into a `VMError` with the current instruction context.
    fn io_error_to_vm_error(&self, error: io::Error) -> VMError {
        VMError::IOError {
            source: error,
            instruction: self.program.instructions()[self.program_counter]
                .instruction()
                .clone(),
        }
    }

    /// Writes the value of the current cell to the specified `Write` implementation and returns the next instruction index.
    ///
    /// # Arguments
    /// * `writer` - The output writer to which the cell's value will be written.
    ///
    /// # Returns
    /// - `Ok(usize)`: The index of the next instruction to execute.
    /// - `Err(VMError)`: If an I/O error occurs during the write operation.
    ///
    /// # Errors
    /// This function returns an error if it encounters any issues with the output operation.
    pub fn write_value<W: Write>(&mut self, writer: &mut W) -> Result<usize, VMError> {
        writer
            .write_all(&[self.tape[self.head].to_value()])
            .map_err(|e| self.io_error_to_vm_error(e))?;
        Ok(self.program_counter + 1)
    }

    /// Reads a value from the specified `Read` implementation, sets the current cell's value, and returns the next instruction index.
    ///
    /// If a value is successfully read, the cell at the current head position is set to this value.
    /// If EOF is encountered (no more data to read), the cell is set to zero, which is the default behavior in Brainfuck.
    /// Any other error during the read operation results in a `VMError`.
    ///
    /// # Arguments
    /// * `reader` - The input reader from which the value will be read.
    ///
    /// # Returns
    /// - `Ok(usize)`: The index of the next instruction to execute.
    /// - `Err(VMError)`: If an I/O error occurs during the read operation.
    ///
    /// # Errors
    /// This function returns an error if it encounters issues with the input operation other than EOF.
    pub fn read_value<R: Read>(&mut self, reader: &mut R) -> Result<usize, VMError> {
        let mut buffer = [0; 1];
        match reader.read(&mut buffer) {
            Ok(0) => {
                // EOF reached; set the cell value to zero
                self.tape[self.head] = T::default();
            }
            Ok(_) => {
                // Successfully read a byte; set the cell value
                let value = buffer[0];
                self.tape[self.head] = T::from_value(value);
            }
            Err(e) => {
                // An error occurred during read
                return Err(self.io_error_to_vm_error(e));
            }
        }
        Ok(self.program_counter + 1)
    }

    /// Constructs a new `BFInterpreter` with the specified program, tape size, and growth behavior.
    ///
    /// # Arguments
    ///
    /// * `program` - A reference to the Brainfuck program to be interpreted.
    /// * `tape_size` - A `NonZeroUsize` specifying the initial tape size.
    /// * `can_grow` - A boolean indicating if the tape should dynamically grow.
    ///
    /// # Examples
    ///
    /// ```
    /// # use bft_interp::BFInterpreter;
    /// # use bft_types::BFProgram;
    /// # use std::num::NonZeroUsize;
    /// # let bf_program = BFProgram::new("example.bf", ">+<-");
    /// let tape_size = NonZeroUsize::new(30_000).unwrap();
    /// let interpreter = BFInterpreter::<u8>::new(&bf_program, tape_size, false);
    /// ```
    pub fn new(
        program: &'a BFProgram,
        tape_size: NonZeroUsize,
        can_grow: bool,
    ) -> BFInterpreter<'a, T> {
        BFInterpreter {
            tape: vec![T::default(); tape_size.get()],
            head: 0,
            can_grow,
            program,
            program_counter: 0,
        }
    }

    /// Moves the head of the interpreter one position to the left on the tape and returns the next instruction index.
    ///
    /// # Returns
    /// - `Ok(usize)`: The index of the next instruction if the head is successfully moved left.
    /// - `Err(VMError)`: If the head is already at the leftmost position.
    ///
    /// # Errors
    /// This function returns an error if the head cannot move left due to reaching the start of the tape.
    pub fn move_head_left(&mut self) -> Result<usize, VMError> {
        if self.head == 0 {
            Err(VMError::InvalidHeadPosition {
                position: self.head,
                line: *self.program.instructions()[self.program_counter].line_number(),
                column: *self.program.instructions()[self.program_counter].char_column(),
            })
        } else {
            self.head -= 1;
            Ok(self.program_counter + 1)
        }
    }

    /// Moves the head of the interpreter one position to the right on the tape and returns the next instruction index.
    ///
    /// # Returns
    /// - `Ok(usize)`: The index of the next instruction if the head is successfully moved right.
    /// - `Err(VMError)`: If the head is at the end of the tape and the tape is not configured to grow.
    ///
    /// # Errors
    /// This function returns an error if the head cannot move right due to reaching the end of a non-growable tape.
    pub fn move_head_right(&mut self) -> Result<usize, VMError> {
        if self.head >= self.tape.len() - 1 && !self.can_grow {
            Err(VMError::InvalidHeadPosition {
                position: self.head,
                line: *self.program.instructions()[self.program_counter].line_number(),
                column: *self.program.instructions()[self.program_counter].char_column(),
            })
        } else {
            self.head += 1;
            if self.head >= self.tape.len() && self.can_grow {
                self.tape.push(T::default());
            }
            Ok(self.program_counter + 1)
        }
    }

    /// Increments the cell value at the current tape head.
    ///
    /// # Returns
    /// Next instruction index or an error.
    pub fn increment_cell(&mut self) -> Result<usize, VMError> {
        self.tape[self.head].increment();
        Ok(self.program_counter + 1)
    }

    /// Decrements the cell value at the current tape head.
    ///
    /// # Returns
    /// Next instruction index or an error.
    pub fn decrement_cell(&mut self) -> Result<usize, VMError> {
        self.tape[self.head].decrement();
        Ok(self.program_counter + 1)
    }

    /// Jumps to the command after the matching `[` if the current cell's value is non-zero.
    ///
    /// If the value at the tape head is non-zero, this function finds the index of the matching
    /// `[` instruction and jumps to the command right after it. If the cell's value is zero, it
    /// proceeds to the next instruction.
    ///
    /// # Returns
    /// - `Ok(usize)`: Index of the command after the matching `[` if the cell's value is non-zero.
    /// - `Ok(usize)`: Index of the next instruction if the cell's value is zero.
    ///
    /// # Errors
    /// - `Err(VMError)`: Returned if a matching `[` cannot be found for a `]` instruction.
    pub fn jump_if_nonzero(&mut self) -> Result<usize, VMError> {
        if self.tape[self.head].to_value() != 0 {
            // Find the index of the matching '['
            let matching_index = self.find_matching_open_bracket(self.program_counter)?;
            // Jump to the instruction after the matching '['
            Ok(matching_index + 1)
        } else {
            Ok(self.program_counter + 1)
        }
    }

    /// Finds the index of the matching `[` for a given `]` instruction index.
    ///
    /// Looks up the corresponding `[` instruction in the `bracket_pairs` hashmap.
    /// Assumes brackets are initially balanced and the hashmap is populated.
    ///
    /// # Arguments
    /// * `start_index` - Index of the `]` instruction.
    ///
    /// # Returns
    /// - `Ok(usize)`: Index of the matching `[` instruction.
    /// - `Err(VMError::UnmatchedBracket)`: If no matching `[` is found.
    ///
    /// # Errors
    /// - [`VMError::UnmatchedBracket`]: When the matching `[` bracket cannot be found.
    pub fn find_matching_open_bracket(&self, start_index: usize) -> Result<usize, VMError> {
        match self.program.instructions().get(start_index) {
            Some(instruction) if instruction.instruction() == &Instruction::LoopEnd => {
                match self.program.get_matching_bracket(start_index) {
                    Some(matching_index) => Ok(matching_index),
                    None => Err(VMError::UnmatchedBracket {
                        bracket_type: String::from("]"),
                        line: *instruction.line_number(),
                        column: *instruction.char_column(),
                    }),
                }
            }
            _ => Err(VMError::UnmatchedBracket {
                bracket_type: String::from("]"),
                line: *self.program.instructions()[start_index].line_number(),
                column: *self.program.instructions()[start_index].char_column(),
            }),
        }
    }

    /// Jumps to the command after the matching `]` if the current cell's value is zero.
    ///
    /// If the value at the tape head is zero, this function finds the index of the matching
    /// `]` instruction and jumps to the command right after it. If the cell's value is non-zero,
    /// it proceeds to the next instruction.
    ///
    /// # Returns
    /// - `Ok(usize)`: Index of the command after the matching `]` if the cell's value is zero.
    /// - `Ok(usize)`: Index of the next instruction if the cell's value is non-zero.
    ///
    /// # Errors
    /// - `Err(VMError)`: Returned if a matching `]` cannot be found for a `[` instruction.
    pub fn jump_if_zero(&mut self) -> Result<usize, VMError> {
        if self.tape[self.head].to_value() == 0 {
            let matching_index = self.find_matching_close_bracket(self.program_counter)?;
            Ok(matching_index + 1)
        } else {
            Ok(self.program_counter + 1)
        }
    }

    /// Finds the index of the matching `]` for a given `[` instruction index.
    ///
    /// Looks up the corresponding `]` instruction in the `bracket_pairs` hashmap.
    /// Assumes brackets are initially balanced and the hashmap is populated.
    ///
    /// # Arguments
    /// * `start_index` - Index of the `[` instruction.
    ///
    /// # Returns
    /// - `Ok(usize)`: Index of the matching `]` instruction.
    /// - `Err(VMError::UnmatchedBracket)`: If no matching `]` is found.
    ///
    /// # Errors
    /// - [`VMError::UnmatchedBracket`]: When the matching `]` bracket cannot be found.
    pub fn find_matching_close_bracket(&self, start_index: usize) -> Result<usize, VMError> {
        match self.program.instructions().get(start_index) {
            Some(instruction) if instruction.instruction() == &Instruction::LoopStart => {
                match self.program.get_matching_bracket(start_index) {
                    Some(matching_index) => Ok(matching_index),
                    None => Err(VMError::UnmatchedBracket {
                        bracket_type: String::from("["),
                        line: *instruction.line_number(),
                        column: *instruction.char_column(),
                    }),
                }
            }
            _ => Err(VMError::UnmatchedBracket {
                bracket_type: String::from("["),
                line: *self.program.instructions()[start_index].line_number(),
                column: *self.program.instructions()[start_index].char_column(),
            }),
        }
    }

    /// Getter for the tape
    pub fn tape(&self) -> &Vec<T> {
        &self.tape
    }

    /// Getter for the head
    pub fn head(&self) -> usize {
        self.head
    }

    /// Getter for the can_grow field
    pub fn can_grow(&self) -> bool {
        self.can_grow
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::io::Cursor;
    use std::num::NonZeroUsize;

    struct FailingReader;

    impl Read for FailingReader {
        fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::Other, "forced read error"))
        }
    }

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::Other, "forced write error"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[rstest]
    #[case(false)]
    #[case(true)]
    fn test_move_head_left(#[case] can_grow: bool) {
        let program = BFProgram::new("dummy.bf", "+-<>");
        let tape_size = NonZeroUsize::new(10).unwrap();
        let mut interpreter: BFInterpreter<u8> = BFInterpreter::new(&program, tape_size, can_grow);

        interpreter.head = 5; // Set head to a non-zero position
        assert!(interpreter.move_head_left().is_ok());
        assert_eq!(interpreter.head, 4); // Head should have moved left
    }

    #[rstest]
    fn test_move_head_left_at_start() {
        let program = BFProgram::new("dummy.bf", "+-<>");
        let tape_size = NonZeroUsize::new(10).unwrap();
        let mut interpreter: BFInterpreter<u8> = BFInterpreter::new(&program, tape_size, false);

        interpreter.head = 0; // Set head to the start
        assert!(matches!(
            interpreter.move_head_left(),
            Err(VMError::InvalidHeadPosition { position, .. }) if position == 0
        ));
    }

    #[rstest]
    #[case(false)]
    fn test_move_head_right_fail_on_non_growable_tape(#[case] can_grow: bool) {
        let program = BFProgram::new("dummy.bf", "+-<>");
        let tape_size = NonZeroUsize::new(10).unwrap(); // Tape size is 10
        let mut interpreter: BFInterpreter<u8> = BFInterpreter::new(&program, tape_size, can_grow);

        interpreter.head = 9; // Set head to the end of the tape

        // Expect a failure when trying to move right
        assert!(matches!(
            interpreter.move_head_right(),
            Err(VMError::InvalidHeadPosition { position, .. }) if position == 9
        ));
    }

    #[rstest]
    #[case(true)]
    fn test_move_head_right_success_on_growable_tape(#[case] can_grow: bool) {
        let program = BFProgram::new("dummy.bf", "+-<>");
        let tape_size = NonZeroUsize::new(10).unwrap(); // Tape size is 10
        let mut interpreter: BFInterpreter<u8> = BFInterpreter::new(&program, tape_size, can_grow);

        interpreter.head = 9; // Set head to the end of the tape

        // Expect success and tape growth when trying to move right
        assert!(interpreter.move_head_right().is_ok());
        assert_eq!(interpreter.head, 10); // Head should have moved right to position 10
    }

    #[rstest]
    fn test_move_head_right_at_end_non_growable() {
        let program = BFProgram::new("dummy.bf", "+-<>");
        let tape_size = NonZeroUsize::new(10).unwrap();
        let mut interpreter: BFInterpreter<u8> = BFInterpreter::new(&program, tape_size, false);

        interpreter.head = 9; // Set head to the end of a non-growable tape
        assert!(matches!(
            interpreter.move_head_right(),
            Err(VMError::InvalidHeadPosition { position, .. }) if position == 9
        ));
    }

    #[rstest]
    #[case(false)]
    #[case(true)]
    fn test_increment_and_decrement_cell(#[case] can_grow: bool) {
        let program = BFProgram::new("dummy.bf", "+-<>");
        let tape_size = NonZeroUsize::new(10).unwrap();
        let mut interpreter: BFInterpreter<u8> = BFInterpreter::new(&program, tape_size, can_grow);

        interpreter.increment_cell().unwrap();
        assert_eq!(interpreter.tape()[interpreter.head()], 1);

        interpreter.decrement_cell().unwrap();
        assert_eq!(interpreter.tape()[interpreter.head()], 0);
    }

    #[rstest]
    #[case(false)]
    #[case(true)]
    fn test_read_value(#[case] can_grow: bool) {
        let bf_program = BFProgram::new("example.bf", ">+<-");
        let tape_size = NonZeroUsize::new(10_000).unwrap();
        let mut interpreter = BFInterpreter::<u8>::new(&bf_program, tape_size, can_grow);

        let input_data = vec![65]; // ASCII value of 'A'
        let mut input_reader = Cursor::new(input_data);

        interpreter.read_value(&mut input_reader).unwrap();
        assert_eq!(interpreter.tape[interpreter.head], 65); // ASCII value of 'A'
    }

    #[rstest]
    #[case(false)]
    #[case(true)]
    fn test_write_value(#[case] can_grow: bool) {
        let bf_program = BFProgram::new("example.bf", ">+<-");
        let tape_size = NonZeroUsize::new(10_000).unwrap();
        let mut interpreter = BFInterpreter::<u8>::new(&bf_program, tape_size, can_grow);

        interpreter.tape[interpreter.head] = 66; // ASCII value of 'B'

        let mut output_buffer = Cursor::new(Vec::new());

        interpreter.write_value(&mut output_buffer).unwrap();
        assert_eq!(output_buffer.into_inner(), &[66]); // ASCII value of 'B'
    }

    #[rstest]
    #[case(false)]
    #[case(true)]
    fn test_read_value_fail(#[case] can_grow: bool) {
        let bf_program = BFProgram::new("example.bf", ","); // Program contains a single Input instruction
        let tape_size = NonZeroUsize::new(10_000).unwrap();
        let mut interpreter = BFInterpreter::<u8>::new(&bf_program, tape_size, can_grow);

        let mut failing_reader = FailingReader;

        let result = interpreter.read_value(&mut failing_reader);
        assert!(matches!(
            result,
            Err(VMError::IOError {
                source: _,
                instruction: Instruction::Input // Asserting specific instruction
            })
        ));
    }

    #[rstest]
    #[case(false)]
    #[case(true)]
    fn test_write_value_fail(#[case] can_grow: bool) {
        let bf_program = BFProgram::new("example.bf", "."); // Program contains a single Output instruction
        let tape_size = NonZeroUsize::new(10_000).unwrap();
        let mut interpreter = BFInterpreter::<u8>::new(&bf_program, tape_size, can_grow);

        interpreter.tape[interpreter.head] = 66; // Set some value

        let mut failing_writer = FailingWriter;

        let result = interpreter.write_value(&mut failing_writer);
        assert!(matches!(
            result,
            Err(VMError::IOError {
                source: _,
                instruction: Instruction::Output // Asserting specific instruction
            })
        ));
    }

    #[rstest]
    #[case("+[+]", 2, 2, true, 3)] // The ']' is at index 3 in the program string, should jump
    #[case("[-]", 0, 1, true, 2)] // The ']' is at index 2, no jump expected as tape is zero
    #[case(">+[+]<", 1, 3, true, 4)] // The ']' is at index 4, should jump to index 3 as cell at head is non-zero
    #[case("++[-]", 2, 3, false, 2)] // The ']' is at index 3, no jump expected as cell at head (2) is zero
    #[case(">>[-]<<", 2, 5, false, 4)] // The ']' is at index 4, no jump expected as cell at head (2) is zero
    #[case(">+[-]<", 1, 5, false, 4)] // The ']' is at index 4, no jump expected as cell at head is zero
    fn test_jump_if_nonzero(
        #[case] program_str: &str,
        #[case] start_head: usize,
        #[case] expected_position: usize,
        #[case] should_jump: bool,
        #[case] program_counter_pos: usize, // Add this parameter
    ) {
        let mut program = BFProgram::new("dummy.bf", program_str);
        // Populate hashmap (we're not testing for unbalanced brackets)
        let _ = program.check_brackets();

        let tape_size = NonZeroUsize::new(10).unwrap();
        let mut interpreter: BFInterpreter<u8> = BFInterpreter::new(&program, tape_size, false);

        // Initialize the tape based on whether a jump is expected
        interpreter.tape = vec![if should_jump { 1 } else { 0 }; tape_size.get()];

        interpreter.head = start_head;
        // Directly set the program counter to the specified position
        interpreter.program_counter = program_counter_pos;

        let result = interpreter.jump_if_nonzero().unwrap();

        println!("Expected: {:?}, Actual: {:?}", expected_position, result);
        assert_eq!(result, expected_position);
    }

    #[rstest]
    #[case(">[<]", 3, Some(1))]
    #[case("[[[]]]", 3, Some(2))]
    #[case("[[[]]]", 4, Some(1))]
    #[case(">[>[<]<]", 7, Some(1))]
    #[case("[[]][]", 5, Some(4))]
    #[case("[][][][]", 5, Some(4))]
    #[case("[[[]]][]", 7, Some(6))]
    #[case("[[][][][]]", 9, Some(0))]
    #[case("[[[[[]]]]]", 8, Some(1))]
    fn test_find_matching_open_bracket(
        #[case] program_str: &str,
        #[case] start_index: usize,
        #[case] expected_matching_index: Option<usize>,
    ) {
        let mut program = BFProgram::new("dummy.bf", program_str);
        program.check_brackets().unwrap(); // Populates the bracket_pairs hashmap
        let interpreter: BFInterpreter<u8> =
            BFInterpreter::new(&program, NonZeroUsize::new(10).unwrap(), false);

        let result = interpreter.find_matching_open_bracket(start_index);

        // Directly check that the result matches the expected index
        assert_eq!(result.unwrap(), expected_matching_index.unwrap());
    }

    #[rstest]
    #[case("[+]", 0, 3, true, 0)] // The '[' is at index 0, should jump as cell at head is zero
    #[case(">[-]<", 1, 4, true, 1)] // The '[' is at index 1, should jump as cell at head is zero
    #[case("++[->+<]", 2, 8, true, 2)] // The '[' is at index 2, should jump as cell at head is zero
    #[case(">[+]<", 1, 2, false, 1)] // No jump as cell at head is non-zero
    #[case(">[>[+]]<", 2, 6, false, 5)] // No jump at nested loop as cell at head is non-zero
    #[case(">[+[+]<", 2, 4, false, 3)] // No jump at first `[` as cell at head is non-zero
    fn test_jump_if_zero(
        #[case] program_str: &str,
        #[case] start_head: usize,
        #[case] expected_position: usize,
        #[case] is_zero: bool,
        #[case] program_counter_pos: usize,
    ) {
        let mut program = BFProgram::new("dummy.bf", program_str);

        // Populate hashmap (we're not testing for unbalanced brackets)
        let _ = program.check_brackets();

        let tape_size = NonZeroUsize::new(10).unwrap();
        let mut interpreter: BFInterpreter<u8> = BFInterpreter::new(&program, tape_size, false);

        // Initialize the tape based on whether the cell at the head is zero or not
        interpreter.tape = vec![if is_zero { 0 } else { 1 }; tape_size.get()];

        interpreter.head = start_head;
        interpreter.program_counter = program_counter_pos;

        let result = interpreter.jump_if_zero().unwrap();

        println!("Expected: {:?}, Actual: {:?}", expected_position, result);
        assert_eq!(result, expected_position);
    }

    #[rstest]
    #[case(">[<]", 1, Some(3))]
    #[case("[[[]]]", 0, Some(5))]
    #[case("[[[]]]", 1, Some(4))]
    #[case(">[>[<]<]", 1, Some(7))]
    #[case("[[]][]", 4, Some(5))]
    #[case("[][][][]", 4, Some(5))]
    #[case("[[[]]][]", 6, Some(7))]
    #[case("[[][][][]]", 0, Some(9))]
    #[case("[[[[[]]]]]", 1, Some(8))]
    fn test_find_matching_close_bracket(
        #[case] program_str: &str,
        #[case] start_index: usize,
        #[case] expected_matching_index: Option<usize>,
    ) {
        let mut program = BFProgram::new("dummy.bf", program_str);
        program.check_brackets().unwrap(); // Populates the bracket_pairs hashmap
        let interpreter: BFInterpreter<u8> =
            BFInterpreter::new(&program, NonZeroUsize::new(10).unwrap(), false);

        let result = interpreter.find_matching_close_bracket(start_index);

        // Directly check that the result matches the expected index
        assert_eq!(result.unwrap(), expected_matching_index.unwrap());
    }
}
