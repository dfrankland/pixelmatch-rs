use image::{
    codecs::png::PngDecoder, DynamicImage, GenericImage, GenericImageView, ImageOutputFormat, Rgba,
};
use std::io::{Read, Write};

pub struct Options {
    /// matching threshold (0 to 1); smaller is more sensitive
    pub threshold: f32,
    /// whether to skip anti-aliasing detection
    pub include_aa: bool,
    /// opacity of original image in diff output
    pub alpha: f32,
    /// color of anti-aliased pixels in diff output
    pub aa_color: [u8; 4],
    /// color of different pixels in diff output
    pub diff_color: [u8; 4],
    /// whether to detect dark on light differences between img1 and img2 and set an alternative color to differentiate between the two
    pub diff_color_alt: Option<[u8; 4]>,
    /// draw the diff over a transparent background (a mask)
    pub diff_mask: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            threshold: 0.1,
            include_aa: false,
            alpha: 0.1,
            aa_color: [255, 255, 0, 255],
            diff_color: [255, 0, 0, 255],
            diff_color_alt: None,
            diff_mask: false,
        }
    }
}

pub fn pixelmatch<IMG1: Read, IMG2: Read, OUT: Write>(
    img1: IMG1,
    img2: IMG2,
    mut output: Option<&mut OUT>,
    width: Option<u32>,
    height: Option<u32>,
    options: Option<Options>,
) -> Result<usize, Box<dyn std::error::Error>> {
    let img1 = DynamicImage::from_decoder(PngDecoder::new(img1)?)?;
    let img2 = DynamicImage::from_decoder(PngDecoder::new(img2)?)?;

    let img1_dimensions = img1.dimensions();
    if img1.dimensions() != img2.dimensions() {
        return Err(<Box<dyn std::error::Error>>::from(
            "Image sizes do not match.",
        ));
    }

    if let (Some(width), Some(height)) = (width, height) {
        if (width, height) != img1_dimensions {
            return Err(<Box<dyn std::error::Error>>::from(
                "Image data size does not match width/height.",
            ));
        }
    }

    let options = options.unwrap_or_default();
    let mut img_out = match output {
        Some(..) => Some(DynamicImage::new_rgba8(
            img1_dimensions.0,
            img1_dimensions.1,
        )),
        None => None,
    };

    // check if images are identical
    let mut identical = true;
    for (pixel1, pixel2) in img1.pixels().zip(img2.pixels()) {
        if pixel1 != pixel2 {
            identical = false;
            break;
        }
    }

    // fast path if identical
    if identical {
        if let (Some(output), Some(img_out)) = (&mut output, &mut img_out) {
            if !options.diff_mask {
                for pixel in img1.pixels() {
                    draw_gray_pixel(&pixel, options.alpha, img_out)?;
                }
            }

            img_out.write_to(*output, ImageOutputFormat::Png)?;
        }

        return Ok(0);
    }

    // maximum acceptable square distance between two colors;
    // 35215 is the maximum possible value for the YIQ difference metric
    let max_delta = 35215_f32 * options.threshold * options.threshold;
    let mut diff: usize = 0;

    for (pixel1, pixel2) in img1.pixels().zip(img2.pixels()) {
        let delta = color_delta(&pixel1.2, &pixel2.2, false);
        if delta.abs() > max_delta {
            // check it's a real rendering difference or just anti-aliasing
            if !options.include_aa
                && (antialiased(
                    &img1,
                    pixel1.0,
                    pixel1.1,
                    img1_dimensions.0,
                    img1_dimensions.1,
                    &img2,
                ) || antialiased(
                    &img2,
                    pixel1.0,
                    pixel1.1,
                    img1_dimensions.0,
                    img1_dimensions.1,
                    &img1,
                ))
            {
                // one of the pixels is anti-aliasing; draw as yellow and do not count as difference
                // note that we do not include such pixels in a mask
                if let (Some(img_out), false) = (&mut img_out, options.diff_mask) {
                    img_out.put_pixel(pixel1.0, pixel1.1, Rgba(options.aa_color));
                }
            } else {
                // found substantial difference not caused by anti-aliasing; draw it as such
                if let Some(img_out) = &mut img_out {
                    let color = if delta < 0.0 {
                        options.diff_color_alt.unwrap_or(options.diff_color)
                    } else {
                        options.diff_color
                    };
                    img_out.put_pixel(pixel1.0, pixel1.1, Rgba(color));
                }
                diff += 1;
            }
        } else if let (Some(img_out), false) = (&mut img_out, options.diff_mask) {
            // pixels are similar; draw background as grayscale image blended with white
            draw_gray_pixel(&pixel1, options.alpha, img_out)?;
        }
    }

    if let (Some(output), Some(img_out)) = (&mut output, &mut img_out) {
        img_out.write_to(*output, ImageOutputFormat::Png)?;
    }

    Ok(diff)
}

// check if a pixel is likely a part of anti-aliasing;
// based on "Anti-aliased Pixel and Intensity Slope Detector" paper by V. Vysniauskas, 2009
fn antialiased(
    img1: &DynamicImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    img2: &DynamicImage,
) -> bool {
    let mut zeroes: u8 = 0;
    let mut min = 0.0;
    let mut max = 0.0;
    let mut min_x = 0;
    let mut min_y = 0;
    let mut max_x = 0;
    let mut max_y = 0;

    let center_rgba = img1.get_pixel(x, y);

    for relative_x in -1_i32..=1 {
        for relative_y in -1_i32..=1 {
            if relative_x == 0 && relative_y == 0 {
                continue;
            }

            // brightness delta between the center pixel and adjacent one
            let adjacent_x = (x as i32)
                .saturating_add(relative_x)
                .max(0)
                .min(width as i32 - 1) as u32;
            let adjacent_y = (y as i32)
                .saturating_add(relative_y)
                .max(0)
                .min(height as i32 - 1) as u32;
            let rgba = img1.get_pixel(adjacent_x, adjacent_y);
            let delta = color_delta(&center_rgba, &rgba, true);

            // count the number of equal, darker and brighter adjacent pixels
            if delta == 0.0 {
                zeroes += 1;
                // if found more than 2 equal siblings, it's definitely not anti-aliasing
                if zeroes > 2 {
                    return false;
                }

                continue;
            }

            // remember the darkest pixel
            if delta < min {
                min = delta;
                min_x = adjacent_x;
                min_y = adjacent_y;

                continue;
            }

            // remember the brightest pixel
            if delta > max {
                max = delta;
                max_x = adjacent_x;
                max_y = adjacent_y;
            }
        }
    }

    // if there are no both darker and brighter pixels among siblings, it's not anti-aliasing
    if min == 0.0 || max == 0.0 {
        return false;
    }

    // if either the darkest or the brightest pixel has 3+ equal siblings in both images
    // (definitely not anti-aliased), this pixel is anti-aliased
    (has_many_siblings(img1, min_x, min_y, width, height)
        && has_many_siblings(img2, min_x, min_y, width, height))
        || (has_many_siblings(img1, max_x, max_y, width, height)
            && has_many_siblings(img2, max_x, max_y, width, height))
}

// check if a pixel has 3+ adjacent pixels of the same color.
fn has_many_siblings(img: &DynamicImage, x: u32, y: u32, width: u32, height: u32) -> bool {
    let mut zeroes: u8 = 0;

    let center_rgba = img.get_pixel(x, y);

    for relative_x in -1_i32..=1 {
        for relative_y in -1_i32..=1 {
            if relative_x == 0 && relative_y == 0 {
                continue;
            }

            let adjacent_x = (x as i32)
                .saturating_add(relative_x)
                .max(0)
                .min(width as i32 - 1) as u32;
            let adjacent_y = (y as i32)
                .saturating_add(relative_y)
                .max(0)
                .min(height as i32 - 1) as u32;
            let rgba = img.get_pixel(adjacent_x, adjacent_y);

            if center_rgba == rgba {
                zeroes += 1;
            }

            if zeroes > 2 {
                return true;
            }
        }
    }

    return false;
}

// calculate color difference according to the paper "Measuring perceived color difference
// using YIQ NTSC transmission color space in mobile applications" by Y. Kotsarenko and F. Ramos
fn color_delta(rgba1: &Rgba<u8>, rgba2: &Rgba<u8>, y_only: bool) -> f32 {
    let mut r1 = rgba1[0] as f32;
    let mut g1 = rgba1[1] as f32;
    let mut b1 = rgba1[2] as f32;
    let mut a1 = rgba1[3] as f32;

    let mut r2 = rgba2[0] as f32;
    let mut g2 = rgba2[1] as f32;
    let mut b2 = rgba2[2] as f32;
    let mut a2 = rgba2[3] as f32;

    if a1 == a2 && r1 == r2 && g1 == g2 && b1 == b2 {
        return 0.0;
    }

    if a1 < 255.0 {
        a1 /= 255.0;
        r1 = blend(r1, a1);
        g1 = blend(g1, a1);
        b1 = blend(b1, a1);
    }

    if a2 < 255.0 {
        a2 /= 255.0;
        r2 = blend(r2, a2);
        g2 = blend(g2, a2);
        b2 = blend(b2, a2);
    }

    let y1 = rgb2y(r1, g1, b1);
    let y2 = rgb2y(r2, g2, b2);
    let y = y1 - y2;

    // brightness difference only
    if y_only {
        return y;
    }

    let i = rgb2i(r1, g1, b1) - rgb2i(r2, g2, b2);
    let q = rgb2q(r1, g1, b1) - rgb2q(r2, g2, b2);

    let delta = 0.5053 * y * y + 0.299 * i * i + 0.1957 * q * q;

    // encode whether the pixel lightens or darkens in the sign
    if y1 > y2 {
        -delta
    } else {
        delta
    }
}

fn draw_gray_pixel(
    (x, y, rgba): &(u32, u32, Rgba<u8>),
    alpha: f32,
    output: &mut DynamicImage,
) -> Result<(), Box<dyn std::error::Error>> {
    if !output.in_bounds(*x, *y) {
        return Err(<Box<dyn std::error::Error>>::from(
            "Pixel is not in bounds of output.",
        ));
    }

    let val = blend(
        rgb2y(rgba[0], rgba[1], rgba[2]),
        (alpha * rgba[3] as f32) / 255.0,
    ) as u8;
    let gray_rgba = Rgba([val, val, val, val]);
    output.put_pixel(*x, *y, gray_rgba);

    Ok(())
}

// blend semi-transparent color with white
fn blend<T: Into<f32>>(c: T, a: T) -> f32 {
    255.0 + (c.into() - 255.0) * a.into()
}

fn rgb2y<T: Into<f32>>(r: T, g: T, b: T) -> f32 {
    r.into() * 0.29889531 + g.into() * 0.58662247 + b.into() * 0.11448223
}
fn rgb2i<T: Into<f32>>(r: T, g: T, b: T) -> f32 {
    r.into() * 0.59597799 - g.into() * 0.27417610 - b.into() * 0.32180189
}
fn rgb2q<T: Into<f32>>(r: T, g: T, b: T) -> f32 {
    r.into() * 0.21147017 - g.into() * 0.52261711 + b.into() * 0.31114694
}
