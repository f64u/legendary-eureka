use std::ops::AddAssign;

use nalgebra::Vector3;
use num_traits::Float;

#[derive(Debug, Clone)]
pub struct AABB<T: Float> {
    pub min: Vector3<T>,
    pub max: Vector3<T>,
}

impl<T> AABB<T>
where
    T: Float + Default + nalgebra::Scalar + nalgebra::ComplexField<RealField = T>,
{
    pub fn new(min: Vector3<T>, max: Vector3<T>) -> Self {
        Self { min, max }
    }

    pub fn distance_to_point(&self, pt: Vector3<T>) -> T {
        let mut d = Vector3::new(T::default(), T::default(), T::default());
        for dim in 0..3 {
            d[dim] = if pt[dim] < self.min[dim] {
                self.min[dim] - pt[dim]
            } else if self.max[dim] < pt[dim] {
                pt[dim] - self.max[dim]
            } else {
                d[dim]
            }
        }

        d.magnitude()
    }

    pub fn add_pt(&mut self, pt: Vector3<T>) {
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

    pub fn center(&self) -> Vector3<T> {
        (self.min + self.max).scale(T::from(0.5).unwrap())
    }
}

impl<T: Float> AddAssign for AABB<T> {
    fn add_assign(&mut self, rhs: Self) {
        for dim in 0..3 {
            self.min[dim] = self.min[dim].min(rhs.min[dim]);
            self.max[dim] = self.max[dim].max(rhs.max[dim]);
        }
    }
}

impl<T> AddAssign<Vector3<T>> for AABB<T>
where
    T: Float + Default + nalgebra::Scalar + nalgebra::ComplexField<RealField = T>,
{
    fn add_assign(&mut self, rhs: Vector3<T>) {
        self.add_pt(rhs)
    }
}
