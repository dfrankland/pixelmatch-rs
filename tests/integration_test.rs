use paste::paste;
use pixelmatch::{pixelmatch, Options};
use std::{env, fs, path::PathBuf};

macro_rules! diff_test {
  ($img1_path: ident, $img2_path: ident, $diff_path: ident, $options: expr, $options_name: ident, $expected_mismatch: literal) => {
    paste! {
      #[test]
      fn [<comparing_ $img1_path _to_ $img2_path _ $options_name>]() -> Result<(), Box<dyn std::error::Error>>{
        let img1: &[u8] = include_bytes!(concat!("fixtures/", stringify!([< $img1_path >]), ".png"));
        let img2: &[u8] = include_bytes!(concat!("fixtures/", stringify!([< $img2_path >]), ".png"));

        let mut img_out = Vec::new();
        let output = Some(&mut img_out);

        let mismatch1 = pixelmatch(img1, img2, output, None, None, $options)?;
        let mismatch2 = pixelmatch(img1, img2, Option::<&mut Vec<u8>>::None, None, None, $options)?;

        if env::var("UPDATE").is_ok() {
          let mut diff_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
          diff_path.push(concat!("tests/fixtures/", stringify!([< $diff_path >]), ".png"));
          fs::write(diff_path, img_out)?;
        } else {
          let expected_diff: &[u8] = include_bytes!(concat!("fixtures/", stringify!([< $diff_path >]), ".png"));
          assert_eq!(img_out, expected_diff);
        }

        assert_eq!(mismatch1, $expected_mismatch);
        assert_eq!(mismatch1, mismatch2);

        Ok(())
      }
    }
  };
}

diff_test!(
    img_1a,
    img_1b,
    img_1diff,
    Some(Options {
        threshold: 0.05,
        ..Default::default()
    }),
    normal,
    143
);
diff_test!(
    img_1a,
    img_1b,
    img_1diffmask,
    Some(Options {
        threshold: 0.05,
        include_aa: false,
        diff_mask: true,
        ..Default::default()
    }),
    diffmask,
    143
);
diff_test!(
    img_1a,
    img_1b,
    img_1emptydiffmask,
    Some(Options {
        threshold: 0.0,
        diff_mask: true,
        ..Default::default()
    }),
    emptydiffmask,
    0
);
diff_test!(
    img_2a,
    img_2b,
    img_2diff,
    Some(Options {
        threshold: 0.05,
        alpha: 0.5,
        aa_color: [0, 192, 0, 255],
        diff_color: [255, 0, 255, 255],
        ..Default::default()
    }),
    alpha_antialias_color_diff_color,
    12437
);
diff_test!(
    img_3a,
    img_3b,
    img_3diff,
    Some(Options {
        threshold: 0.05,
        ..Default::default()
    }),
    normal,
    212
);
diff_test!(
    img_4a,
    img_4b,
    img_4diff,
    Some(Options {
        threshold: 0.05,
        ..Default::default()
    }),
    normal,
    36049
);
diff_test!(
    img_5a,
    img_5b,
    img_5diff,
    Some(Options {
        threshold: 0.05,
        ..Default::default()
    }),
    normal,
    0
);
diff_test!(
    img_6a,
    img_6b,
    img_6diff,
    Some(Options {
        threshold: 0.05,
        ..Default::default()
    }),
    normal,
    51
);
diff_test!(
    img_6a,
    img_6b,
    img_6empty,
    Some(Options {
        threshold: 0.0,
        ..Default::default()
    }),
    empty,
    0
);
diff_test!(
    img_7a,
    img_7b,
    img_7diff,
    Some(Options {
        diff_color_alt: Some([0, 255, 0, 255]),
        ..Default::default()
    }),
    diff_color_alt,
    2448
);

#[test]
fn throws_error_if_image_sizes_do_not_match() {
    let img1: &[u8] = &[0; 8];
    let img2: &[u8] = &[0; 9];
    let output: Option<&mut Vec<u8>> = None;
    assert_eq!(
        pixelmatch(img1, img2, output, Some(2), Some(1), None).map_err(|err| err.to_string()),
        Err(String::from("Image sizes do not match"))
    );
}
