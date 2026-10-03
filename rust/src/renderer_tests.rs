//! CPU-side tests for `Painter`. `Renderer` needs a GPU surface and stays
//! untested here; it only uploads `Painter::vertices` as one triangle list.

use super::*;

const RED: Color = [1.0, 0.0, 0.0, 1.0];
const BLUE: Color = [0.0, 0.0, 1.0, 0.5];

fn painter() -> Painter {
    Painter::new(200.0, 100.0)
}

fn assert_close(a: [f32; 2], b: [f32; 2]) {
    assert!(
        (a[0] - b[0]).abs() < 1e-5 && (a[1] - b[1]).abs() < 1e-5,
        "{a:?} != {b:?}"
    );
}

fn glyph_pixels(ch: char) -> usize {
    BASIC_FONTS
        .get(ch)
        .unwrap()
        .iter()
        .map(|row| row.count_ones() as usize)
        .sum()
}

#[test]
fn rect_emits_two_triangles_in_clip_space() {
    let mut p = painter();
    p.rect(
        Rect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 50.0,
        },
        RED,
    );
    assert_eq!(p.vertices.len(), 6);
    assert!(p.vertices.iter().all(|v| v.color == RED));
    // Top-left of the window maps to (-1, 1); the rect covers its top-left quarter.
    assert_close(p.vertices[0].position, [-1.0, 1.0]);
    assert_close(p.vertices[1].position, [0.0, 1.0]);
    assert_close(p.vertices[2].position, [0.0, 0.0]);
    assert_close(p.vertices[5].position, [-1.0, 0.0]);
}

#[test]
fn empty_or_negative_rects_emit_nothing() {
    let mut p = painter();
    for (w, h) in [(0.0, 10.0), (10.0, 0.0), (-5.0, 10.0)] {
        p.rect(
            Rect {
                x: 1.0,
                y: 1.0,
                w,
                h,
            },
            RED,
        );
    }
    assert!(p.vertices.is_empty());
}

#[test]
fn triangle_emits_three_vertices() {
    let mut p = painter();
    p.triangle([0.0, 0.0], [200.0, 0.0], [100.0, 100.0], BLUE);
    assert_eq!(p.vertices.len(), 3);
    assert_close(p.vertices[2].position, [0.0, -1.0]);
    assert!(p.vertices.iter().all(|v| v.color == BLUE));
}

#[test]
fn line_is_a_quad_of_the_given_thickness() {
    // A 2x2 window makes one pixel one clip-space unit.
    let mut p = Painter::new(2.0, 2.0);
    p.line(0.0, 1.0, 2.0, 1.0, 0.5, RED);
    assert_eq!(p.vertices.len(), 6);
    let ys = p.vertices.iter().map(|v| v.position[1]);
    let span = ys.clone().fold(f32::MIN, f32::max) - ys.fold(f32::MAX, f32::min);
    assert!((span - 0.5).abs() < 1e-5, "{span}");
}

#[test]
fn degenerate_line_emits_nothing() {
    let mut p = painter();
    p.line(5.0, 5.0, 5.0, 5.0, 2.0, RED);
    assert!(p.vertices.is_empty());
}

#[test]
fn outline_is_four_lines_and_circle_is_96_segments() {
    let mut p = painter();
    let rect = Rect {
        x: 10.0,
        y: 10.0,
        w: 20.0,
        h: 30.0,
    };
    p.outline(rect, 1.0, RED);
    assert_eq!(p.vertices.len(), 4 * 6);
    let mut c = painter();
    c.circle(50.0, 50.0, 20.0, 1.0, BLUE);
    assert_eq!(c.vertices.len(), 96 * 6);
    assert!(c.vertices.iter().all(|v| v.color == BLUE));
}

#[test]
fn text_draws_one_rect_per_font8x8_pixel() {
    let pixels = glyph_pixels('A');
    assert!(pixels > 10);
    let mut p = painter();
    p.text(0.0, 0.0, "A", 1.0, RED);
    assert_eq!(p.vertices.len(), pixels * 6);
    assert!(p.vertices.iter().all(|v| v.color == RED));

    let mut two = painter();
    two.text(0.0, 0.0, "AH", 2.0, RED);
    assert_eq!(two.vertices.len(), (pixels + glyph_pixels('H')) * 6);
}

#[test]
fn text_advances_eight_cells_per_char_and_skips_blanks() {
    let mut p = painter();
    p.text(0.0, 0.0, " ", 3.0, RED);
    assert!(p.vertices.is_empty());
    // A glyph after a space starts 8 * scale pixels to the right.
    let mut a = Painter::new(2.0, 2.0);
    a.text(0.0, 0.0, "A", 0.01, RED);
    let mut sa = Painter::new(2.0, 2.0);
    sa.text(0.0, 0.0, " A", 0.01, RED);
    assert_eq!(a.vertices.len(), sa.vertices.len());
    for (x, y) in a.vertices.iter().zip(&sa.vertices) {
        assert!((y.position[0] - x.position[0] - 0.08).abs() < 1e-5);
    }
    // Characters outside BASIC_FONTS draw nothing.
    let mut u = painter();
    u.text(0.0, 0.0, "\u{2603}", 1.0, RED);
    assert!(u.vertices.is_empty());
}

#[test]
fn draws_append_in_order_without_merging() {
    let mut p = painter();
    let r = Rect {
        x: 0.0,
        y: 0.0,
        w: 10.0,
        h: 10.0,
    };
    p.rect(r, RED);
    p.rect(r, RED);
    p.rect(r, BLUE);
    assert_eq!(p.vertices.len(), 18);
    assert_eq!(p.vertices[11].color, RED);
    assert_eq!(p.vertices[12].color, BLUE);
}

#[test]
fn rect_contains_is_half_open() {
    let r = Rect {
        x: 10.0,
        y: 20.0,
        w: 5.0,
        h: 5.0,
    };
    assert!(r.contains(10.0, 20.0));
    assert!(r.contains(14.9, 24.9));
    assert!(!r.contains(15.0, 22.0));
    assert!(!r.contains(12.0, 25.0));
    assert!(!r.contains(9.9, 22.0));
}
