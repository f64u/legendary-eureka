use std::ops::AddAssign;

use nalgebra::{Point3, Scalar, Vector3};
use num_traits::Float;

use crate::camera::Camera;

pub enum IntersectionStatus {
    Outside,
    Intersecting,
    Inside,
}

#[derive(Debug, Clone)]
pub struct AABB<T: Float + Scalar> {
    pub min: Point3<T>,
    pub max: Point3<T>,
}

impl<T> AABB<T>
where
    T: Float + Default + nalgebra::Scalar + nalgebra::ComplexField<RealField = T>,
{
    pub fn new(min: Point3<T>, max: Point3<T>) -> Self {
        Self { min, max }
    }

    pub fn get_vertex_p(&self, normal: &Vector3<T>) -> Point3<T> {
        let mut p = self.min;
        if normal.x >= T::default() {
            p.x = self.max.x;
        }

        if normal.y >= T::default() {
            p.y = self.max.y;
        }

        if normal.z >= T::default() {
            p.z = self.max.z;
        }

        p
    }

    pub fn get_vertex_n(&self, normal: &Vector3<T>) -> Point3<T> {
        let mut p = self.max;
        if normal.x >= T::default() {
            p.x = self.min.x;
        }

        if normal.y >= T::default() {
            p.y = self.min.y;
        }

        if normal.z >= T::default() {
            p.z = self.min.z;
        }

        p
    }

    pub fn distance_to_point(&self, pt: Point3<T>) -> T {
        let mut d = Point3::new(T::default(), T::default(), T::default());
        for dim in 0..3 {
            d[dim] = if pt[dim] < self.min[dim] {
                self.min[dim] - pt[dim]
            } else if self.max[dim] < pt[dim] {
                pt[dim] - self.max[dim]
            } else {
                d[dim]
            }
        }

        d.coords.magnitude()
    }

    pub fn add_pt(&mut self, pt: Point3<T>) {
        if pt.x < self.min.x {
            self.min.x = pt.x;
        } else if pt.x > self.max.x {
            self.max.x = pt.x;
        }

        if pt.y < self.min.y {
            self.min.y = pt.y;
        } else if pt.y > self.max.y {
            self.max.y = pt.y;
        }

        if pt.z < self.min.z {
            self.min.z = pt.z;
        } else if pt.z > self.max.z {
            self.max.z = pt.z;
        }
    }

    pub fn center(&self) -> Point3<T> {
        self.min + (self.max - self.min).scale(T::from(0.5).unwrap())
    }
}

impl<T: Float + Scalar> AddAssign for AABB<T> {
    fn add_assign(&mut self, rhs: Self) {
        for dim in 0..3 {
            self.min[dim] = self.min[dim].min(rhs.min[dim]);
            self.max[dim] = self.max[dim].max(rhs.max[dim]);
        }
    }
}

impl<T> AddAssign<Point3<T>> for AABB<T>
where
    T: Float + Default + nalgebra::Scalar + nalgebra::ComplexField<RealField = T>,
{
    fn add_assign(&mut self, rhs: Point3<T>) {
        self.add_pt(rhs)
    }
}

#[derive(Debug)]
pub struct Plane {
    normal: Vector3<f64>,
    point: Point3<f64>,
}

impl Plane {
    fn distance(&self, point: &Point3<f64>) -> f64 {
        self.normal.dot(&(point - self.point))
    }
}

#[derive(Debug)]
pub struct Frustum {
    pub top_face: Plane,
    pub bottom_face: Plane,

    pub left_face: Plane,
    pub right_face: Plane,

    pub near_face: Plane,
    pub far_face: Plane,
}

impl Frustum {
    pub fn new(camera: &Camera) -> Self {
        let half_h_side = camera.far_z * (camera.fov * 0.5).to_radians().tan();
        let half_v_side = half_h_side * camera.asepect_ratio;

        let front = camera.front();
        let right = camera.right();
        let front_mult_far = camera.far_z * front;

        Self {
            near_face: Plane {
                normal: front,
                point: camera.pos + camera.near_z * front,
            },
            far_face: Plane {
                normal: -front,
                point: camera.pos + front_mult_far,
            },
            right_face: Plane {
                normal: -(front_mult_far + right * half_h_side)
                    .cross(&camera.up())
                    .normalize(),
                point: camera.pos,
            },
            left_face: Plane {
                normal: -camera
                    .up()
                    .cross(&(front_mult_far - right * half_h_side))
                    .normalize(),
                point: camera.pos,
            },
            top_face: Plane {
                normal: (front_mult_far - right * half_v_side)
                    .cross(&right)
                    .normalize(),
                point: camera.pos,
            },
            bottom_face: Plane {
                normal: right
                    .cross(&(front_mult_far + camera.up() * half_v_side))
                    .normalize(),
                point: camera.pos,
            },
        }
    }

    pub fn intersect(&self, abox: &AABB<f64>) -> IntersectionStatus {
        let planes = [
            &self.far_face,
            &self.near_face,
            &self.top_face,
            &self.bottom_face,
            &self.right_face,
            &self.left_face,
        ];

        let mut intersect = false;

        for (i, plane) in planes.iter().enumerate() {
            let a = plane.distance(&abox.get_vertex_p(&plane.normal));
            let b = plane.distance(&abox.get_vertex_n(&plane.normal));
            if a < 0.0 {
                return IntersectionStatus::Outside;
            }

            if b < 0.0 {
                intersect = true;
            }
        }

        if intersect {
            IntersectionStatus::Intersecting
        } else {
            IntersectionStatus::Inside
        }
    }
}

#[cfg(test)]
mod test {
    use nalgebra::Vector3;

    use super::{Plane, AABB};

    #[test]
    fn issa_test_flight() {
        let plane = Plane {
            point: [0.0, 1.0, 0.0].into(),
            normal: [0.0, 1.0, 0.0].into(),
        };

        assert_eq!(plane.distance(&[0.0, 2.0, 1.0].into()), 1.0);
        assert_eq!(plane.distance(&[0.0, 0.0, 1.0].into()), -1.0);
    }

    #[test]
    fn issa_positive_test() {
        let abox = AABB::new([-1.0, -1.0, -1.0].into(), [1.0, 1.0, 1.0].into());
        println!(
            "{}",
            abox.get_vertex_n(&Vector3::new(1.0, 1.0, 1.0).normalize())
        )
    }

    #[test]
    fn issa_frustrating_test() {}
}
