//! Braille maple leaf shown in place of the Codex logo when Maple is the provider.

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

/// Rasterizes the leaf facing the viewer.
pub(super) fn frame(columns: u16, rows: u16) -> Vec<Cell> {
    let columns = usize::from(columns);
    let rows = usize::from(rows);
    let dot_width = (columns * 2) as f64;
    let dot_height = (rows * 4) as f64;
    let radius = (dot_height / 2.0 - 1.0).max(1.0);
    let mut cells = vec![Cell::default(); columns * rows];
    for (index, cell) in cells.iter_mut().enumerate() {
        let column = index % columns;
        let row = index / columns;
        for (dot, bit) in DOT_BITS.iter().enumerate() {
            let dot_x = (column * 2 + dot % 2) as f64 + 0.5;
            let dot_y = (row * 4 + dot / 2) as f64 + 0.5;
            let x = (dot_x - dot_width / 2.0) / radius;
            let y = -(dot_y - dot_height / 2.0) / radius;
            if inside(x, y) {
                cell.dots |= bit;
            }
        }
    }
    cells
}
