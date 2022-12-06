use std::ops::AddAssign;

use nalgebra::{Point3, Scalar, Vector3};
use num_traits::Float;

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

    pub fn get_vertex_p(&self, normal: Vector3<T>) -> Point3<T> {
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

    pub fn get_vertex_n(&self, normal: Vector3<T>) -> Point3<T> {
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
