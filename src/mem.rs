use std::default::Default;
use std::io::IoResult;

pub static MEMORY_SIZE: u16 = 4096;
pub static ROM_LOC: u16 = 0x200;
pub static ROM_SIZE: u16 = MEMORY_SIZE - ROM_LOC;
pub static FONT_SPRITE_SIZE: uint = 5;
pub static FONT_SPRITES: uint = 16;
static FONT_LOC: uint = 0;

pub struct Memory {
    mem: [u8, ..MEMORY_SIZE]
}

impl Memory {
    pub fn new() -> Memory {
        Memory { mem: [0, ..MEMORY_SIZE as uint] }
    }

    pub fn load_rom(&mut self, rom: Rom) {
        let dst = self.mem.mut_slice_from(ROM_LOC as uint);
        dst.copy_from(rom.prgm.as_slice());
    }

    pub fn load_font(&mut self, sprites: &[u8]) {
        assert!(sprites.len() == FONT_SPRITE_SIZE * FONT_SPRITES);
        let dst = self.mem.mut_slice(FONT_LOC, FONT_SPRITE_SIZE * FONT_SPRITES);
        dst.copy_from(sprites);
    }

    pub fn get(&self, i: u16) -> u8 {
        self.mem[i as uint]
    }

    pub fn slice<'a>(&'a self, start: u16, end: u16) -> &'a [u8] {
        self.mem.slice(start as uint, end as uint)
    }

    pub fn mut_slice<'a>(&'a mut self, start: u16, end: u16) -> &'a mut [u8] {
        self.mem.mut_slice(start as uint, end as uint)
    }

    pub fn font_offset(&self, n: u8) -> u16 {
        let n: uint = (n & 0xf) as uint;
        (FONT_LOC + n * FONT_SPRITE_SIZE) as u16
    }
}

impl Default for Memory {
    fn default() -> Memory { Memory::new() }
}

pub struct Rom {
    prgm: [u8, ..ROM_SIZE]
}

impl Rom {
    pub fn from_reader(r: &mut Reader) -> IoResult<Rom> {
        let mut buf = [0u8, ..ROM_SIZE as uint];
        r.read(buf.as_mut_slice()).map(|_| Rom { prgm: buf  })
    }
}
