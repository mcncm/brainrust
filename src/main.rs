use std::env;
use std::fs;
use std::io;
use std::path;
use std::process;

// use termion;

const MEM_SIZE: usize = 30_000;

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
    pos: (usize, usize),  // Screen position
}

// Transform a sequence of characters into a sequence of instructions
fn parse(chs: &Vec<char>) -> Result<Vec<Instruction>, ()> {
    let mut instructions: Vec<Instruction> = Vec::new();
    let mut brack_stack: Vec<usize> = Vec::new();

    let (mut pos_x, mut pos_y): (usize, usize) = (0, 1);
    for (i, ch) in chs.iter().enumerate() {
        pos_x += 1;
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
            '\n' => { pos_x = 0;
                      pos_y += 1;
                      Command:: NoOp
            }
            _ => { Command::NoOp },
        };

        instructions.push(
            Instruction {
                command: command,
                pos: (pos_x, pos_y),
            }
        );
    }

    Ok(instructions)
}

// Language virtual machine
struct Machine {
    prog_src: Vec<char>,
    prog: Vec<Instruction>,
    data: [u8; MEM_SIZE],
    prog_ctr: usize,
    data_ptr: usize,

    visible: bool,
    last_data_cell: usize,
}

impl Machine {
    fn new(program: String) -> Result<Machine, ()> {
        let prog_src = program.chars().collect();
        let machine = Machine {
            prog: parse(&prog_src)?,
            prog_src: prog_src,

            data: [0; ARRAY_SIZE],
            prog_ctr: 0,
            data_ptr: 0,

            last_data_cell: 0,
            visible: false,
        };

        Ok(machine)
    }

    // Run the machine to termination.
    fn run(&mut self) {
        if self.visible {
            loop {
                self.advance();
            }
        } else {
            loop {
                self.advance();
            }
        }
    }

    // Advance to next non-noop command
    fn advance(&mut self) {
        while let Command::NoOp = &self.prog[self.prog_ctr].command {
            self.inc_prog_ctr();
        }
        self.execute();
        self.inc_prog_ctr();
    }

    // Step forward or terminate
    fn inc_prog_ctr(&mut self) {
        if self.prog_ctr == self.prog.len() - 1 {
            process::exit(0);
        }
        self.prog_ctr += 1;
    }

    // Execute the command under the read head
    fn execute(&mut self) {
        match self.prog[self.prog_ctr].command {
            Command::JumpForward(i) => { self.jmp_forward(i); },
            Command::JumpBackward(i) => { self.jmp_backward(i); },
            Command::DecPtr => { self.data_ptr -= 1; },
            Command::IncPtr => { self.data_ptr += 1; },
            Command::DecData => { self.dec_data(); },
            Command::IncData => { self.inc_data(); },
            Command::Output => { print!("{}", self.data[self.data_ptr] as char); },
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
