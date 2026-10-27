use nalgebra::{Point3, Vector3};

pub struct Aabb {
    pub min: Point3<f32>,
    pub max: Point3<f32>,
}

impl Aabb {
    pub fn intersects(&self, other: &Aabb) -> bool {
        self.min.x < other.max.x
            && self.max.x > other.min.x
            && self.min.y < other.max.y
            && self.max.y > other.min.y
            && self.min.z < other.max.z
            && self.max.z > other.min.z
    }

    pub fn sweep(&self, other: &Aabb, v: Vector3<f32>) -> Option<(f32, Vector3<f32>)> {
        let mut entry = f32::NEG_INFINITY;
        let mut exit = f32::INFINITY;
        let mut normal = Vector3::zeros();

        for i in 0..3 {
            if v[i] == 0.0 {
                // The boxes must strictly overlap on this axis.
                // If they only touch, movement along another axis cannot
                // create a collision with this obstacle.
                if self.max[i] <= other.min[i] || self.min[i] >= other.max[i] {
                    return None;
                }
                continue;
            }

            let t1 = (other.min[i] - self.max[i]) / v[i];
            let t2 = (other.max[i] - self.min[i]) / v[i];

            let axis_entry = t1.min(t2);
            let axis_exit = t1.max(t2);

            if axis_entry > entry {
                entry = axis_entry;
                normal = Vector3::zeros();
                normal[i] = if v[i] > 0.0 { -1.0 } else { 1.0 };
            }

            exit = exit.min(axis_exit);
        }

        if entry > exit || exit <= 0.0 || entry > 1.0 {
            return None;
        }

        // Reject contacts that are not approaching the obstacle.
        if entry <= 0.0 && v.dot(&normal) >= 0.0 {
            return None;
        }

        Some((entry.max(0.0), normal))
    }
}
