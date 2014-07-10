#![crate_type="bin"]
#![feature(concat_idents, macro_rules, phase)]

#[phase(plugin, link)] extern crate log;

use std::default::Default;
use std::rand::{Rng, StdRng};

use cpu::Registers;
use display::Display;
use mem::{ROM_LOC, Memory, Rom};

mod cpu;
mod display;
mod mem;

struct Vm {
    mem: Memory,
    reg: Registers,
    pc: u16,
    dt: u8, // delay timer
    st: u8, // sound timer
    i: u16, // index register
    ret_stack: Vec<u16>, // return stack
    _display: Display,
    rng: StdRng
}

impl Vm {
    fn new(r: Rom, rng: StdRng) -> Vm {
        let mut mem = Memory::new();
        mem.load_rom(r);
        Vm {
            mem: mem,
            reg: Default::default(),
            pc: mem::ROM_LOC,
            dt: 0,
            st: 0,
            i: 0,
            ret_stack: vec![],
            _display: Display::new(),
            rng: rng
        }
    }

    fn math_op(&mut self, x: u8, y: u8, op: u8) {
        match op {
            0x0 => {
                let dst = self.reg.get_mut(x);
                *dst = y;
            },
            0x4 => {
            }
            0xe => {
                self.reg.set_flag(y & 0x80);
                let dst = self.reg.get_mut(x);
                *dst = y << 1;
            },
            _ => fail!("math op {:01x} unimplemented", op)
        }
    }

    fn misc(&mut self, x: u8, nn: u8) -> bool {
        match nn {
            0x07 => { // set register from delay timer
                *self.reg.get_mut(x) = self.dt;
            },
            0x1e => {
                self.i += self.reg.get(x) as u16;
            },
            0x65 => {
                let start = self.i;
                for i in range(0, x + 1) {
                    *self.reg.get_mut(i) = self.mem.get(start + i as u16)
                }
                self.i = self.i + x as u16 + 1;
            }
            _ => return false
        }
        true
    }

    fn tick(&mut self) {
        let (lo, hi) = (self.mem.get(self.pc), self.mem.get(self.pc + 1));
        let ins: u16 = (lo as u16) << 8 | hi as u16;
        let op = (lo >> 4) & 0xf;
        let x = lo & 0xf;
        let y = (hi >> 4) & 0xf;
        let n = hi & 0xf;
        let nn = hi & 0xff;
        let nnn = ins & 0xfff;

        debug!("{:04x}", ins);
        debug!("{}", self.reg);

        self.pc += 2;

        if ins == 0x00e0 { // clear screen, TODO
            println!("FIXME (clear screen)");
            return;
        }

        if ins == 0x00ee { // return
            self.pc = self.ret_stack.pop().expect("stack underflow");
            return;
        }

        // match_hex! macro ??
        match op {
            0x1 => { // jump
                self.pc = nnn;
            },
            0x2 => { // call
                self.ret_stack.push(self.pc);
                self.pc = nnn;
            },
            0x3 => { // skip if eq
                if self.reg.get(x) == nn {
                    self.pc += 2;
                }
            },
            0x4 => { // skip if ne
                if self.reg.get(x) != nn {
                    self.pc += 2;
                }
            },
            0x6 => { // store
                *self.reg.get_mut(x) = nn;
            },
            0x7 => { // add
                let r = self.reg.get_mut(x);
                *r = *r + nn;
            },
            0x8 => { // math
                self.math_op(x, y, n);
            },
            0xa => { // set index register
                self.i = nnn;
            },
            0xc => {
                *self.reg.get_mut(x) = self.rng.gen::<u8>() & nn;
            }
            0xd => { // draw sprite, TODO
                println!("FIXME (draw sprite)");
            },
            0xe => { // keypresses
                println!("FIXME (keypresses)");
            },
            0xf if nn == 0x15 => { // set delay timer from register
                self.dt = self.reg.get(x);
            },
            0xf => {
                if !self.misc(x, nn) {
                    fail!("{:04x} not yet implemented", ins)
                }
            },
            _ => fail!("{:04x} not yet implemented", ins)
        }
    }

    fn render(&mut self) {
        if self.dt > 0 { self.dt -= 1 }
        if self.st > 0 { self.st -= 1 }
    }
}

pub fn main() {
    use std::io::File;
    use std::io::Timer;

    static CYCLES_PER_FRAME: u8 = 100;

    let mut rom_file = File::open(&Path::new("smiley.rom"));
    let rom = match Rom::from_reader(&mut rom_file) {
        Ok(r) => r,
        Err(e) => {
            println!("Error loading ROM: {}", e.desc);
            return;
        }
    };

    let rng = match StdRng::new() {
        Ok(r) => r,
        Err(e) => {
            println!("Error creating RNG: {}", e.desc);
            return;
        }
    };

    let mut vm = Vm::new(rom, rng);

    let mut timer = Timer::new().unwrap();
    let sixty_hz = timer.periodic(1000 / 60); // not really 60 Hz...

    loop {
        for _ in range(0, CYCLES_PER_FRAME) {
            vm.tick();
        }
        sixty_hz.recv();
        vm.render();
    }
}
