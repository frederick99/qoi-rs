use image::{Rgba, RgbaImage};

macro_rules! wrapping {
    ($lhs:tt + $rhs:tt $($rest:tt)*) => {
        wrapping!($lhs).wrapping_add(wrapping!($rhs $($rest)*))
    };
    ($lhs:tt - $rhs:tt $($rest:tt)*) => {
        wrapping!($lhs).wrapping_sub(wrapping!($rhs $($rest)*))
    };
    ($lhs:tt * $rhs:tt $($rest:tt)*) => {
        wrapping!(($lhs.wrapping_mul($rhs)) $($rest)*)
    };
    ($els:tt) => {
        $els
    };
}

#[derive(Debug, Clone, Copy)]
pub enum QoiOp {
    RGB { r: u8, g: u8, b: u8 },
    RGBA { r: u8, g: u8, b: u8, a: u8 },
    Index { idx: u8 },                     // 6-bit index
    Diff { dr: u8, dg: u8, db: u8 },       // 2-bit differences, bias of 2
    Luma { dg: u8, dr_dg: u8, db_dg: u8 }, // dg - 6-bit (bias of 32), dr_dg and db_dg - 4-bit (bias of 8)
    Run { len: u8 },                       // 6-bit, in [1..62] with bias of -1
}

impl QoiOp {
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            &QoiOp::RGB { r, g, b } => vec![0b11111110, r, g, b],
            &QoiOp::RGBA { r, g, b, a } => vec![0b11111111, r, g, b, a],
            &QoiOp::Index { idx } => {
                assert!(idx <= 62);
                vec![(0b00 << 6) | idx]
            }
            &QoiOp::Diff { dr, dg, db } => {
                assert!(dr <= 3 && dg <= 3 && db <= 3);
                vec![(0b01 << 6) | (dr << 4) | (dg << 2) | (db << 0)]
            }
            &QoiOp::Luma { dg, dr_dg, db_dg } => {
                assert!(dg < 64 && dr_dg < 16 && db_dg < 16);
                vec![(0b10 << 6) | dg, (dr_dg << 4) | db_dg]
            }
            &QoiOp::Run { len } => {
                assert!(len <= 62);
                vec![(0b11 << 6) | (len - 1)]
            }
        }
    }

    fn from_bytes_impl(buf: &[u8]) -> Option<(Self, &[u8])> {
        let (head, rest) = buf.split_first()?;
        match head {
            0b11111110 => {
                let (&r, rest) = rest.split_first()?;
                let (&g, rest) = rest.split_first()?;
                let (&b, rest) = rest.split_first()?;
                Some((QoiOp::RGB { r, g, b }, rest))
            }
            0b11111111 => {
                let (&r, rest) = rest.split_first()?;
                let (&g, rest) = rest.split_first()?;
                let (&b, rest) = rest.split_first()?;
                let (&a, rest) = rest.split_first()?;
                Some((QoiOp::RGBA { r, g, b, a }, rest))
            }
            &x if x >> 6 == 0b00 => {
                let idx = x & 0b00111111;
                Some((QoiOp::Index { idx }, rest))
            }
            &x if x >> 6 == 0b01 => {
                let dr = (x >> 4) & 0b11;
                let dg = (x >> 2) & 0b11;
                let db = (x >> 0) & 0b11;
                Some((QoiOp::Diff { dr, dg, db }, rest))
            }
            &x if x >> 6 == 0b10 => {
                let dg = x & 0b00111111;
                let (next, rest) = rest.split_first()?;
                let dr_dg = next >> 4;
                let db_dg = next & 0b1111;
                Some((QoiOp::Luma { dg, dr_dg, db_dg }, rest))
            }
            &x if x >> 6 == 0b11 => {
                let len = (x & 0b00111111) + 1;
                Some((QoiOp::Run { len }, rest))
            }
            _ => unreachable!()
        }
    }

    pub fn from_bytes(buf: &[u8]) -> (Option<Self>, &[u8]) {
        match Self::from_bytes_impl(buf) {
            Some((op, rest)) => (Some(op), rest),
            None => (None, buf),
        }
    }
}

type Pixel = Rgba<u8>;

fn hash(pixel: &Pixel) -> u8 {
    // (pixel.0[0] * 3 + pixel.0[1] * 5 + pixel.0[2] * 7 + pixel.0[3] * 11) % 64
    let &Rgba::<u8>([r, g, b, a]) = pixel;
    (r.wrapping_mul(3).wrapping_add(
        g.wrapping_mul(5)
            .wrapping_add(b.wrapping_mul(7).wrapping_add(a.wrapping_mul(11))),
    )) % 64
}

struct Encoder {
    width: u32,
    height: u32,
    channels: u8,
    colorspace: u8,
    cache: [Pixel; 64],
    prev: Pixel,
}

impl Encoder {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            channels: 4,
            colorspace: 0,
            cache: [Pixel::from([0, 0, 0, 255]); 64],
            prev: Pixel::from([0, 0, 0, 255]),
        }
    }

    fn header(&self) -> Vec<u8> {
        let mut v = vec![b'q', b'o', b'i', b'f'];
        v.extend(self.width.to_be_bytes());
        v.extend(self.height.to_be_bytes());
        v.push(self.channels);
        v.push(self.colorspace);
        v
    }

    fn encode(&mut self, img: &RgbaImage) -> Vec<u8> {
        let mut buf = vec![];

        // header
        buf.extend(self.header());

        let mut is_running = false;
        let mut run_length = 0;
        let mut ops = Vec::<QoiOp>::new();

        // body
        for pixel in img.pixels() {
            let prev = self.prev;
            self.prev = *pixel;
            let &Rgba::<u8>([r, g, b, a]) = pixel;
            let &Rgba::<u8>([pr, pg, pb, pa]) = &prev;

            if is_running {
                if prev.eq(pixel) {
                    if run_length >= 62 {
                        ops.push(QoiOp::Run { len: 62 });
                        run_length -= 62;
                    }
                    run_length += 1;
                    continue;
                } else {
                    is_running = false;
                    if run_length > 0 {
                        ops.push(QoiOp::Run { len: run_length });
                    }
                }
            }

            if prev.eq(pixel) {
                assert!(!is_running);
                is_running = true;
                run_length = 1;
                continue;
            }

            let h = hash(pixel);

            if self.cache[h as usize].eq(pixel) {
                ops.push(QoiOp::Index { idx: h });
                continue;
            }

            let dr = r.wrapping_sub(pr).wrapping_add(2);
            let dg = g.wrapping_sub(pg).wrapping_add(2);
            let db = b.wrapping_sub(pb).wrapping_add(2);
            let da = a.wrapping_sub(pa);

            if da == 0 && 0 <= dr && dr <= 3 && 0 <= dg && dg <= 3 && 0 <= db && db <= 3 {
                ops.push(QoiOp::Diff {
                    dr: dr as u8,
                    dg: dg as u8,
                    db: db as u8,
                });
                continue;
            }

            let dg = wrapping!(g - pg);
            let dr = wrapping!(r - pr);
            let db = wrapping!(b - pb);
            let dr_dg = wrapping!(8u8 + dr - dg);
            let db_dg = wrapping!(8u8 + db - dg);
            let dg = wrapping!(32u8 + dg);

            if da == 0 && 0 <= dg && dg < 64 && 0 <= dr_dg && dr_dg < 16 && 0 <= db_dg && db_dg < 16
            {
                ops.push(QoiOp::Luma { dg, dr_dg, db_dg, });
                continue;
            }

            if da == 0 {
                ops.push(QoiOp::RGB { r, g, b });
            } else {
                ops.push(QoiOp::RGBA { r, g, b, a });
            }
        }

        if is_running {
            ops.push(QoiOp::Run { len: run_length });
        }

        for op in ops {
            // println!("Encoded op: {:?}", op);
            buf.extend(op.to_bytes());
        }

        // footer
        buf.extend_from_slice(&[0u8, 0, 0, 0, 0, 0, 0, 1]);

        buf
    }
}

struct Decoder {
    cache: [Pixel; 64],
    prev: Pixel,
}

impl Decoder {
    pub fn new() -> Self {
        Self {
            cache: [Pixel::from([0, 0, 0, 255]); 64],
            prev: Rgba::<u8>([0, 0, 0, 255]),
        }
    }

    pub fn decode(&mut self, data: &[u8]) -> Option<RgbaImage> {
        // header
        let (magic, data) = data.split_at_checked(4)?;
        if !magic.eq(b"qoif") {
            return None;
        }

        let (width_bytes, data) = data.split_first_chunk::<4>()?;
        let width = u32::from_be_bytes(*width_bytes);
        let (height_bytes, data) = data.split_first_chunk::<4>()?;
        let height = u32::from_be_bytes(*height_bytes);

        let (_channels, data) = data.split_first()?;
        let (_colorspace, data) = data.split_first()?;

        // body
        let mut data = data;
        let mut buf = Vec::<u8>::with_capacity((width * height * 4) as usize);
        loop {
            let (op, rest) = QoiOp::from_bytes(data);
            // println!("Decoded op: {:?}", op?);
            let mut count: u8 = 1;
            let pixel = match op? {
                QoiOp::RGB { r, g, b } => {
                    let a = self.prev.0[3];
                    Rgba::<u8>([r, g, b, a])
                }
                QoiOp::RGBA { r, g, b, a } => {
                    Rgba::<u8>([r, g, b, a])
                }
                QoiOp::Index { idx } => {
                    *self.cache.get(idx as usize)?
                }
                QoiOp::Diff { dr, dg, db } => {
                    let Rgba::<u8>([pr, pg, pb, a]) = self.prev;
                    let r = wrapping!{ pr + dr - 2 };
                    let g = wrapping!{ pg + dg - 2 };
                    let b = wrapping!{ pb + db - 2 };
                    Rgba::<u8>([r, g, b, a])
                }
                QoiOp::Luma { dg, dr_dg, db_dg } => {
                    let dg = wrapping!{ dg - 32 };
                    let dr = wrapping!{ dr_dg + dg - 8 };
                    let db = wrapping!{ db_dg + dg - 8 };
                    let Rgba::<u8>([pr, pg, pb, a]) = self.prev;
                    let r = wrapping!{ pr + dr };
                    let g = wrapping!{ pg + dg };
                    let b = wrapping!{ pb + db };
                    Rgba::<u8>([r, g, b, a])
                }
                QoiOp::Run { len } => {
                    count = len;
                    self.prev
                }
            };
            self.prev = pixel;
            self.cache[hash(&pixel) as usize] = pixel;
            for _ in 0..count {
                buf.extend_from_slice(&pixel.0);
            }
            data = rest;
            if buf.len() >= (width * height * 4) as usize {
                break;
            }
        }

        // footer
        if ![0u8, 0, 0, 0, 0, 0, 0, 1].eq(data) {
            return None;
        }

        RgbaImage::from_vec(width, height, buf)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    #[test]
    fn test() -> Result<(), Box<dyn std::error::Error>> {
        use super::*;

        let now = Instant::now();
        let img = image::ImageReader::open("assets/suz.png")?.decode()?;
        println!("PNG decoder took {} us", now.elapsed().as_micros());

        let mut encoder = Encoder::new(img.width(), img.height());

        let img_buf = img.as_rgba8().unwrap();

        let now = Instant::now();
        let data = encoder.encode(img_buf);
        std::fs::write("test.qoi", &data)?;
        println!("QOI encoder took {} us", now.elapsed().as_micros());

        let now = Instant::now();
        img.save("test.png")?;
        println!("PNG encoder took {} us", now.elapsed().as_micros());

        let now = Instant::now();
        let mut decoder = Decoder::new();
        let data = std::fs::read("test.qoi")?;
        let decoded = decoder.decode(&data).ok_or("failed to decode")?;
        println!("QOI decoder took {} us", now.elapsed().as_micros());

        assert!(&decoded.as_raw().eq(img_buf.as_raw()));
        decoded.save("test2.png")?;

        Ok(())
    }

    #[test]
    fn test_wrapping_macro() {
        assert_eq!(wrapping! { 50u8 * 6u8 }, 44);
        assert_eq!(wrapping! { 250u8 + 6u8 }, 0);
        assert_eq!(wrapping! { 50u8 * 6u8 + 1 }, 45);
        assert_eq!(wrapping! { 50u8 * 6u8 + 34u8 + 69u8 * 8u8 }, 118);
    }
}
