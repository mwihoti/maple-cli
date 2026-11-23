//! Braille maple leaf shown in place of the Codex logo when Maple is the provider.

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
