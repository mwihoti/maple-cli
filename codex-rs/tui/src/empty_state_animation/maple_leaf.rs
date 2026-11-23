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
