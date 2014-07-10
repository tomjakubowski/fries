use std::default::Default;
use std::fmt;

static REGISTERS: u8 = 16;

macro_rules! reg {
    ($i:ident $n:expr) => {
        #[allow(dead_code)] pub static $i: u8 = $n;
    }
}

reg! { V0 0x0 }
reg! { V1 0x1 }
reg! { V2 0x2 }
reg! { V3 0x3 }
reg! { V4 0x4 }
reg! { V5 0x5 }
reg! { V6 0x6 }
reg! { V7 0x7 }
reg! { V8 0x8 }
reg! { V9 0x9 }
reg! { VA 0xa }
reg! { VB 0xb }
reg! { VC 0xc }
reg! { VD 0xd }
reg! { VE 0xe }
reg! { VF 0xf }

pub struct Registers {
    regs: [u8, ..REGISTERS]
}

impl Default for Registers {
    fn default() -> Registers { Registers { regs: [0, ..REGISTERS as uint] } }
}

impl Registers {
    pub fn get(&self, i: u8) -> u8 {
        self.regs[i as uint]
    }

    pub fn get_mut<'a>(&'a mut self, i: u8) -> &'a mut u8 {
        assert!(i < REGISTERS);
        unsafe { self.regs.as_mut_slice().unsafe_mut_ref(i as uint) }
    }

    pub fn set_flag(&mut self, val: u8) {
        self.regs[VF as uint] = val;
    }
}

impl fmt::Show for Registers {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut first = true;
        for r in self.regs.as_slice().iter() {
            if !first {
                try!(write!(f, " "));
            }
            try!(write!(f, "{:02x}", *r));
            first = false;
        }
        Ok(())
    }
}
