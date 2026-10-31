//! Image brute de l'écran → miniature réduite → JPEG (IMPL.md, 1.1).

use anyhow::Result;
use fast_image_resize::images::Image;
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};

/// Ordre des canaux de l'image source (4 octets par pixel).
#[derive(Clone, Copy, Debug)]
pub enum Order {
    #[cfg_attr(windows, allow(dead_code))]
    Bgra,
    Rgba,
}

pub struct Thumbnailer {
    width: u32,
    quality: u8,
    resizer: Resizer,
}

impl Thumbnailer {
    pub fn new(width: u32, quality: u8) -> Self {
        Self { width, quality, resizer: Resizer::new() }
    }

    /// `pixels` : image source contiguë (sans marge en fin de ligne).
    pub fn make(&mut self, w: u32, h: u32, pixels: &mut [u8], order: Order) -> Result<Vec<u8>> {
        let tw = self.width.min(w);
        let th = ((h as u64 * tw as u64) / w as u64).max(1) as u32;
        let src = Image::from_slice_u8(w, h, pixels, PixelType::U8x4)?;
        let mut dst = Image::new(tw, th, PixelType::U8x4);
        self.resizer.resize(&src, &mut dst, &ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Bilinear)))?;
        let mut jpeg = Vec::with_capacity(16 * 1024);
        let color = match order {
            Order::Bgra => jpeg_encoder::ColorType::Bgra,
            Order::Rgba => jpeg_encoder::ColorType::Rgba,
        };
        jpeg_encoder::Encoder::new(&mut jpeg, self.quality).encode(dst.buffer(), tw as u16, th as u16, color)?;
        Ok(jpeg)
    }
}

/// Copie une image dont les lignes ont une marge (`stride` > largeur × 4)
/// dans un tampon contigu.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn compact(src: &[u8], w: u32, h: u32, stride: usize) -> Vec<u8> {
    let row = w as usize * 4;
    let mut out = Vec::with_capacity(row * h as usize);
    for y in 0..h as usize {
        out.extend_from_slice(&src[y * stride..y * stride + row]);
    }
    out
}

/// Miniature de démonstration : fond de la couleur du poste, une
/// « fenêtre » claire et une bande qui avance à chaque image. Sert aux
/// essais sans écran réel.
pub fn synthetic(seed: usize, frame: u64, width: u32, quality: u8) -> Vec<u8> {
    let (w, h) = (width as usize, (width as usize * 9 / 16).max(1));
    let hue = (seed * 47 % 360) as f32;
    let (r, g, b) = hsv(hue, 0.45, 0.55);
    let bar = (frame as usize * 23) % w;
    let mut px = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        for x in 0..w {
            let in_window = (w / 8..w * 7 / 8).contains(&x) && (h / 6..h * 5 / 6).contains(&y);
            let c = if x >= bar && x < bar + 12 {
                (250, 250, 250)
            } else if in_window {
                (246, 247, 249)
            } else {
                (r, g, b)
            };
            px.extend_from_slice(&[c.0, c.1, c.2]);
        }
    }
    let mut out = Vec::new();
    let _ = jpeg_encoder::Encoder::new(&mut out, quality).encode(&px, w as u16, h as u16, jpeg_encoder::ColorType::Rgb);
    out
}

fn hsv(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    (((r + m) * 255.0) as u8, ((g + m) * 255.0) as u8, ((b + m) * 255.0) as u8)
}
