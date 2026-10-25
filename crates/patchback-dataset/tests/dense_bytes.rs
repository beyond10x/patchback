//! The bytes of the dense files of a real 48-sample generation (16 / 16 / 16, seed 42) through
//! [`dense::write_split`] (DESIGN.md § 7.2, Da7-8, Da7-9, Da7-11): each split's `x.f32` is
//! 16 × 32 × 4 = 2,048 bytes, `y.f32` 16 × 8 × 4 = 512 and `order.u32` 16 × 4 = 64; row `i`
//! of `x.f32` and `y.f32` is the split's `i`th sample in draw order, as `f32`, row-major,
//! little-endian, `x` z-scored under the training split's statistics for every split (Da8-30:
//! `((x − mean) / std) as f32`, divisor 1 where the standard deviation is 0, the oracle
//! recomputed here from the statistics' means and standard deviations); `order.u32` is 0 to 15.

use std::fs;
use std::path::{Path, PathBuf};

use patchback_analysis::NormalizationStats;
use patchback_dataset::dense;
use patchback_dataset::{Generator, SplitCounts, SplitRole, SplitState, training_stats};

/// Value `j` of a row, z-scored under `stats` in `f64`.
fn z(stats: &NormalizationStats, j: usize, value: f64) -> f64 {
    let std = stats.stds()[j];
    (value - stats.means()[j]) / if std == 0.0 { 1.0 } else { std }
}

fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-dataset-dense-bytes")
        .join(test);
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn f32s(bytes: &[u8]) -> Vec<f32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|bytes| f32::from_le_bytes(*bytes))
        .collect()
}

#[test]
fn a_48_sample_generation_writes_the_dense_files_of_each_split() {
    let out = scratch("forty-eight");
    let counts = SplitCounts::from_samples(48);
    let mut generated = Generator::new(42, counts, out.clone()).run().unwrap();
    let stats = training_stats(&generated).unwrap();

    for role in SplitRole::ALL {
        let split = generated.split_id(role);
        let dir = out.join(role.dir());
        fs::create_dir_all(&dir).unwrap();
        dense::write_split(&mut generated.store, split, &dir, &stats, dense::create_new).unwrap();
        assert_eq!(
            generated.store.split(split).unwrap().state,
            SplitState::Written
        );

        let x = fs::read(dir.join("x.f32")).unwrap();
        let y = fs::read(dir.join("y.f32")).unwrap();
        let order = fs::read(dir.join("order.u32")).unwrap();
        assert_eq!((x.len(), y.len(), order.len()), (2_048, 512, 64), "{role}");

        let samples = generated.store.samples_in(split);
        assert_eq!(samples.len(), 16);
        let x = f32s(&x);
        let y = f32s(&y);
        for (i, sample) in samples.iter().enumerate() {
            let expected_x: Vec<f32> = sample
                .x
                .iter()
                .enumerate()
                .map(|(j, &v)| z(&stats, j, v) as f32)
                .collect();
            let expected_y: Vec<f32> = sample.y.iter().map(|&v| v as f32).collect();
            assert_eq!(x[i * 32..(i + 1) * 32], expected_x[..], "{role} x row {i}");
            assert_eq!(y[i * 8..(i + 1) * 8], expected_y[..], "{role} y row {i}");
        }
        let entries: Vec<u32> = order
            .as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| u32::from_le_bytes(*bytes))
            .collect();
        assert_eq!(entries, (0..16).collect::<Vec<u32>>(), "{role}");

        let mut files: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        files.sort();
        assert_eq!(files, ["order.u32", "x.f32", "y.f32"], "{role}");
    }
}

/// One value by hand: the third sample of the validation split, its `x[5]` z-scored under the
/// training statistics as `f32`, laid out least significant byte first at byte `(2 · 32 + 5) · 4`
/// of `val/x.f32`; its `y[7]` at byte `(2 · 8 + 7) · 4` of `val/y.f32`.
#[test]
fn one_value_by_hand_is_little_endian_row_major_f32() {
    let out = scratch("by-hand");
    let mut generated = Generator::new(42, SplitCounts::from_samples(48), out.clone())
        .run()
        .unwrap();
    let stats = training_stats(&generated).unwrap();
    let split = generated.split_id(SplitRole::Validation);
    let dir = out.join("val");
    fs::create_dir_all(&dir).unwrap();
    dense::write_split(&mut generated.store, split, &dir, &stats, dense::create_new).unwrap();

    let sample = generated.store.samples_in(split)[2].clone();
    for (file, offset, value) in [
        ("x.f32", (2 * 32 + 5) * 4, z(&stats, 5, sample.x[5])),
        ("y.f32", (2 * 8 + 7) * 4, sample.y[7]),
    ] {
        let bits = (value as f32).to_bits();
        let by_hand = [
            (bits & 0xff) as u8,
            ((bits >> 8) & 0xff) as u8,
            ((bits >> 16) & 0xff) as u8,
            (bits >> 24) as u8,
        ];
        let bytes = fs::read(dir.join(file)).unwrap();
        assert_eq!(bytes[offset..offset + 4], by_hand, "{file} at {offset}");
        assert_ne!(
            bits, 0,
            "{file}: a value that is not zero, so the order of bytes shows"
        );
    }
}
