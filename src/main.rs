use image::{codecs::png::PngDecoder, ImageDecoder};
use pixelmatch::{pixelmatch, Options};
use std::{fs, path::PathBuf, process, time};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "example", about = "An example of StructOpt usage.")]
struct Opt {
    /// Image 1
    #[structopt(parse(from_os_str))]
    img1_path: PathBuf,

    /// Image 2
    #[structopt(parse(from_os_str))]
    img2_path: PathBuf,

    /// Diff image
    #[structopt(parse(from_os_str))]
    diff_path: Option<PathBuf>,

    /// Threshold
    #[structopt(short, long)]
    threshold: Option<f64>,

    /// Include antialiasing
    #[structopt(short, long)]
    include_aa: Option<bool>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opt = Opt::from_args();

    let mut options = Options::default();
    if let Some(threshold) = opt.threshold {
        options.threshold = threshold;
    }
    if let Some(include_aa) = opt.include_aa {
        options.include_aa = include_aa;
    }

    let img1 = fs::read(opt.img1_path)?;
    let img2 = fs::read(opt.img2_path)?;

    let (width1, height1) = PngDecoder::new(img1.as_slice())?.dimensions();
    let (width2, height2) = PngDecoder::new(img2.as_slice())?.dimensions();
    if width1 != width2 || height1 != height2 {
        println!(
            "Image dimensions do not match: {}x{} vs {}x{}",
            width1, height1, width2, height2
        );
        process::exit(65);
    }

    let mut img_out = Vec::new();
    let output = match opt.diff_path {
        Some(..) => Some(&mut img_out),
        None => None,
    };

    let now = time::Instant::now();

    let diffs = pixelmatch(
        img1.as_slice(),
        img2.as_slice(),
        output,
        None,
        None,
        Some(options),
    )?;

    println!("matched in {}", now.elapsed().as_micros());

    println!("different pixels: {}", diffs);

    println!(
        "error: {}%",
        ((100.0 * 100.0 * diffs as f64) / (width1 as f64 * height1 as f64)).round() / 100.0
    );

    if let Some(diff_path) = opt.diff_path {
        fs::write(diff_path, img_out)?;
    }

    if diffs > 0 {
        process::exit(66);
    }

    Ok(())
}
