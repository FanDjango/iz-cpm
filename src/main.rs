use std::fs::File;
use std::io::prelude::*;
use clap::{Arg, App};

use iz80::*;

mod bdos;
mod bios;
mod constants;
mod bdos_console;
mod bdos_drive;
mod bdos_file;
mod cpm_machine;
mod fcb;

use self::bdos::Bdos;
use self::bios::Bios;
use self::constants::*;
use self::cpm_machine::*;
use self::fcb::*;


static CCP_BINARY: &'static [u8] = include_bytes!("../cpm22/OS2CCP.BIN");

fn main() {
    // Parse arguments
    let matches = App::new("Z80 CP/M 2.2 emulator")
        .arg(Arg::with_name("CMD")
            .help("The z80 image to run")
            .required(false)
            .index(1))
            .arg(Arg::with_name("ARGS")
            .help("Parameters for the given command")
            .required(false)
            .index(2))
        .arg(Arg::with_name("call_trace")
            .short("t")
            .long("call-trace")
            .help("Trace BDOS and BIOS calls"))
        .arg(Arg::with_name("cpu_trace")
            .short("c")
            .long("cpu-trace")
            .help("Trace BDOS and BIOS calls"))
        .get_matches();
    let filename = matches.value_of("CMD");
    let params = matches.value_of("ARGS");
    let call_trace = matches.is_present("call_trace");
    let cpu_trace = matches.is_present("cpu_trace");
    let call_trace_skip_console = true;

    // Init device
    let mut machine = CpmMachine::new();
    let mut cpu = Cpu::new();

    // Init cpm
    let mut bios = Bios::new();
    bios.setup(&mut machine);
    let mut bdos = Bdos::new();
    bdos.setup(&mut machine);


    // Load CCP or program
    let binary: &[u8];
    let binary_address: u16;
    let binary_size: usize;
    let mut buf = [0u8;65536 - (TPA_BASE_ADDRESS as usize)];
    match filename {
        None => {
            // Load TPA
            binary = CCP_BINARY;
            binary_address = CCP_BASE_ADDRESS;
            binary_size = CCP_BINARY.len();
        },
        Some(name) => {
            /*
            If the file is found, it is assumed to be a memory image of a
            program that executes in the TPA and thus implicity originates
            at TBASE in memory.
            */
            let mut file = File::open(name).unwrap();            
            binary_size = file.read(&mut buf).unwrap();
            binary = &buf;
            binary_address = TPA_BASE_ADDRESS;
        }
    }

    // Load the code in Z80 memory
    for i in 0..binary_size {
        machine.poke(binary_address + i as u16, binary[i]);
    }

    // Copy parameters
    /*
    As an added convenience, the default buffer area at location BOOT+0080H is
    initialized to the command line tail typed by the operator following the
    program name. The first position contains the number of characters, with
    the characters themselves following the character count. The characters are
    translated to upper-case ASCII with uninitialized memory following the last
    valid character. 
    */
    match params {
        None => machine.poke(SYSTEM_PARAMS_ADDRESS, 0),
        Some(p) => {
            let mut len = p.len();
            if len > 0x7E {
                len = 0x7E; // Max 0x7E chars for parameters
            }
            machine.poke(SYSTEM_PARAMS_ADDRESS, (len + 1) as u8);
            machine.poke(SYSTEM_PARAMS_ADDRESS, ' ' as u8);
            let p_bytes = p.as_bytes();
            for i in 0..len {
                machine.poke(SYSTEM_PARAMS_ADDRESS + (i as u16) + 2, p_bytes[i]);
            }

            /*
            As a convenience, the CCP takes the first two parameters that appear
            in the command tail, attempts to parse them as though they were file
            names, and places the results in FCBI and FCB2. The results, in this
            context, mean that the logical disk letter is converted to its FCB
            representation, and the file name and type, converted to uppercase,
            are placed in the FCB in the correct bytes.
            In addition, any use of "*" in the file name is expanded to one or
            more question marks. For example, a file name of "abc*.*" will be
            converted to a name of "ABC!!???" and type of "???".
            Notice that FCB2 starts only 16 bytes above FCBI, yet a normal FCB
            is at least 33 bytes long (36 bytes if you want to use random access).
            In many cases, programs only require a single file name. Therefore,
            you can proceed to use FCBI straight away, not caring that FCB2 will
            be overwritten.
            */
            let mut parts = p.split_ascii_whitespace();
            if let Some(arg1) = parts.next() {
                if let Some(file1) = name_to_8_3(arg1) {
                    Fcb::new(FCB1_ADDRESS, &mut machine).set_name(file1);
                }
            }
            if let Some(arg2) = parts.next() {
                if let Some(file2) = name_to_8_3(arg2) {
                    Fcb::new(FCB2_ADDRESS, &mut machine).set_name(file2);
                }
            }
        }
    }

    /*
    Upon entry to a transient program, the CCP leaves the stack pointer set to
    an eight-level stack area with the CCP return address pushed onto the stack,
    leaving seven levels before overflow occurs. 
    */
    if binary_address == TPA_BASE_ADDRESS {
        let mut sp = TPA_STACK_ADDRESS;
        // Push 0x0000
        machine.poke(sp, (0x0000 >> 8) as u8);
        sp -= 1;
        machine.poke(sp, 0x0000 as u8);
        sp -= 1;
        cpu.registers().set16(Reg16::SP, sp);
    }

    cpu.registers().set_pc(binary_address);
    cpu.set_trace(cpu_trace);
    loop {
        cpu.execute_instruction(&mut machine);

        let pc = cpu.registers().pc();

        if cpu.is_halted() {
            panic!("HALT instruction")
        }

        if bios.execute(cpu.registers(), call_trace) {
            println!("Terminated");
            break;
        }

        bdos.execute(&mut bios, &mut machine, cpu.registers(), call_trace, call_trace_skip_console);

        if pc == BDOS_BASE_ADDRESS - 1 {
            // Guard to detect code reaching BDOS (usually NOPs)
            panic!("Executing into BDOS area");
       }

    }
}