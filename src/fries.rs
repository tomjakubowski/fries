#![crate_type="bin"]
#![feature(concat_idents, macro_rules, phase)]

#[phase(plugin, link)] extern crate log;

extern crate sdl2;

use sdl2::keycode;
use sdl2::keycode::KeyCode;
use sdl2::render::{Renderer, Texture};
use sdl2::video::Window;

use std::collections::HashMap;
use std::default::Default;
use std::rand::{Rng, StdRng};

use cpu::Registers;
use display::Display;
use mem::{ROM_LOC, Memory, Rom};

mod cpu;
mod display;
mod mem;

static SCALE: uint         = 10;
static WINDOW_WIDTH: uint  = display::COLS * SCALE;
static WINDOW_HEIGHT: uint = display::ROWS * SCALE;

struct Vm {
    mem: Memory,
    reg: Registers,
    pc: u16,
    dt: u8, // delay timer
    st: u8, // sound timer
    i: u16, // index register
    ret_stack: Vec<u16>, // return stack
    display: Display,
    rng: StdRng,
    blocked: bool,
    blocked_reg: u8,
    keys: u16,
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
            display: Display::new(),
            rng: rng,
            blocked: false,
            blocked_reg: 255,
            keys: 0
        }
    }

    fn math_op(&mut self, x: u8, y: u8, op: u8) {
        let vx = self.reg.get(x);
        let vy = self.reg.get(y);
        match op {
            0x0 => { // VX = VY
                let dst = self.reg.get_mut(x);
                *dst = vy;
            },
            0x1 => { // VX |= VY
                let dst = self.reg.get_mut(x);
                *dst |= vy;
            },
            0x2 => { // VX &= VY
                let dst = self.reg.get_mut(x);
                *dst &= vy;
            },
            0x3 => { // VX ^= VY
                let dst = self.reg.get_mut(x);
                *dst ^= vy;
            },
            0x4 => { // VX += VY, carry -> VF
                let res: u8 = {
                    let dst = self.reg.get_mut(x);
                    *dst += vy;
                    *dst
                };
                self.reg.set_flag((res < vy) as u8);
            },
            0x5 => { // VX -= VY, borrow -> VF
                self.reg.set_flag((vy > vx) as u8);
                let dst = self.reg.get_mut(x);
                *dst -= vy;
            },
            0x6 => { // VX = VY >> 1, VF = LSB(VY)
                // The documentation + implementations of the shift
                // instructions for CHIP-8 are inconsistent and
                // contradictory to say the least. We follow Octo
                // here.
                let res = vy >> 1;
                self.reg.set_flag(vy & 0x1);
                *self.reg.get_mut(x) = res;
            },
            0x7 => { // VX = VY - VX, borrow -> VF
                self.reg.set_flag((vx > vy) as u8);
                let dst = self.reg.get_mut(x);
                *dst = vy - *dst;
            },
            0xe => { // VX = VY << 1, VF = MSB(VY)
                // The documentation + implementations of the shift
                // instructions for CHIP-8 are inconsistent and
                // contradictory to say the least. We follow Octo
                // here.
                let res = vy << 1;
                self.reg.set_flag((vy >> 7) & 0x1);
                *self.reg.get_mut(x) = res;
            },
            _ => fail!("math op {:01x} unimplemented", op)
        }
    }

    fn misc(&mut self, x: u8, nn: u8) -> bool {
        match nn {
            0x07 => { // set register from delay timer
                *self.reg.get_mut(x) = self.dt;
            },
            0x15 => {
                self.dt = self.reg.get(x)
            },
            0x18 => {
                self.st = self.reg.get(x)
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

        if ins == 0x00e0 { // clear screen
            self.display.clear();
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
            0xc => { // random number
                *self.reg.get_mut(x) = self.rng.gen::<u8>() & nn;
            }
            0xd => { // draw sprite
                let sprite = self.mem.slice(self.i, self.i + (n as u16));
                let (vx, vy) = (self.reg.get(x), self.reg.get(y));
                self.display.draw(sprite, vx, vy);
            },
            0xe if nn == 0x9e => { // skip if key in VX is pressed
                debug!("checking {:1x} in {:016t}", self.reg.get(x), self.keys);
                if self.is_key_pressed(self.reg.get(x) as uint) {
                    self.pc += 2;
                }
            },
            0xe if nn == 0xa1 => { // skip if key in VX is not pressed
                debug!("checking {:1x} in {:016t}", self.reg.get(x), self.keys);
                if !self.is_key_pressed(self.reg.get(x) as uint) {
                    self.pc += 2;
                }
            },
            0xf if nn == 0x0a => { // wait for keypress
                self.blocked_reg = x;
                self.blocked = true;
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

    fn render(&mut self, texture: &Texture) -> Result<(), String> {
        use std::mem;

        if self.dt > 0 { self.dt -= 1 }
        if self.st > 0 { self.st -= 1 }
        static PIXEL_SIZE: uint = 4; // sizeof u32 / sizeof u8

        let vec: Vec<u32> = self.display.pixels().map(|px| {
            if px.is_on() { 0xFFCC00FF } else { 0x996600FF }
        }).collect();
        let slice: &[u8] = unsafe { mem::transmute(vec.as_slice()) };
        try!(texture.update(None, slice, (display::COLS * PIXEL_SIZE) as int));
        Ok(())
    }

    fn is_key_pressed(&self, key: uint) -> bool {
        assert!(key < 16);
        (self.keys & 1 << key) >> key == 1
    }

    fn keydown(&mut self, key: uint) {
        assert!(key < 16);
        self.keys |= 1 << key;
    }

    fn keyup(&mut self, key: uint) {
        assert!(key < 16);
        self.keys &= !(1 << key);
        if self.blocked {
            self.blocked = false;
            *self.reg.get_mut(self.blocked_reg) = key as u8;
            self.blocked_reg = 255;
        }
    }
}

macro_rules! try {
    ($e:expr) => {
        match $e {
            Ok(inner) => inner,
            Err(e) => return Err(e)
        }
    };
    ($e:expr, $f:expr) => {
        match $e.map_err($f) {
            Ok(inner) => inner,
            Err(e) => return Err(e)
        }
    }
}

// FIXME: real error type I guess?
fn window() -> Result<Window, String> {
    use sdl2::video;

    Window::new("CHIP-8", video::PosCentered, video::PosCentered,
                WINDOW_WIDTH as int, WINDOW_HEIGHT as int, video::OpenGL)
}

fn renderer(win: Window) -> Result<Renderer<Window>, String> {
    use sdl2::render;
    Renderer::from_window(win, render::DriverAuto, render::Accelerated)
}

fn keymap() -> HashMap<KeyCode, uint> {
    let mut map = HashMap::new();
    map.insert(keycode::XKey,    0x0);
    map.insert(keycode::Num1Key, 0x1);
    map.insert(keycode::Num2Key, 0x2);
    map.insert(keycode::Num3Key, 0x3);
    map.insert(keycode::QKey,    0x4);
    map.insert(keycode::WKey,    0x5);
    map.insert(keycode::EKey,    0x6);
    map.insert(keycode::AKey,    0x7);
    map.insert(keycode::SKey,    0x8);
    map.insert(keycode::DKey,    0x9);
    map.insert(keycode::ZKey,    0xa);
    map.insert(keycode::CKey,    0xb);
    map.insert(keycode::Num4Key, 0xc);
    map.insert(keycode::RKey,    0xd);
    map.insert(keycode::FKey,    0xe);
    map.insert(keycode::VKey,    0xf);
    map
}

fn run_emulator(mut vm: Vm) -> Result<Vm, String> {
    use display::{ROWS, COLS};
    use sdl2::event;
    use sdl2::pixels::RGBA8888;
    use sdl2::render::AccessStreaming;
    use std::io::Timer;

    static CYCLES_PER_FRAME: u16 = 100;

    sdl2::init(sdl2::InitVideo);
    let win = try!(window());
    let renderer = try!(renderer(win));
    try!(renderer.clear());

    let texture = try!(renderer.create_texture(RGBA8888, AccessStreaming,
                                               COLS as int,
                                               ROWS as int));

    let keymap = keymap();

    let mut timer = Timer::new().unwrap();
    let sixty_hz = timer.periodic(1000 / 60); // not really 60 Hz...

    'main: loop {
        for _ in range(0, CYCLES_PER_FRAME) {
            if vm.blocked {
                break;
            }
            vm.tick();
        }

        match event::poll_event() {
            event::QuitEvent(_) => break 'main,
            sdl2::event::KeyDownEvent(_, _, key, _, _) => {
                if key == sdl2::keycode::EscapeKey {
                    break 'main;
                } else {
                    keymap.find_copy(&key).map(|code| vm.keydown(code));
                }
            },
            sdl2::event::KeyUpEvent(_, _, key, _, _) => {
                keymap.find_copy(&key).map(|code| vm.keyup(code));
            },
            _ => {}
        };
        sixty_hz.recv();
        try!(vm.render(&texture));
        try!(renderer.copy(&texture, None, None));
        renderer.present();
    }

    sdl2::quit();

    Ok(vm)
}

pub fn main() {
    use std::io::stdio;
    use std::io::File;
    use std::os;

    let mut stderr = stdio::stderr();

    let rom_path = match os::args().as_slice() {
        [] => { return; },
        [_] => {
            let _ = writeln!(stderr, "Usage: fries ROM");
            return;
        },
        [_, ref rom, ..] => Path::new(rom.clone())
    };

    let mut rom_file = File::open(&rom_path);
    let rom = match Rom::from_reader(&mut rom_file) {
        Ok(r) => r,
        Err(e) => {
            let _ = writeln!(stderr, "Error loading ROM: {}", e.desc);
            return;
        }
    };

    let rng = match StdRng::new() {
        Ok(r) => r,
        Err(e) => {
            let _ = writeln!(stderr, "Error creating RNG: {}", e.desc);
            return;
        }
    };

    let vm = Vm::new(rom, rng);
    match run_emulator(vm) {
        Err(e) => { let _ = writeln!(stderr, "Error: {}", e); },
        Ok(_) => {},
    }
}
