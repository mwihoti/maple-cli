//! Braille maple leaf shown in place of the Codex logo when Maple is the provider.
//!
//! The leaf turns about its stem using the same phase as the logo, so the
//! shared timing, fading and replay logic applies unchanged.

use std::f64::consts::TAU;

use super::renderer::Cell;

/// Right half of the outline, from the top tip down the stem and back to the
/// centre line. The x axis points right and the y axis points up; the left
/// half is the mirror image.
const HALF_OUTLINE: [(f64, f64); 14] = [
    (0.0, 1.0),
    (0.13, 0.70),
    (0.27, 0.78),
    (0.20, 0.24),
    (0.50, 0.50),
    (0.55, 0.36),
    (0.84, 0.42),
    (0.76, 0.14),
    (0.95, 0.02),
    (0.50, -0.34),
    (0.57, -0.52),
    (0.035, -0.44),
    (0.035, -0.88),
    (0.0, -0.88),
];
// Braille dot bits in row-major order for a cell two dots wide and four tall.
const DOT_BITS: [u8; 8] = [1, 8, 2, 16, 4, 32, 64, 128];
const TIP_COLOR: (f64, f64, f64) = (247.0, 166.0, 60.0);
const BASE_COLOR: (f64, f64, f64) = (206.0, 58.0, 36.0);

fn inside(x: f64, y: f64) -> bool {
    let x = x.abs();
    let mut inside = false;
    for (i, &(x1, y1)) in HALF_OUTLINE.iter().enumerate() {
        let (x2, y2) = HALF_OUTLINE[(i + 1) % HALF_OUTLINE.len()];
        if (y1 > y) != (y2 > y) && x < (x2 - x1) * (y - y1) / (y2 - y1) + x1 {
            inside = !inside;
        }
    }
    inside
}

/// Rasterizes the leaf for `phase`, where each half unit is one full turn.
pub(super) fn frame(columns: u16, rows: u16, phase: f64) -> Vec<Cell> {
    let columns = usize::from(columns);
    let rows = usize::from(rows);
    let dot_width = (columns * 2) as f64;
    let dot_height = (rows * 4) as f64;
    let radius = (dot_height / 2.0 - 1.0).max(1.0);
    let turn = (phase * 2.0 * TAU).cos();
    let facing = turn.abs();
    let mut cells = vec![Cell::default(); columns * rows];
    for (index, cell) in cells.iter_mut().enumerate() {
        let column = index % columns;
        let row = index / columns;
        for (dot, bit) in DOT_BITS.iter().enumerate() {
            let dot_x = (column * 2 + dot % 2) as f64 + 0.5;
            let dot_y = (row * 4 + dot / 2) as f64 + 0.5;
            let x = (dot_x - dot_width / 2.0) / radius / facing;
            let y = -(dot_y - dot_height / 2.0) / radius;
            if inside(x, y) {
                cell.dots |= bit;
            }
        }
        if cell.dots != 0 {
            let y = 1.0 - (row as f64 + 0.5) / rows as f64 * 2.0;
            let blend = ((y + 1.0) / 2.0).clamp(0.0, 1.0);
            let channel = |base: f64, tip: f64| (base + (tip - base) * blend) as u32;
            cell.rgb = channel(BASE_COLOR.0, TIP_COLOR.0) << 16
                | channel(BASE_COLOR.1, TIP_COLOR.1) << 8
                | channel(BASE_COLOR.2, TIP_COLOR.2);
        }
    }
    cells
}
