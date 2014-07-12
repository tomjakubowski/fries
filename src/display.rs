use std::fmt;

pub static COLS: uint = 64;
pub static ROWS: uint = 32;
pub static MAX_SPRITE_HEIGHT: uint = 15;

pub enum Pixel {
    On,
    Off
}

impl Pixel {
    #[inline]
    fn from_bool(b: bool) -> Pixel {
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

impl fmt::Show for Pixel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", if self.is_on() { '█' } else { ' ' })
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

    pub fn draw(&mut self, sprite: &[u8], x: u8, y: u8) {
        debug_assert!(sprite.len() <= MAX_SPRITE_HEIGHT);
        let (x, mut y) = (x as uint % COLS, y as uint % ROWS);
        for sprite in sprite.iter() {
            let sprite: u64 = (*sprite as u64) << (64 - 8);
            self.p[y] ^= sprite >> x;
            if x != 0 { self.p[y] ^= sprite << (64 - x); }
            y = (y + 1) % ROWS;
        }
    }

    pub fn clear(&mut self) {
        self.p = [0, ..ROWS]
    }
}

impl fmt::Show for Display {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let bar = String::from_char(64, '-');
        try!(writeln!(f, "+{}+", bar));
        for (i, px) in self.pixels().enumerate() {
            if i % 64 == 0 {
                try!(write!(f, "|"));
            }
            try!(write!(f, "{}", px));
            if i % 64 == 63 {
                try!(writeln!(f, "|"));
            }
        }
        writeln!(f, "+{}+", bar)
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

    #[test]
    fn test_draw() {
        let mut d = Display::new();
        let sprite = [0b11111111];
        d.draw(sprite.as_slice(), 0, 0);
        assert!(d.pixels().take(8).all(|x| x.is_on()));
        assert!(d.pixels().skip(8).all(|x| x.is_off()));
        d.draw(sprite.as_slice(), 0, 0);
        assert!(d.pixels().all(|x| x.is_off()));
    }

    #[test]
    fn test_draw_wrapping_cols() {
        use super::{COLS};
        let mut d = Display::new();
        let sprite = [0b11111111];
        d.draw(sprite.as_slice(), COLS as u8 - 1, 0);
        println!("{}", d);
        assert!(d.pixels().take(7).all(|x| x.is_on()));
        assert!(d.pixels().skip(63).take(1).all(|x| x.is_on()));
        assert!(d.pixels().skip(7).take(56).all(|x| x.is_off()));
    }

    #[test]
    fn test_draw_wrapping_rows() {
        use super::{ROWS};
        let mut d = Display::new();
        let sprite = [0b11111111, 0b11111111];
        d.draw(sprite.as_slice(), 0, ROWS as u8 - 1);
        println!("{}", d);
        assert!(d.pixels().take(8).all(|x| x.is_on()));
        assert!(d.pixels().skip(64 * (ROWS - 1)).take(8).all(|x| x.is_on()));
    }

    #[test]
    fn smoke_test_draw() {
        let mut d = Display::new();
        let sprite = [0b00100100,
                      0b00100100,
                      0b00000000,
                      0b10000001,
                      0b01000010,
                      0b00111100];
        d.draw(sprite.as_slice(), 0, 0);
        println!("{}", d);
    }
}
