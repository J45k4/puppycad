//! Orbit camera for the native 3D preview, mirroring the web part view
//! (`src/ui/part.ts` and `src/ui/orbit-pivot.ts`): right-drag rotates in screen
//! axes around a pivot picked under the cursor, middle-drag pans so the model
//! follows the pointer, and the wheel zooms about the view center.

/// Radians of rotation per dragged pixel (web: `dx * 0.01`).
pub const ROTATE_PER_PIXEL: f32 = 0.01;
/// Wheel zoom sensitivity per pixel of wheel delta (web: `PREVIEW_ZOOM_SENSITIVITY`).
pub const ZOOM_SENSITIVITY: f32 = 0.0015;
const MIN_ZOOM: f32 = 0.02;
const MAX_ZOOM: f32 = 200.0;

pub type Vec3 = [f32; 3];

pub fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Quat {
    pub const IDENTITY: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };

    pub fn from_axis_angle(axis: Vec3, angle: f32) -> Self {
        let (sin, cos) = (angle * 0.5).sin_cos();
        Self {
            x: axis[0] * sin,
            y: axis[1] * sin,
            z: axis[2] * sin,
            w: cos,
        }
    }

    /// Same as three.js `Quaternion.setFromEuler` with the default `XYZ` order.
    pub fn from_euler_xyz(x: f32, y: f32, z: f32) -> Self {
        let (s1, c1) = (x * 0.5).sin_cos();
        let (s2, c2) = (y * 0.5).sin_cos();
        let (s3, c3) = (z * 0.5).sin_cos();
        Self {
            x: s1 * c2 * c3 + c1 * s2 * s3,
            y: c1 * s2 * c3 - s1 * c2 * s3,
            z: c1 * c2 * s3 + s1 * s2 * c3,
            w: c1 * c2 * c3 - s1 * s2 * s3,
        }
    }

    /// `self * other`: applies `other` first, then `self`.
    pub fn mul(self, other: Self) -> Self {
        Self {
            x: self.w * other.x + self.x * other.w + self.y * other.z - self.z * other.y,
            y: self.w * other.y - self.x * other.z + self.y * other.w + self.z * other.x,
            z: self.w * other.z + self.x * other.y - self.y * other.x + self.z * other.w,
            w: self.w * other.w - self.x * other.x - self.y * other.y - self.z * other.z,
        }
    }

    pub fn conjugate(self) -> Self {
        Self {
            x: -self.x,
            y: -self.y,
            z: -self.z,
            w: self.w,
        }
    }

    pub fn normalize(self) -> Self {
        let length = (self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w).sqrt();
        if length <= f32::EPSILON {
            return Self::IDENTITY;
        }
        Self {
            x: self.x / length,
            y: self.y / length,
            z: self.z / length,
            w: self.w / length,
        }
    }

    pub fn rotate(self, v: Vec3) -> Vec3 {
        let u = [self.x, self.y, self.z];
        let t = cross(u, v).map(|c| c * 2.0);
        add(add(v, t.map(|c| c * self.w)), cross(u, t))
    }
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Incremental rotation in screen axes (web: `screenRotation`); angles in radians.
pub fn screen_rotation(horizontal: f32, vertical: f32) -> Quat {
    let axis = [vertical, horizontal, 0.0];
    let angle = (vertical * vertical + horizontal * horizontal).sqrt();
    if angle == 0.0 {
        return Quat::IDENTITY;
    }
    Quat::from_axis_angle(axis.map(|c| c / angle), angle)
}

/// Maps native CAD coordinates (Z up) to view-model coordinates (Y up, like three.js).
/// A proper rotation that puts the CAD +X, +Y and +Z sides on the model's -X, +Z and +Y
/// sides, which are the ones the web's default orientation shows.
pub fn cad_to_model(p: Vec3) -> Vec3 {
    [-p[0], p[2], p[1]]
}

/// Camera state in view-model units (millimeters). View space has +X right, +Y up and
/// +Z toward the viewer; `to_view(p) = pan + pivot + R (p - pivot)`, as in the web
/// preview's root and content groups.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrbitView {
    pub orientation: Quat,
    pub pan: Vec3,
    pub pivot: Vec3,
    pub zoom: f32,
    pub perspective: bool,
}

impl Default for OrbitView {
    /// Web `PART_PROJECT_DEFAULT_ROTATION`: pitch π/5, yaw π/4.
    fn default() -> Self {
        Self {
            orientation: Quat::from_euler_xyz(
                std::f32::consts::PI / 5.0,
                std::f32::consts::FRAC_PI_4,
                0.0,
            ),
            pan: [0.0; 3],
            pivot: [0.0; 3],
            zoom: 1.0,
            perspective: false,
        }
    }
}

impl OrbitView {
    pub fn model_to_view(&self, p: Vec3) -> Vec3 {
        add(
            add(self.pan, self.pivot),
            self.orientation.rotate(sub(p, self.pivot)),
        )
    }

    pub fn view_to_model(&self, v: Vec3) -> Vec3 {
        add(
            self.pivot,
            self.orientation
                .conjugate()
                .rotate(sub(v, add(self.pan, self.pivot))),
        )
    }

    /// Rotates a model direction into view space (for facing and lighting).
    pub fn rotate(&self, direction: Vec3) -> Vec3 {
        self.orientation.rotate(direction)
    }

    /// Right-drag orbit: the model follows the pointer in screen axes, including roll.
    pub fn rotate_drag(&mut self, dx: f32, dy: f32) {
        self.orientation = screen_rotation(dx * ROTATE_PER_PIXEL, dy * ROTATE_PER_PIXEL)
            .mul(self.orientation)
            .normalize();
    }

    /// Moves the orbit pivot without moving anything on screen (web: `setPreviewOrbitPivot`).
    pub fn set_pivot(&mut self, pivot: Vec3) {
        let before = sub(self.pivot, self.orientation.rotate(self.pivot));
        let after = sub(pivot, self.orientation.rotate(pivot));
        self.pan = add(self.pan, sub(before, after));
        self.pivot = pivot;
    }

    /// Middle-drag pan; `units_per_pixel` keeps the model under the pointer.
    pub fn pan_drag(&mut self, dx: f32, dy: f32, units_per_pixel: f32) {
        self.pan[0] += dx * units_per_pixel;
        self.pan[1] -= dy * units_per_pixel;
    }

    /// Wheel zoom about the view center; positive `delta_y` (scrolling down) zooms out.
    pub fn zoom_wheel(&mut self, delta_y: f32) {
        self.zoom = (self.zoom / (delta_y * ZOOM_SENSITIVITY).exp()).clamp(MIN_ZOOM, MAX_ZOOM);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-4)
    }

    #[test]
    fn euler_matches_three_js_xyz_order() {
        let q = Quat::from_euler_xyz(0.3, 0.0, 0.0);
        assert!(close(
            q.rotate([0.0, 1.0, 0.0]),
            [0.0, 0.3f32.cos(), 0.3f32.sin()]
        ));
        let q = Quat::from_euler_xyz(0.2, 0.5, 0.0);
        let expected = Quat::from_axis_angle([1.0, 0.0, 0.0], 0.2)
            .mul(Quat::from_axis_angle([0.0, 1.0, 0.0], 0.5));
        assert!(close(
            q.rotate([1.0, 2.0, 3.0]),
            expected.rotate([1.0, 2.0, 3.0])
        ));
    }

    #[test]
    fn dragging_moves_the_near_side_with_the_pointer() {
        let mut view = OrbitView {
            orientation: Quat::IDENTITY,
            ..OrbitView::default()
        };
        let front = [0.0, 0.0, 10.0];
        view.rotate_drag(20.0, 0.0);
        assert!(
            view.model_to_view(front)[0] > 0.0,
            "drag right moves the front right"
        );
        let mut view = OrbitView {
            orientation: Quat::IDENTITY,
            ..OrbitView::default()
        };
        view.rotate_drag(0.0, 20.0);
        assert!(
            view.model_to_view(front)[1] < 0.0,
            "drag down moves the front down"
        );
    }

    #[test]
    fn orbit_is_unrestricted_and_keeps_roll() {
        let mut view = OrbitView::default();
        // Half a turn: 100 steps of π px at 0.01 rad/px, with no pitch clamp at the pole.
        for _ in 0..100 {
            view.rotate_drag(0.0, std::f32::consts::PI);
        }
        let top = view.model_to_view([0.0, 10.0, 0.0]);
        assert!(top[1] < 0.0, "orbiting past the pole turns the model over");
        let before = view;
        view.rotate_drag(15.0, 0.0);
        view.rotate_drag(0.0, 15.0);
        view.rotate_drag(-15.0, 0.0);
        view.rotate_drag(0.0, -15.0);
        assert_ne!(
            view.orientation, before.orientation,
            "screen-axis loops accumulate roll"
        );
    }

    #[test]
    fn moving_the_pivot_keeps_the_picture_still() {
        let mut view = OrbitView {
            pan: [3.0, -2.0, 1.0],
            ..OrbitView::default()
        };
        let point = [4.0, 5.0, -6.0];
        let before = view.model_to_view(point);
        view.set_pivot([10.0, 0.0, 2.0]);
        assert!(close(view.model_to_view(point), before));
        assert!(close(view.view_to_model(view.model_to_view(point)), point));
    }

    #[test]
    fn rotation_happens_around_the_pivot() {
        let mut view = OrbitView::default();
        let pivot = [5.0, 1.0, 2.0];
        view.set_pivot(pivot);
        let at = view.model_to_view(pivot);
        view.rotate_drag(40.0, -25.0);
        assert!(close(view.model_to_view(pivot), at));
    }

    #[test]
    fn pan_follows_pointer_and_wheel_zooms_like_the_web() {
        let mut view = OrbitView::default();
        let point = [1.0, 2.0, 3.0];
        let before = view.model_to_view(point);
        view.pan_drag(10.0, 4.0, 0.5);
        let after = view.model_to_view(point);
        assert!(close(sub(after, before), [5.0, -2.0, 0.0]));
        view.zoom_wheel(100.0);
        assert!((view.zoom - (-0.15f32).exp()).abs() < 1e-5);
        view.zoom_wheel(-100.0);
        assert!((view.zoom - 1.0).abs() < 1e-5);
    }

    #[test]
    fn cad_to_model_is_a_rotation_with_cad_up_as_view_up() {
        assert_eq!(cad_to_model([0.0, 0.0, 1.0]), [0.0, 1.0, 0.0]);
        let [x, y, z] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]].map(cad_to_model);
        assert_eq!(cross(x, y), z, "right-handed, so nothing is mirrored");
    }
}
