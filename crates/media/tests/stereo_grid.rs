//! Native sample-grid contract shared by picture and video frontends.
use craft_media::stereo::{Eye, PackedGrid, Packing, SampleAspect};

fn grid(width: usize, height: usize, packing: Packing, first: Eye, aspect: (u32, u32)) -> PackedGrid {
    PackedGrid::new(width, height, packing, first, SampleAspect::new(aspect.0, aspect.1).unwrap()).unwrap()
}

#[test]
fn half_width_pair_copies_ids_without_expanding_samples() {
    let source: Vec<u32> = (0..48).collect();
    let g = grid(12, 4, Packing::SideBySide { gap: 0 }, Eye::Left, (2, 1));
    assert_eq!(g.eye_size(), (6, 4));
    assert_eq!(g.pixel_aspect_ratio(), (2, 1));
    let pair = g.copy_eyes(&source, 48 * 4).unwrap();
    assert_eq!(pair.eye_size(), (6, 4));
    assert_eq!(pair.pixel_aspect_ratio(), (2, 1));
    let (left, right) = (pair.left, pair.right);
    assert_eq!(left, [0, 1, 2, 3, 4, 5, 12, 13, 14, 15, 16, 17, 24, 25, 26, 27, 28, 29, 36, 37, 38, 39, 40, 41]);
    assert_eq!(right, [6, 7, 8, 9, 10, 11, 18, 19, 20, 21, 22, 23, 30, 31, 32, 33, 34, 35, 42, 43, 44, 45, 46, 47]);
}

#[test]
fn half_height_pair_keeps_native_rows_and_right_first_order() {
    let source: Vec<u16> = (0..32).collect();
    let g = grid(4, 8, Packing::TopBottom { gap: 0 }, Eye::Right, (1, 2));
    assert_eq!(g.eye_size(), (4, 4));
    assert_eq!(g.pixel_aspect_ratio(), (1, 2));
    let pair = g.copy_eyes(&source, 64).unwrap();
    assert_eq!(pair.eye_size(), (4, 4));
    assert_eq!(pair.pixel_aspect_ratio(), (1, 2));
    let (left, right) = (pair.left, pair.right);
    assert_eq!(left, (16..32).collect::<Vec<_>>());
    assert_eq!(right, (0..16).collect::<Vec<_>>());
}

#[test]
fn interleaved_rows_copy_each_id_once_without_repeated_lines() {
    let source: Vec<u8> = (0..24).collect();
    let g = grid(4, 6, Packing::Rows, Eye::Left, (1, 2));
    assert_eq!(g.eye_size(), (4, 3));
    let pair = g.copy_eyes(&source, 24).unwrap();
    let (left, right) = (pair.left, pair.right);
    assert_eq!(left, [0, 1, 2, 3, 8, 9, 10, 11, 16, 17, 18, 19]);
    assert_eq!(right, [4, 5, 6, 7, 12, 13, 14, 15, 20, 21, 22, 23]);
}

#[test]
fn interleaved_columns_keep_native_width_and_eye_order() {
    let source: Vec<[u8; 3]> = (0..12).map(|id| [id, 255 - id, 17]).collect();
    let g = grid(6, 2, Packing::Columns, Eye::Right, (2, 1));
    assert_eq!(g.eye_size(), (3, 2));
    let pair = g.copy_eyes(&source, 36).unwrap();
    let (left, right) = (pair.left, pair.right);
    assert_eq!(left, [1, 3, 5, 7, 9, 11].map(|id| [id, 255 - id, 17]));
    assert_eq!(right, [0, 2, 4, 6, 8, 10].map(|id| [id, 255 - id, 17]));
}

#[test]
fn padding_is_excluded_without_moving_or_mixing_eye_samples() {
    let source = [1, 2, 3, 99, 99, 7, 8, 9, 4, 5, 6, 99, 99, 10, 11, 12];
    let g = grid(8, 2, Packing::SideBySide { gap: 2 }, Eye::Left, (1, 1));
    let pair = g.copy_eyes(&source, 48).unwrap();
    assert_eq!((pair.left, pair.right), (vec![1, 2, 3, 4, 5, 6], vec![7, 8, 9, 10, 11, 12]));
    let vertical = [1, 2, 3, 4, 99, 99, 7, 8, 9, 10];
    let g = grid(2, 5, Packing::TopBottom { gap: 1 }, Eye::Left, (1, 1));
    let pair = g.copy_eyes(&vertical, 32).unwrap();
    assert_eq!((pair.left, pair.right), (vec![1, 2, 3, 4], vec![7, 8, 9, 10]));
}

#[test]
fn invalid_geometry_and_sample_shapes_are_rejected_before_allocation() {
    assert!(SampleAspect::new(0, 1).is_none());
    assert!(SampleAspect::new(1, 0).is_none());
    let aspect = SampleAspect::new(2, 2).unwrap();
    for (w, h, packing) in [
        (0, 4, Packing::Rows),
        (4, 0, Packing::Columns),
        (5, 4, Packing::SideBySide { gap: 0 }),
        (4, 5, Packing::TopBottom { gap: 0 }),
        (4, 3, Packing::Rows),
        (3, 4, Packing::Columns),
        (4, 4, Packing::SideBySide { gap: 4 }),
        (4, 4, Packing::TopBottom { gap: usize::MAX }),
        (usize::MAX - 1, 4, Packing::Columns),
    ] {
        assert!(PackedGrid::new(w, h, packing, Eye::Left, aspect).is_err(), "{w}×{h}: {packing:?}");
    }
}

#[test]
fn source_length_budget_and_coordinates_are_checked() {
    let g = grid(4, 2, Packing::SideBySide { gap: 0 }, Eye::Left, (1, 1));
    assert!(g.copy_eyes(&[0u32; 7], 32).is_err());
    assert!(g.copy_eyes(&[0u32; 9], 36).is_err());
    assert!(g.copy_eyes(&[0u32; 8], 31).is_err());
    assert!(g.copy_eyes(&[0u32; 8], 32).is_ok());
    assert_eq!(g.source_index(Eye::Left, 2, 0), None);
    assert_eq!(g.source_index(Eye::Right, 0, 2), None);
    assert_eq!(g.source_index(Eye::Right, usize::MAX, 0), None);
    assert_eq!(g.source_index(Eye::Right, 1, 1), Some(7));
}

#[test]
fn checkerboard_compacts_native_samples_without_filling_holes() {
    let g = grid(6, 3, Packing::Checkerboard, Eye::Right, (2, 1));
    let pair = g.copy_eyes(&(0u8..18).collect::<Vec<_>>(), 18).unwrap();
    assert_eq!(pair.eye_size(), (3, 3));
    assert_eq!(pair.left, [1, 3, 5, 6, 8, 10, 13, 15, 17]);
    assert_eq!(pair.right, [0, 2, 4, 7, 9, 11, 12, 14, 16]);
}

#[test]
fn sample_shape_is_reduced_and_quarter_turns_exchange_its_axes() {
    let aspect = SampleAspect::new(24, 18).unwrap();
    assert_eq!(aspect.ratio(), (4, 3));
    assert_eq!(aspect.rotated().ratio(), (3, 4));
    assert_eq!(aspect.rotated().rotated(), aspect);
}
