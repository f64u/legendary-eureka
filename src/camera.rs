use bytemuck::{Pod, Zeroable};
use nalgebra::{Matrix4, OPoint, Perspective3, Point3, Vector3};

use crate::app::vs;

pub struct Plane {
    normal: Vector3<f64>,
    distance: f64,
}

pub struct Frustum {
    pub top_face: Plane,
    pub bottom_face: Plane,

    pub left_face: Plane,
    pub right_face: Plane,

    pub near_face: Plane,
    pub far_face: Plane,
}

impl Frustum {
    fn new(camera: &Camera) -> Self {
        let half_v_side = camera.far_z * (camera.fov.to_radians() * 0.5).tan();
        let half_h_side = half_v_side * camera.asepect_ratio;
        let front_mult_far = camera.far_z * camera.front();

        let origin = Point3::new(0.0, 0.0, 0.0);
        let distance = nalgebra::distance(&camera.pos, &origin);

        Self {
            near_face: Plane {
                normal: camera.front(),
                distance: nalgebra::distance(
                    &(camera.pos + camera.near_z * camera.front()),
                    &origin,
                ),
            },
            far_face: Plane {
                normal: -camera.front(),
                distance: nalgebra::distance(&(camera.pos + front_mult_far), &origin),
            },
            right_face: Plane {
                normal: camera
                    .up()
                    .cross(&(front_mult_far + camera.right() * half_h_side)),
                distance,
            },
            left_face: Plane {
                normal: (front_mult_far - camera.right() * half_h_side).cross(&camera.up()),
                distance,
            },
            top_face: Plane {
                normal: camera
                    .right()
                    .cross(&(front_mult_far - camera.right() * half_v_side)),
                distance,
            },
            bottom_face: Plane {
                normal: (front_mult_far + camera.up() * half_v_side).cross(&camera.right()),
                distance,
            },
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Zeroable, Pod)]
pub struct Camera {
    pub pos: Point3<f64>,
    pub target: Point3<f64>,
    pub up: Vector3<f64>,
    pub near_z: f64,
    pub far_z: f64,
    pub asepect_ratio: f64,
    pub fov: f64,
    pub error_factor: f64,
    pub width: i64,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            pos: Point3::new(500.0, 50.0, 500.0),
            target: OPoint::origin() + -Vector3::z(),
            up: -Vector3::y(),
            near_z: 0.1,
            far_z: 10000.0,
            asepect_ratio: 16.0 / 9.0,
            fov: 60.0,
            error_factor: 0.1,
            width: 200,
        }
    }
}

impl Camera {
    pub fn frustum(&self) -> Frustum {
        Frustum::new(self)
    }

    pub fn reset(&mut self) {
        *self = Camera::default();
    }

    pub fn situate(&mut self, p: Point3<f64>) -> Vector3<f64> {
        p - self.pos
    }

    pub const fn stride(&self) -> f64 {
        //Self::STRIDE_FACTOR * (self.pos - self.target).norm().abs()
        50.0
    }

    pub const fn angle_d(&self) -> f64 {
        50.0
    }

    pub fn view_transform(&self) -> Matrix4<f64> {
        Matrix4::look_at_lh(&self.pos, &self.target, &self.up())
    }

    pub fn proj_transform(&self) -> Matrix4<f64> {
        Perspective3::new(
            self.asepect_ratio,
            self.fov.to_radians(),
            self.near_z,
            self.far_z,
        )
        .as_matrix()
        .to_owned()
    }

    pub fn up(&self) -> Vector3<f64> {
        self.up.normalize()
    }

    pub fn front(&self) -> Vector3<f64> {
        (self.target - self.pos).normalize()
    }

    pub fn right(&self) -> Vector3<f64> {
        self.front().cross(&self.up()).normalize()
    }

    pub fn set_viewport(&mut self, width: i64, height: i64) {
        self.asepect_ratio = width as f64 / height as f64;
        self.width = width;
        self.recompute_error_factor()
    }

    pub fn set_fov(&mut self, degrees: f64) {
        self.fov = degrees.to_radians();
        self.recompute_error_factor()
    }

    fn recompute_error_factor(&mut self) {
        self.error_factor = self.width as f64 / (2.0 * (self.fov / 2.0).tan())
    }

    pub fn set_near_and_far(&mut self, near_z: f64, far_z: f64) {
        self.near_z = near_z;
        self.far_z = far_z;
    }

    pub fn move_to(&mut self, pos: Point3<f64>) {
        self.pos = pos;
    }

    pub fn move_by(&mut self, dir: Vector3<f64>) {
        self.pos += dir;
    }

    pub fn shift_by(&mut self, dir: Vector3<f64>) {
        self.move_by(dir);
        self.target += dir; // correct target
    }

    pub fn look_at(&mut self, target: Point3<f64>) {
        self.target = target;
    }

    pub fn make_up(&mut self, up: Vector3<f64>) {
        self.up = up.normalize();
    }

    pub fn screen_error(&self, dist: f64, err: f64) -> f64 {
        self.error_factor * (err / dist)
    }

    pub fn world_object(&self, scale: [f32; 3]) -> vs::ty::WorldObject {
        vs::ty::WorldObject {
            model: Matrix4::new_nonuniform_scaling(&scale.into())
                .append_translation(&Vector3::zeros())
                .into(),
            view: self.view_transform().cast::<f32>().into(),
            proj: self.proj_transform().cast::<f32>().into(),
        }
    }

    pub fn move_up(&mut self) {
        self.shift_by(-self.stride() * self.up())
    }

    pub fn move_left(&mut self) {
        self.shift_by(self.stride() * self.right())
    }

    pub fn move_right(&mut self) {
        self.shift_by(-self.stride() * self.right())
    }

    pub fn move_down(&mut self) {
        self.shift_by(self.stride() * self.up.normalize())
    }

    pub fn move_forward(&mut self) {
        self.shift_by(-self.stride() * self.front())
    }

    pub fn move_backward(&mut self) {
        self.shift_by(self.stride() * self.front())
    }

    pub fn rotate_ccw_horizontally(&mut self) {
        self.target += self.angle_d() * self.right();
    }
    pub fn rotate_cw_horizontally(&mut self) {
        self.target -= self.angle_d() * self.right();
    }

    pub fn rotate_ccw_vertically(&mut self) {
        self.target += self.angle_d() * self.up();
        self.up = self.right().cross(&self.front());
    }
    pub fn rotate_cw_vertically(&mut self) {
        self.target -= self.angle_d() * self.up();
        self.up = self.right().cross(&self.front()).normalize();
    }

    pub fn rotate_ccw_sideways(&mut self) {
        let rot = nalgebra::Rotation3::new(-self.front() * self.angle_d().to_radians() * 0.2);
        self.up = (rot * self.up).normalize();
    }
    pub fn rotate_cw_sideways(&mut self) {
        let rot = nalgebra::Rotation3::new(self.front() * self.angle_d().to_radians() * 0.2);
        self.up = (rot * self.up).normalize();
    }
}
