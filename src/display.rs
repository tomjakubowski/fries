static ROWS: uint = 32;

pub enum Pixel {
    On,
    Off
}

impl Pixel {
    #[inline]
    pub fn from_bool(b: bool) -> Pixel {
        if b { On } else { Off }
    }

    #[inline]
    pub fn is_on(&self) -> bool {
        match *self {
            On => true,
            _ => false
        }
    }

    #[inline]
    pub fn is_off(&self) -> bool {
        match *self {
            Off => true,
            _ => false
        }
    }
}

pub struct Display {
    p: [u64, ..ROWS]
}

impl Display {
    pub fn new() -> Display {
        Display {
            p: [0, ..ROWS as uint]
        }
    }

    pub fn pixels<'a>(&'a self) -> Pixels<'a> {
        Pixels {
            display: self,
            row_idx: 0,
            bit: 63,
        }
    }

    pub fn clear(&mut self) {
        self.p = [0, ..ROWS]
    }
}

pub struct Pixels<'a> {
    display: &'a Display,
    row_idx: uint,
    bit: uint, // 63, 62, 61, 60, ... 0
}

impl<'a> Iterator<Pixel> for Pixels<'a> {
    fn next(&mut self) -> Option<Pixel> {
        if self.row_idx >= ROWS {
            return None;
        }

        let row = self.display.p[self.row_idx];
        let shift = self.bit;
        let on = (row & (1 << shift)) >> shift == 1;

        if self.bit == 0 {
            self.row_idx += 1;
        }

        self.bit = (self.bit - 1) % 64;
        Some(Pixel::from_bool(on))
    }
}

#[cfg(test)]
mod test {
    use super::Display;

    #[test]
    fn test_pixels() {
        let mut d = Display::new();
        d.p[0] = 0b1111 << 60;
        d.p[1] = 0b00001111 << 56;
        let pixels = d.pixels();
        assert!(pixels.take(4).all(|x| x.is_on()));
        assert!(pixels.skip(4).take(60).all(|x| x.is_off()));
        assert!(pixels.skip(64).skip(4).take(4).all(|x| x.is_on()));
    }

    #[test]
    fn test_clear() {
        let mut d = Display::new();
        d.p[0] = 943853945;
        d.clear();
        assert!(d.pixels().all(|x| x.is_off()));
    }
}
