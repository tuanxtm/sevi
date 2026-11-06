//! Desktop download-page compositions and the release disk-image background.
use crate::{Args, Result};
use image::{
    Rgba, RgbaImage,
    imageops::{self, FilterType::Lanczos3},
};
use std::{fs::File, io::BufWriter, path::Path};
const W: u32 = 2400;
const H: u32 = 1800;

pub fn dmg(a: &Args<'_>) -> Result {
    let output = a.args.first().ok_or("dmg-background <output.png>")?;
    let mut pixels = vec![0; 1320 * 840 * 3];
    for y in 0..840i32 {
        for x in 0..1320i32 {
            let arrow = (524..=796).contains(&x) && (375..=386).contains(&y)
                || (348..=412).contains(&y)
                    && (760..=804 - ((y - 380).abs() * 44 + 16) / 32).contains(&x);
            let color = if arrow {
                [165, 170, 185]
            } else {
                [246, 247, 250]
            };
            let i = (y as usize * 1320 + x as usize) * 3;
            pixels[i..i + 3].copy_from_slice(&color);
        }
    }
    let mut encoder = png::Encoder::new(BufWriter::new(File::create(output)?), 1320, 840);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_pixel_dims(Some(png::PixelDimensions {
        xppu: 5669,
        yppu: 5669,
        unit: png::Unit::Meter,
    }));
    encoder.write_header()?.write_image_data(&pixels)?;
    Ok(())
}
fn open(p: impl AsRef<Path>) -> Result<RgbaImage> {
    Ok(image::open(p)?.into_rgba8())
}
fn resize(im: &RgbaImage, scale: f64) -> Result<RgbaImage> {
    let w = (im.width() as f64 * scale) as u32;
    let h = (im.height() as f64 * scale) as u32;
    if w == 0 || h == 0 || !scale.is_finite() || scale <= 0. {
        return Err("invalid image scale".into());
    }
    Ok(imageops::resize(im, w, h, Lanczos3))
}
fn cover(path: &str) -> Result<RgbaImage> {
    let mut im = open(path)?;
    for p in im.pixels_mut() {
        p[3] = 255;
    }
    let s = (W as f64 / im.width() as f64).max(H as f64 / im.height() as f64);
    let im = imageops::resize(
        &im,
        (im.width() as f64 * s) as u32 + 1,
        (im.height() as f64 * s) as u32 + 1,
        Lanczos3,
    );
    Ok(imageops::crop_imm(&im, (im.width() - W) / 2, (im.height() - H) / 2, W, H).to_image())
}
fn bbox(s: &str) -> Result<[i32; 4]> {
    let v: Vec<i32> = s
        .split(',')
        .map(str::parse)
        .collect::<std::result::Result<_, _>>()?;
    v.try_into().map_err(|_| "expected l,t,r,b".into())
}
fn crop(im: &RgbaImage, b: [i32; 4]) -> Result<RgbaImage> {
    let [l, t, r, b] = b;
    if r <= l || b <= t {
        return Err("empty crop".into());
    }
    let mut out = RgbaImage::new((r - l) as u32, (b - t) as u32);
    imageops::overlay(&mut out, im, -i64::from(l), -i64::from(t));
    Ok(out)
}
fn rounded(im: &RgbaImage, radius: f64) -> (RgbaImage, RgbaImage) {
    let mut pic = im.clone();
    let mut mask = RgbaImage::new(im.width(), im.height());
    let r = radius
        .min(im.width().min(im.height()).saturating_sub(1) as f64 / 2.)
        .max(0.)
        .floor() as usize;
    let mut edge = vec![0usize; r + 1];
    let (mut cx, mut cy) = (r as i64, 0i64);
    loop {
        edge[cy as usize] = edge[cy as usize].max(cx as usize);
        if cx == 0 && cy == r as i64 {
            break;
        }
        let error = |(x, y): (i64, i64)| (x * x + y * y - (r * r) as i64).abs();
        let mut next = (cx, cy + 1);
        if cx > 0 {
            for candidate in [(cx - 1, cy + 1), (cx - 1, cy)] {
                if error(candidate) < error(next) {
                    next = candidate;
                }
            }
        }
        (cx, cy) = next;
    }
    for (x, y, p) in pic.enumerate_pixels_mut() {
        let row = y.min(im.height() - 1 - y) as usize;
        let inset = if row < r { r - edge[r - row] } else { 0 };
        let inside = x as usize >= inset && (x as usize) < im.width() as usize - inset;
        if !inside {
            *p = Rgba([0, 0, 0, 0]);
        }
        *mask.get_pixel_mut(x, y) = if inside {
            Rgba([10, 10, 30, 110])
        } else {
            Rgba([0, 0, 0, 0])
        };
    }
    (pic, mask)
}
fn drop(bg: &mut RgbaImage, pic: &RgbaImage, mask: &RgbaImage, x: i64, y: i64) {
    let mut shadow = RgbaImage::new(pic.width() + 300, pic.height() + 300);
    imageops::replace(&mut shadow, mask, 150, 186);
    let shadow = gaussian(shadow, 50.);
    composite(bg, &shadow, x - 150, y - 150);
    composite(bg, pic, x, y);
}
// Compositions start on an opaque RGB wallpaper. Preserve integer rounding at
// each alpha-composite step rather than truncating floating-point channels.
fn composite(bg: &mut RgbaImage, pic: &RgbaImage, left: i64, top: i64) {
    for (x, y, p) in pic.enumerate_pixels() {
        let (x, y) = (i64::from(x) + left, i64::from(y) + top);
        if x < 0 || y < 0 || x >= i64::from(bg.width()) || y >= i64::from(bg.height()) {
            continue;
        }
        let b = bg.get_pixel_mut(x as u32, y as u32);
        let alpha = u32::from(p[3]);
        for channel in 0..3 {
            let total = u32::from(p[channel]) * alpha + u32::from(b[channel]) * (255 - alpha) + 128;
            b[channel] = ((total + (total >> 8)) >> 8) as u8;
        }
        b[3] = 255;
    }
}
// Preserve Pillow's three fractional box passes and fixed-point rounding.
// Formula: https://github.com/python-pillow/Pillow/blob/main/src/libImaging/BoxBlur.c
fn gaussian(mut im: RgbaImage, sigma: f32) -> RgbaImage {
    let variance = sigma * sigma / 3.;
    let l = (((12. * f64::from(variance) + 1.).sqrt() - 1.) / 2.).floor() as f32;
    let fraction =
        ((2. * l + 1.) * (l * (l + 1.) - 3. * variance)) / (6. * (variance - (l + 1.) * (l + 1.)));
    let radius = l + fraction;
    let r = radius as usize;
    let weight = ((1u32 << 24) as f32 / (radius * 2. + 1.)) as u64;
    let far_weight = ((1u64 << 24) - (r as u64 * 2 + 1) * weight) / 2;
    let w = im.width() as usize;
    let h = im.height() as usize;
    for vertical in [false, true] {
        let (lines, length) = if vertical { (w, h) } else { (h, w) };
        for _ in 0..3 {
            let mut out = vec![0u8; w * h * 4];
            let input = im.as_raw();
            for line in 0..lines {
                let index = |x: usize| {
                    if vertical {
                        (x * w + line) * 4
                    } else {
                        (line * w + x) * 4
                    }
                };
                for channel in 0..4 {
                    let pixel = |x: isize| {
                        u64::from(input[index(x.clamp(0, length as isize - 1) as usize) + channel])
                    };
                    let mut sum = (0..=r * 2)
                        .map(|n| pixel(n as isize - r as isize))
                        .sum::<u64>();
                    for x in 0..length {
                        let x = x as isize;
                        let r = r as isize;
                        let total =
                            sum * weight + (pixel(x - r - 1) + pixel(x + r + 1)) * far_weight;
                        out[index(x as usize) + channel] = ((total + (1 << 23)) >> 24) as u8;
                        sum = sum + pixel(x + r + 1) - pixel(x - r);
                    }
                }
            }
            im = RgbaImage::from_raw(w as u32, h as u32, out).unwrap();
        }
    }
    im
}
fn save(im: RgbaImage, out: &str) -> Result {
    let rgb = image::DynamicImage::ImageRgba8(im).into_rgb8();
    let encoder = webp::Encoder::from_rgb(rgb.as_raw(), rgb.width(), rgb.height());
    let mut config = webp::WebPConfig::new().map_err(|_| "WebP configuration unavailable")?;
    config.quality = 88.;
    config.method = 6;
    let bytes = encoder
        .encode_advanced(&config)
        .map_err(|e| format!("WebP encoding: {e:?}"))?;
    std::fs::write(out, &*bytes)?;
    Ok(())
}
pub fn shots(a: &Args<'_>) -> Result {
    let get = |i| {
        a.args
            .get(i)
            .map(String::as_str)
            .ok_or("dl-shots: missing argument")
    };
    match get(0)? {
        "desktop" => {
            let mut win = crop(&open(get(1)?)?, bbox(get(2)?)?)?;
            let radius: f64 = a.args.get(5).map(|v| v.parse()).transpose()?.unwrap_or(14.);
            let scale: f64 = a.args.get(6).map(|v| v.parse()).transpose()?.unwrap_or(1.);
            if scale != 1. {
                win = resize(&win, scale)?;
            }
            let s = 1f64
                .min(1920. / win.width() as f64)
                .min(1512. / win.height() as f64);
            if s < 1. {
                win = resize(&win, s)?;
            }
            let (pic, mask) = rounded(&win, (radius * 2. * scale * s).floor());
            let mut bg = cover(get(3)?)?;
            drop(
                &mut bg,
                &pic,
                &mask,
                (W as i64 - win.width() as i64) / 2,
                (H as i64 - win.height() as i64) / 2,
            );
            save(bg, get(4)?)?;
            println!("{} ({}, {})", get(4)?, win.width(), win.height());
        }
        "scene" => {
            let b = bbox(get(2)?)?;
            let im = crop(&open(get(1)?)?, b)?;
            save(imageops::resize(&im, W, H, Lanczos3), get(3)?)?;
            println!("{} ({}, {}, {}, {})", get(3)?, b[0], b[1], b[2], b[3]);
        }
        "macdesk" => {
            let shot = open(get(1)?)?;
            let bar_h: i32 = get(2)?.parse()?;
            let mut bg = cover(get(4)?)?;
            let left = resize(&crop(&shot, [0, 0, 600, bar_h])?, 2.)?;
            let right = resize(
                &crop(
                    &shot,
                    [shot.width() as i32 - 600, 0, shot.width() as i32, bar_h],
                )?,
                2.,
            )?;
            let dock = crop(&shot, bbox(get(3)?)?)?;
            let dock = resize(&dock, 1.6f64.min((W - 40) as f64 / dock.width() as f64))?;
            let win = open(get(5)?)?;
            let maxh = H as i64 - i64::from(bar_h) * 2 - i64::from(dock.height()) - 140;
            let s = 1f64.min(maxh as f64 / win.height() as f64);
            let win = resize(&win, s)?;
            let (pic, mask) = rounded(&win, (22. * s).floor());
            drop(
                &mut bg,
                &pic,
                &mask,
                (W as i64 - win.width() as i64) / 2,
                i64::from(bar_h) * 2 + (maxh - i64::from(win.height())) / 2 + 60,
            );
            composite(&mut bg, &left, 0, 0);
            composite(&mut bg, &right, 1200, 0);
            composite(
                &mut bg,
                &dock,
                (W as i64 - dock.width() as i64) / 2,
                H as i64 - dock.height() as i64 - 8,
            );
            save(bg, get(6)?)?;
            println!(
                "{} ({}, {}) ({}, {})",
                get(6)?,
                win.width(),
                win.height(),
                dock.width(),
                dock.height()
            );
        }
        _ => return Err("dl-shots desktop|scene|macdesk <arguments>".into()),
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    #[test]
    fn shadow_blur_matches_original_pillow_pixels() {
        let image = RgbaImage::from_fn(10, 9, |x, y| {
            Rgba([
                (x * 19 % 256) as u8,
                (y * 31 % 256) as u8,
                ((x + y) * 7 % 256) as u8,
                ((x * 11 + y * 23) % 256) as u8,
            ])
        });
        let actual = gaussian(image, 2.5);
        assert_eq!(
            format!("{:x}", Sha256::digest(actual.as_raw())),
            "657db109efeff264b0a632fba8ae211e8ec0d9bc42b31c5c705d7ee953e7c13e"
        );
    }
    #[test]
    fn window_corners_match_original_pillow_mask() {
        let (pic, _) = rounded(&RgbaImage::from_pixel(200, 150, Rgba([255; 4])), 22.);
        let alpha = pic.pixels().map(|p| p[3]).collect::<Vec<_>>();
        assert_eq!(
            format!("{:x}", Sha256::digest(alpha)),
            "27be816f5b86c4087e06fd9903b16370508ad200dcec20ab226be53305d21fed"
        );
    }
}
