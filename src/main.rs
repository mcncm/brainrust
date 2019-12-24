use std::env;
use std::fs;
use std::io;
use std::io::{Write, stdin, stdout};
use std::fmt;
use std::path;
use std::thread;
use std::time::Duration;
use std::process;

use termion::color;
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use itertools::{Itertools, EitherOrBoth};

const MEM_SIZE: usize = 30_000;
const WELCOME_MESSAGE: &'static str = r#"Welcome to BrainRust!
[q] quit, [a] advance
"#;

// Commands known to the VM
enum Command {
    JumpForward(usize),
    JumpBackward(usize),
    DecPtr,
    IncPtr,
    DecData,
    IncData,
    Input,
    Output,
    NoOp,
}

// Parsed instruction with satellite data
struct Instruction {
    command: Command,
    ch: char,
    pos: (usize, usize),  // Screen position
}

// Transform a sequence of characters into a sequence of instructions
fn parse(chs: &Vec<char>) -> Result<Vec<Instruction>, ()> {
    let mut instructions: Vec<Instruction> = Vec::new();
    let mut brack_stack: Vec<usize> = Vec::new();

    let (mut pos_x, mut pos_y): (usize, usize) = (0, 0);
    for (i, ch) in chs.iter().enumerate() {
        // Is this bad form?
        let command = match ch {
            '[' => {
                brack_stack.push(i);
                // To be replaced. This is probably confusing/bad form. There's
                // probably a better data structure to use that doesn't lead to
                // this confusion.
                Command::JumpForward(0)
            },
            ']' => {
                let match_pos = brack_stack.pop().ok_or(())?;
                instructions[match_pos].command = Command::JumpForward(i);
                Command::JumpBackward(match_pos)
            },
            '<' => { Command::DecPtr },
            '>' => { Command::IncPtr },
            '-' => { Command::DecData },
            '+' => { Command::IncData },
            '.' => { Command::Output },
            ',' => { Command::Input },
            '\n' => { pos_y += 1;
                      Command:: NoOp
            }
            _ => { Command::NoOp },
        };

        instructions.push(
            Instruction {
                command: command,
                ch: *ch,
                pos: (pos_x, pos_y),
            }
        );

        if *ch == '\n' {
            pos_x = 0;
        } else {
            pos_x += 1;
        }
    }

    Ok(instructions)
}

struct DisplaySpec {
    visible: bool,
    decimal: bool,
    hex: bool,
    ascii: bool,
    frame_dur: Duration,
}

impl DisplaySpec {
    fn new(rate: f32) -> DisplaySpec {
        DisplaySpec {
            visible: true,
            decimal: true,
            hex: true,
            ascii: true,
            frame_dur: Duration::from_millis((1000.0 / rate) as u64),
        }
    }
}

// Language virtual machine
struct Machine {
    prog: Vec<Instruction>,
    data: [u8; MEM_SIZE],
    prog_ctr: usize,
    data_ptr: usize,

    last_data_cell: usize,
    prog_src: Vec<String>,
    display_spec: DisplaySpec,
    output: String,
}


impl Machine {
    fn new(program: String) -> Result<Machine, ()> {
        let machine = Machine {
            prog: parse(&program.chars().collect())?,

            data: [0; MEM_SIZE],
            prog_ctr: 0,
            data_ptr: 0,

            prog_src: program.split('\n')
                .map(|s| s.to_owned())
                .collect(),
            last_data_cell: 0,
            display_spec : DisplaySpec::new(1.0),
            output: String::new(),
        };

        Ok(machine)
    }


    // Run the machine to termination.
    fn run(&mut self) {
        println!("{}{}{}{}",
                 termion::cursor::Goto(1,1),
                 termion::clear::AfterCursor,
                 WELCOME_MESSAGE,
                 termion::cursor::Hide);

        if self.display_spec.visible {
            let input_stream = stdin(); // should this go here?
            let mut output_stream = stdout().into_raw_mode().unwrap();
            self.redraw(&mut output_stream);
            for c in input_stream.keys() {
                match c.unwrap() {
                    Key::Char('q') => {
                        write!(output_stream, "{}", termion::cursor::Show).unwrap();
                        break
                    },
                    Key::Char('a') => { self.advance(); },
                    _ => { },
                }
                self.redraw(&mut output_stream);
            }
        } else {
            loop {
                self.advance();
            }
        }

    }

    // Draw the machine state
    fn redraw(&self, output_stream: &mut std::io::Stdout) {
        writeln!(output_stream, "{}{}{}",
               termion::cursor::Goto(1,3),
               termion::clear::AfterCursor,
               self);
        output_stream.flush().unwrap();
    }

    // Advance to next non-noop command
    fn advance(&mut self) {
        self.execute();
        self.inc_prog_ctr();
        while let Command::NoOp = &self.prog[self.prog_ctr].command {
            self.inc_prog_ctr();
        }
    }

    // Step forward or terminate
    fn inc_prog_ctr(&mut self) {
        if self.prog_ctr == self.prog.len() - 1 {
            println!("{}", termion::cursor::Show);
            process::exit(0);
        }
        self.prog_ctr += 1;
    }

    // Execute the command under the read head
    fn execute(&mut self) {
        match self.prog[self.prog_ctr].command {
            Command::JumpForward(i) => { self.jmp_eq(i); },
            Command::JumpBackward(i) => { self.jmp_ne(i); },
            Command::DecPtr => { self.data_ptr -= 1; },
            Command::IncPtr => { self.data_ptr += 1; },
            Command::DecData => { self.dec_data(); },
            Command::IncData => { self.inc_data(); },
            Command::Output => { self.output.push(self.data[self.data_ptr] as char); },
            Command::Input => { todo!(); },
            Command::NoOp => { },
        }
    }

    // Jump to point if zero under read head
    fn jmp_eq(&mut self, i: usize) {
        if self.data[self.data_ptr] == 0 {
            self.prog_ctr = i;
        }
    }

    // Jump to point if nonzero under read head
    fn jmp_ne(&mut self, i: usize) {
        if self.data[self.data_ptr] != 0 {
            self.prog_ctr = i;
        }
    }

    // Decrement the data cell; track last nonzero cell.
    // TODO consider using a (slightly) more sophisticated data structure here.
    fn dec_data(&mut self) {
        self.data[self.data_ptr] -= 1;
        if self.data[self.data_ptr] == 0 &&
            self.data_ptr == self.last_data_cell {
                let mut p = self.data_ptr;
                while self.data[p] == 0 || p > 0 {
                    p -= 1;
                }
                self.last_data_cell = p;
            }
    }

    // Increment the data cell.
    fn inc_data(&mut self) {
        if self.data[self.data_ptr] == 0 &&
            self.data_ptr > self.last_data_cell {
                self.last_data_cell = self.data_ptr;
            }
        self.data[self.data_ptr] += 1;
    }

    // Returns a formatted data cell in decimal, hex, and ascii
    // TODO This is pretty janky. I feel like I'm missing an abstraction here.
    // Should I be using a custom formatter?
    // TODO I'm not sure what the "right" place to put it is.
    fn fmt_data_cell(&self, cell: usize) -> String {
        let data = &self.data[cell];
        let text = format!("{}{}{}",
                           if self.display_spec.decimal {
                               format!("{:03}", data)       // Decimal column
                           } else {
                               String::new()
                           },

                           if self.display_spec.hex {      // Hex column
                               format!(" 0x{:02x}", data)
                           } else {
                               String::new()
                           },

                           if self.display_spec.ascii {    // Ascii  column
                               // Printable ascii: is there a better way to do this?
                               // Also, this gets DEL wrong
                               format!(" {:}", if *data >= 0x20 as u8 { (*data as char) }
                                       else { ' ' })
                           } else {
                               String::new()
                           },
        );
        if cell == self.data_ptr {
            format!("{}{}{}",
                    color::Bg(color::Blue),
                    text,
                    color::Bg(color::Reset)
            )
        } else {
            text
        }
    }

    // Returns a formatted line of source code with read-head highlighting
    fn fmt_src_line(&self, linum: usize) -> String {
        let (pos_x, pos_y) = self.prog[self.prog_ctr].pos;
        if linum == pos_y {
            let (head, tail) = &self.prog_src[linum].split_at(pos_x);
            let (ch, tail) = if tail.len() > 0 {
                tail.split_at(1)
            } else {
                ("", "")
            };
            format!("{}{}{}{}{}",
                    head,
                    color::Bg(color::Blue),
                    ch,
                    color::Bg(color::Reset),
                    tail)
        } else {
            self.prog_src[linum].clone()
        }
    }
}

impl fmt::Display for Machine {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let data_col_width = self.fmt_data_cell(0).len();
        let repr = (0..=std::cmp::max(self.last_data_cell, self.data_ptr))  // Data column
            // TODO should I put the `fmt_data_cell` here, or in the `match cols` below?
            // .map(|x| self.fmt_data_cell(x))  // Format the left-hand column
            .zip_longest(0..self.prog_src.len())     // Zip with source column
            .map(|cols| {                            // Join the columns
                match cols {
                    EitherOrBoth::Both(cell, src) => {
                        format!("{} {}\r\n", self.fmt_data_cell(cell),
                                self.fmt_src_line(src))
                    }
                    EitherOrBoth::Left(cell) => {
                        format!("{}\r\n", self.fmt_data_cell(cell))
                    },
                    EitherOrBoth::Right(src) => {
                        format!("           {}\r\n",  // TODO this is a bug
                                self.fmt_src_line(src))
                                //width = data_col_width + 1)
                    },
                }
            })
            .collect::<String>();

        write!(f, "{}\r\n{}\r\n{}",      // The output line
               color::Fg(color::Green),
               self.output,
               color::Fg(color::Reset),
        );
        write!(f, "{}", repr)            // The memory and source
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let script: &str = &args[1];

    let program = fs::read_to_string(path::Path::new(script))
        .unwrap_or_else(|_| {
            eprintln!("File read failed!");
            process::exit(1);
        });
    let mut machine = Machine::new(program)
        .unwrap_or_else(|_| {
            eprintln!("Failed to parse program!");
            process::exit(2);
        });
    machine.run();
}
