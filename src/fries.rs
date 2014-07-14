#![crate_type="bin"]
#![feature(concat_idents, macro_rules, phase)]

#[phase(plugin, link)] extern crate log;

extern crate rsfml;

use rsfml::graphics::{RenderWindow, Texture};
use rsfml::window::keyboard;
use rsfml::window::keyboard::Key;

use std::collections::TreeMap;
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

static FONT: [u8, ..mem::FONT_SPRITE_SIZE * mem::FONT_SPRITES] = [
    0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
    0x20, 0x60, 0x20, 0x20, 0x70, // 1
    0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
    0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
    0x90, 0x90, 0xF0, 0x10, 0x10, // 4
    0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
    0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
    0xF0, 0x10, 0x20, 0x40, 0x40, // 7
    0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
    0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
    0xF0, 0x90, 0xF0, 0x90, 0x90, // A
    0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
    0xF0, 0x80, 0x80, 0x80, 0xF0, // C
    0xE0, 0x90, 0x90, 0x90, 0xE0, // D
    0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
    0xF0, 0x80, 0xF0, 0x80, 0x80  // F
];

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
        mem.load_font(FONT);

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

    fn misc(&mut self, x: u8, nn: u8) {
        match nn {
            0x07 => { // set register from delay timer
                *self.reg.get_mut(x) = self.dt;
            },
            0x0a => { // wait for keypress
                self.blocked_reg = x;
                self.blocked = true;
            },
            0x15 => {
                self.dt = self.reg.get(x);
            },
            0x18 => {
                self.st = self.reg.get(x);
            },
            0x29 => {
                self.i = self.mem.font_offset(self.reg.get(x));
            },
            0x33 => { // set [I, I+1, I+2] to BCD repr of VX
                let val = self.reg.get(x);
                let dst: &mut [u8] = self.mem.mut_slice(self.i, self.i + 3);
                dst[0] = (val / 100) % 10;
                dst[1] = (val / 10) % 10;
                dst[2] = val % 10;
            },
            0x1e => {
                self.i += self.reg.get(x) as u16;
            },
            0x55 => { // store registers to memory
                let new_i = self.i + x as u16 + 1;
                let dst: &mut [u8] = self.mem.mut_slice(self.i, new_i);
                let src: &[u8] = self.reg.slice(0, x + 1);
                dst.copy_from(src);
                self.i = new_i;
            },
            0x65 => { // load registers from memory
                let new_i = self.i + x as u16 + 1;
                let dst: &mut [u8] = self.reg.mut_slice(0, x + 1);
                let src: &[u8] = self.mem.slice(self.i, new_i);
                dst.copy_from(src);
                self.i = new_i;
            },
            _ => {
                fail!("f{:01x}{:02x} not implemented", x, nn)
            }
        }
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
            0x3 => { // skip if VX eq NN
                if self.reg.get(x) == nn {
                    self.pc += 2;
                }
            },
            0x4 => { // skip if VX ne NN
                if self.reg.get(x) != nn {
                    self.pc += 2;
                }
            },
            0x5 => { // skip if VX == VY
                if self.reg.get(x) == self.reg.get(y) {
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
            0x9 => { // skip if VX != VY
                if self.reg.get(x) != self.reg.get(y) {
                    self.pc += 2
                }
            },
            0xa => { // set index register
                self.i = nnn;
            },
            0xb => { // jump to nnn + v0
                self.pc = nnn + self.reg.get(0) as u16;
            },
            0xc => { // random number
                *self.reg.get_mut(x) = self.rng.gen::<u8>() & nn;
            }
            0xd => { // draw sprite
                let sprite = self.mem.slice(self.i, self.i + (n as u16));
                let (vx, vy) = (self.reg.get(x), self.reg.get(y));
                let flag = if self.display.draw(sprite, vx, vy) { 0x1 } else { 0x0 };
                self.reg.set_flag(flag);
            },
            0xe if nn == 0x9e => { // skip if key in VX is pressed
                if self.is_key_pressed(self.reg.get(x) as uint) {
                    self.pc += 2;
                }
            },
            0xe if nn == 0xa1 => { // skip if key in VX is not pressed
                if !self.is_key_pressed(self.reg.get(x) as uint) {
                    self.pc += 2;
                }
            },
            0xf => {
                self.misc(x, nn);
            },
            _ => fail!("{:04x} not yet implemented", ins)
        }
    }

    fn render(&mut self, texture: &mut Texture) {
        if self.dt > 0 { self.dt -= 1 }
        if self.st > 0 { self.st -= 1 }

        let on: [u8, ..4]  = [0xff, 0xcc, 0x00, 0xff];
        let off: [u8, ..4] = [0x99, 0x66, 0x00, 0xff];

        let vec: Vec<u8> = self.display.pixels().flat_map(|px| {
            if px.is_on() { on.iter() } else { off.iter() }
        }).map(|&x| x).collect();
        texture.update_from_pixels(vec.as_slice(), display::COLS, display::ROWS, 0, 0);
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

// FIXME: real error type I guess?
fn window() -> Result<RenderWindow, String> {
    use rsfml::graphics::RenderWindow;
    use rsfml::window::{Close, ContextSettings, VideoMode};
    let settings = ContextSettings::default();
    match RenderWindow::new(VideoMode::new_init(WINDOW_WIDTH, WINDOW_HEIGHT, 32),
                            "CHIP-8",
                            Close,
                            &settings) {
        Some(window) => Ok(window),
        None => Err("Could not create RenderWindow.".to_string())
    }
}

fn texture() -> Result<Texture, String> {
    match Texture::new(display::COLS as uint, display::ROWS as uint) {
        Some(texture) => Ok(texture),
        None => Err("Could not create texture.".to_string())
    }
}

fn keymap() -> TreeMap<Key, uint> {
    let mut map = TreeMap::new();
    map.insert(keyboard::X,    0x0);
    map.insert(keyboard::Num1, 0x1);
    map.insert(keyboard::Num2, 0x2);
    map.insert(keyboard::Num3, 0x3);
    map.insert(keyboard::Q,    0x4);
    map.insert(keyboard::W,    0x5);
    map.insert(keyboard::E,    0x6);
    map.insert(keyboard::A,    0x7);
    map.insert(keyboard::S,    0x8);
    map.insert(keyboard::D,    0x9);
    map.insert(keyboard::Z,    0xa);
    map.insert(keyboard::C,    0xb);
    map.insert(keyboard::Num4, 0xc);
    map.insert(keyboard::R,    0xd);
    map.insert(keyboard::F,    0xe);
    map.insert(keyboard::V,    0xf);
    map
}

fn run_emulator(mut vm: Vm) -> Result<Vm, String> {
    use std::io::Timer;
    use rsfml::graphics::Sprite;

    static CYCLES_PER_FRAME: u16 = 100;

    let mut win = try!(window());
    let mut texture = try!(texture());
    let keymap = keymap();

    let mut timer = Timer::new().unwrap();
    let sixty_hz = timer.periodic(1000 / 60); // not really 60 Hz...

    'main: loop {
        use rsfml::window::{event, keyboard};

        for _ in range(0, CYCLES_PER_FRAME) {
            if vm.blocked {
                break;
            }
            vm.tick();
        }

        match win.poll_event() {
            event::Closed => break 'main,
            event::KeyPressed { code: key, .. } => {
                if key == keyboard::Escape {
                    break 'main;
                } else {
                    keymap.find(&key).map(|&code| vm.keydown(code));
                }
            },
            event::KeyReleased { code: key, .. }=> {
                keymap.find(&key).map(|&code| vm.keyup(code));
            },
            _ => {}
        };
        sixty_hz.recv();
        vm.render(&mut texture);
        let mut sprite = Sprite::new_with_texture(&texture).unwrap(); // FIXME
        sprite.scale2f(10., 10.);
        win.draw(&sprite);
        win.display();
    }

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
