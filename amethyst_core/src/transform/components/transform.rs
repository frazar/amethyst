//! Local transform component.
use std::fmt;

use crate::{
    alga::num::{One, Zero},
    ecs::prelude::{Component, DenseVecStorage, FlaggedStorage},
    float::{Float, FloatBase},
    math::{
        self as na, ComplexField, Isometry3, Matrix4, Quaternion, Translation3, Unit,
        UnitQuaternion, Vector3,
    },
};
use serde::{
    de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor},
    ser::{Serialize, Serializer},
};

/// Local position, rotation, and scale (from parent if it exists).
///
/// Used for rendering position and orientation.
///
/// The transforms are preformed in this order: scale, then rotation, then translation.
#[derive(Getters, Setters, MutGetters, Clone, Debug, PartialEq)]
pub struct Transform {
    /// Translation + rotation value
    #[get = "pub"]
    #[set = "pub"]
    #[get_mut = "pub"]
    isometry: Isometry3<Float>,
    /// Scale vector
    #[get = "pub"]
    #[set = "pub"]
    #[get_mut = "pub"]
    scale: Vector3<Float>,
    /// The global transformation matrix.
    #[get = "pub"]
    pub(crate) global_matrix: Matrix4<Float>,
}

impl Transform {
    /// Create a new Transform.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use amethyst_core::transform::Transform;
    /// # use amethyst_core::math::{Isometry3, Translation3, UnitQuaternion, Vector3};
    /// let position = Translation3::new(0.0, 2.0, 4.0);
    /// let rotation = UnitQuaternion::from_euler_angles(0.4, 0.2, 0.0);
    /// let scale = Vector3::new(1.0, 1.0, 1.0);
    ///
    /// let t = Transform::new(position, rotation, scale);
    ///
    /// assert_eq!(t.translation().y, 2.0);
    /// ```
    pub fn new(
        position: Translation3<Float>,
        rotation: UnitQuaternion<Float>,
        scale: Vector3<Float>,
    ) -> Self {
        Transform {
            isometry: Isometry3::from_parts(position, rotation),
            scale,
            global_matrix: na::one(),
        }
    }

    /// Makes the entity point towards `target`.
    ///
    /// `up` says which direction the entity should be 'rolled' to once it is pointing at
    /// `position`. If `up` is parallel to the direction the entity is looking, the result will be
    /// garbage.
    ///
    /// This function only works with respect to the coordinate system of its parent, so when used
    /// with an object that's not a sibling it will not do what you expect.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use amethyst_core::transform::Transform;
    /// # use amethyst_core::math::{UnitQuaternion, Quaternion, Vector3};
    /// let mut t = Transform::default();
    /// // No rotation by default
    /// assert_eq!(*t.rotation().quaternion(), Quaternion::identity());
    /// // look up with up pointing backwards
    /// t.face_towards(Vector3::new(0.0, 1.0, 0.0), Vector3::new(0.0, 0.0, 1.0));
    /// // our rotation should match the angle from straight ahead to straight up
    /// let rotation = UnitQuaternion::rotation_between(
    ///     &Vector3::new(0.0, 1.0, 0.0),
    ///     &Vector3::new(0.0, 0.0, 1.0),
    /// ).unwrap();
    /// assert_eq!(*t.rotation(), rotation);
    /// // now if we move forwards by 1.0, we'll end up at the point we are facing
    /// // (modulo some floating point error)
    /// t.move_forward(1.0);
    /// assert!((*t.translation() - Vector3::new(0.0, 1.0, 0.0)).magnitude() <= 0.0001);
    /// ```
    #[inline]
    pub fn face_towards(&mut self, target: Vector3<Float>, up: Vector3<Float>) -> &mut Self {
        self.isometry.rotation =
            UnitQuaternion::face_towards(&(self.isometry.translation.vector - target), &up);
        self
    }

    /// Returns the local object matrix for the transform.
    ///
    /// Combined with the parent's `GlobalTransform` component it gives
    /// the global (or world) matrix for the current entity.
    #[inline]
    pub fn matrix(&self) -> Matrix4<Float> {
        self.isometry
            .to_homogeneous()
            .prepend_nonuniform_scaling(&self.scale)
    }

    /// Returns a reference to the translation vector.
    #[inline]
    pub fn translation(&self) -> &Vector3<Float> {
        &self.isometry.translation.vector
    }

    /// Returns a mutable reference to the translation vector.
    #[inline]
    pub fn translation_mut(&mut self) -> &mut Vector3<Float> {
        &mut self.isometry.translation.vector
    }

    /// Returns a reference to the rotation quaternion.
    #[inline]
    pub fn rotation(&self) -> &UnitQuaternion<Float> {
        &self.isometry.rotation
    }

    /// Returns a mutable reference to the rotation quaternion.
    #[inline]
    pub fn rotation_mut(&mut self) -> &mut UnitQuaternion<Float> {
        &mut self.isometry.rotation
    }

    /// Move relatively to its current position, but the parent's (or
    /// global, if no parent exists) orientation.
    ///
    /// For example, if the object is rotated 45 degrees about its Y axis,
    /// then you *prepend* a translation along the Z axis, it will still
    /// move along the parent's Z axis rather than its local Z axis (which
    /// is rotated 45 degrees).
    #[inline]
    pub fn prepend_translation(&mut self, translation: Vector3<Float>) -> &mut Self {
        self.isometry.translation.vector += translation;
        self
    }

    /// Move relatively to its current position and orientation.
    ///
    /// For example, if the object is rotated 45 degrees about its Y axis,
    /// then you append a translation along the Z axis, that Z axis is now
    /// rotated 45 degrees, and so the appended translation will go along that
    /// rotated Z axis.
    ///
    /// Equivalent to rotating the translation by the transform's current
    /// rotation before applying.
    #[inline]
    pub fn append_translation(&mut self, translation: Vector3<Float>) -> &mut Self {
        self.isometry.translation.vector += self.isometry.rotation * translation;
        self
    }

    /// Move a distance along an axis relative to the parent's orientation
    /// (or the global orientation if no parent exists).
    ///
    /// For example, if the object is rotated 45 degrees about its Y axis,
    /// then you *prepend* a translation along the Z axis, it will still
    /// move along the parent's Z axis rather than its local Z axis (which
    /// is rotated 45 degrees).
    #[inline]
    pub fn prepend_translation_along<N: Into<Float>>(
        &mut self,
        direction: Unit<Vector3<Float>>,
        distance: N,
    ) -> &mut Self {
        self.isometry.translation.vector += direction.as_ref() * distance.into();
        self
    }

    /// Move a distance along an axis relative to the local orientation.
    #[inline]
    pub fn append_translation_along<N: Into<Float>>(
        &mut self,
        direction: Unit<Vector3<Float>>,
        distance: N,
    ) -> &mut Self {
        self.isometry.translation.vector +=
            self.isometry.rotation * direction.as_ref() * distance.into();
        self
    }

    /// Move forward relative to current position and orientation.
    #[inline]
    pub fn move_forward<N: Into<Float>>(&mut self, amount: N) -> &mut Self {
        // sign is reversed because z comes towards us
        self.append_translation(Vector3::new(0.0.into(), 0.0.into(), -amount.into()))
    }

    /// Move backward relative to current position and orientation.
    #[inline]
    pub fn move_backward<N: Into<Float>>(&mut self, amount: N) -> &mut Self {
        self.append_translation(Vector3::new(0.0.into(), 0.0.into(), amount.into()))
    }

    /// Move right relative to current position and orientation.
    #[inline]
    pub fn move_right<N: Into<Float>>(&mut self, amount: N) -> &mut Self {
        self.append_translation(Vector3::new(amount.into(), 0.0.into(), 0.0.into()))
    }

    /// Move left relative to current position and orientation.
    #[inline]
    pub fn move_left<N: Into<Float>>(&mut self, amount: N) -> &mut Self {
        self.append_translation(Vector3::new(-amount.into(), 0.0.into(), 0.0.into()))
    }

    /// Move up relative to current position and orientation.
    #[inline]
    pub fn move_up<N: Into<Float>>(&mut self, amount: N) -> &mut Self {
        self.append_translation(Vector3::new(0.0.into(), amount.into(), 0.0.into()))
    }

    /// Move down relative to current position and orientation.
    #[inline]
    pub fn move_down<N: Into<Float>>(&mut self, amount: N) -> &mut Self {
        self.append_translation(Vector3::new(0.0.into(), -amount.into(), 0.0.into()))
    }

    /// Adds the specified amount to the translation vector's x component.
    /// i.e. move relative to the parent's (or global, if no parent exists)
    /// x axis.
    #[inline]
    pub fn prepend_translation_x<N: Into<Float>>(&mut self, amount: N) -> &mut Self {
        self.isometry.translation.vector.x += amount.into();
        self
    }

    /// Adds the specified amount to the translation vector's y component.
    /// i.e. move relative to the parent's (or global, if no parent exists)
    /// y axis.
    #[inline]
    pub fn prepend_translation_y<N: Into<Float>>(&mut self, amount: N) -> &mut Self {
        self.isometry.translation.vector.y += amount.into();
        self
    }

    /// Adds the specified amount to the translation vector's z component.
    /// i.e. move relative to the parent's (or global, if no parent exists)
    /// z axis.
    #[inline]
    pub fn prepend_translation_z<N: Into<Float>>(&mut self, amount: N) -> &mut Self {
        self.isometry.translation.vector.z += amount.into();
        self
    }

    /// Sets the translation vector's x component to the specified value.
    #[inline]
    pub fn set_translation_x<N: Into<Float>>(&mut self, value: N) -> &mut Self {
        self.isometry.translation.vector.x = value.into();
        self
    }

    /// Sets the translation vector's y component to the specified value.
    #[inline]
    pub fn set_translation_y<N: Into<Float>>(&mut self, value: N) -> &mut Self {
        self.isometry.translation.vector.y = value.into();
        self
    }

    /// Sets the translation vector's z component to the specified value.
    #[inline]
    pub fn set_translation_z<N: Into<Float>>(&mut self, value: N) -> &mut Self {
        self.isometry.translation.vector.z = value.into();
        self
    }

    /// Premultiply a rotation about the x axis, i.e. perform a rotation about
    /// the parent's x axis (or the global x axis if no parent exists).
    ///
    /// `delta_angle` is specified in radians.
    #[inline]
    pub fn prepend_rotation_x_axis<N: Into<Float>>(&mut self, delta_angle: N) -> &mut Self {
        self.prepend_rotation(Vector3::x_axis(), delta_angle.into())
    }

    /// Postmultiply a rotation about the x axis, i.e. perform a rotation about
    /// the *local* x-axis, including any prior rotations that have been performed.
    ///
    /// `delta_angle` is specified in radians.
    #[inline]
    pub fn append_rotation_x_axis<N: Into<Float>>(&mut self, delta_angle: N) -> &mut Self {
        self.append_rotation(Vector3::x_axis(), delta_angle.into())
    }

    /// Set the rotation about the parent's x axis (or the global x axis
    /// if no parent exists). This will *clear any other rotations that have
    /// previously been performed*!
    ///
    /// `angle` is specified in radians.
    #[inline]
    pub fn set_rotation_x_axis<N: Into<Float>>(&mut self, angle: N) -> &mut Self {
        self.set_rotation_euler(angle.into(), Float::zero(), Float::zero())
    }

    /// Premultiply a rotation about the y axis, i.e. perform a rotation about
    /// the parent's y axis (or the global y axis if no parent exists).
    ///
    /// `delta_angle` is specified in radians.
    #[inline]
    pub fn prepend_rotation_y_axis<N: Into<Float>>(&mut self, delta_angle: N) -> &mut Self {
        self.prepend_rotation(Vector3::y_axis(), delta_angle.into())
    }

    /// Postmultiply a rotation about the y axis, i.e. perform a rotation about
    /// the *local* y-axis, including any prior rotations that have been performed.
    ///
    /// `delta_angle` is specified in radians.
    #[inline]
    pub fn append_rotation_y_axis<N: Into<Float>>(&mut self, delta_angle: N) -> &mut Self {
        self.append_rotation(Vector3::y_axis(), delta_angle.into())
    }

    /// Set the rotation about the parent's y axis (or the global y axis
    /// if no parent exists). This will *clear any other rotations that have
    /// previously been performed*!
    ///
    /// `angle` is specified in radians.
    #[inline]
    pub fn set_rotation_y_axis<N: Into<Float>>(&mut self, angle: N) -> &mut Self {
        self.set_rotation_euler(0.0.into(), angle.into(), 0.0.into())
    }

    /// Premultiply a rotation about the z axis, i.e. perform a rotation about
    /// the parent's z axis (or the global z axis if no parent exists).
    ///
    /// `delta_angle` is specified in radians.
    #[inline]
    pub fn prepend_rotation_z_axis<N: Into<Float>>(&mut self, delta_angle: N) -> &mut Self {
        self.prepend_rotation(-Vector3::z_axis(), delta_angle.into())
    }

    /// Postmultiply a rotation about the z axis, i.e. perform a rotation about
    /// the *local* z-axis, including any prior rotations that have been performed.
    ///
    /// `delta_angle` is specified in radians.
    #[inline]
    pub fn append_rotation_z_axis<N: Into<Float>>(&mut self, delta_angle: N) -> &mut Self {
        self.append_rotation(-Vector3::z_axis(), delta_angle.into())
    }

    /// Set the rotation about the parent's z axis (or the global z axis
    /// if no parent exists). This will *clear any other rotations that have
    /// previously been performed*!
    ///
    /// `angle` is specified in radians.
    #[inline]
    pub fn set_rotation_z_axis<N: Into<Float>>(&mut self, angle: N) -> &mut Self {
        self.set_rotation_euler(0.0.into(), 0.0.into(), angle.into())
    }

    /// Perform a rotation about the axis perpendicular to X and Y,
    /// i.e. the most common way to rotate an object in a 2d game.
    ///
    /// `delta_angle` is specified in radians.
    #[inline]
    pub fn rotate_2d<N: Into<Float>>(&mut self, delta_angle: N) -> &mut Self {
        self.prepend_rotation_z_axis(delta_angle.into())
    }

    /// Set the rotation about the axis perpendicular to X and Y,
    /// i.e. the most common way to rotate an object in a 2d game.
    ///
    /// `angle` is specified in radians.
    #[inline]
    pub fn set_rotation_2d<N: Into<Float>>(&mut self, angle: N) -> &mut Self {
        self.set_rotation_euler(0.0.into(), 0.0.into(), angle.into())
    }

    /// Premultiply a rotation, i.e. rotate relatively to the parent's orientation
    /// (or the global orientation if no parent exists), about a specified axis.
    ///
    /// `delta_angle` is specified in radians.
    #[inline]
    pub fn prepend_rotation<N: Into<Float>>(
        &mut self,
        axis: Unit<Vector3<Float>>,
        angle: N,
    ) -> &mut Self {
        let q = UnitQuaternion::from_axis_angle(&axis, angle.into());
        self.isometry.rotation = q * self.isometry.rotation;
        self
    }

    /// Postmultiply a rotation, i.e. rotate relatively to the local orientation (the
    /// currently applied rotations), about a specified axis.
    ///
    /// `delta_angle` is specified in radians.
    #[inline]
    pub fn append_rotation<N: Into<Float>>(
        &mut self,
        axis: Unit<Vector3<Float>>,
        angle: N,
    ) -> &mut Self {
        self.isometry.rotation *= UnitQuaternion::from_axis_angle(&axis, angle.into());
        self
    }

    /// Set the position.
    pub fn set_translation(&mut self, position: Vector3<Float>) -> &mut Self {
        self.isometry.translation.vector = position;
        self
    }

    /// Adds the specified amounts to the translation vector.
    pub fn append_translation_xyz<N: Into<Float>>(&mut self, x: N, y: N, z: N) -> &mut Self {
        self.append_translation(Vector3::new(x.into(), y.into(), z.into()));
        self
    }

    /// Sets the specified values of the translation vector.
    pub fn set_translation_xyz<N: Into<Float>>(&mut self, x: N, y: N, z: N) -> &mut Self {
        self.set_translation(Vector3::new(x.into(), y.into(), z.into()))
    }

    /// Sets the rotation of the transform.
    pub fn set_rotation(&mut self, rotation: UnitQuaternion<Float>) -> &mut Self {
        self.isometry.rotation = rotation;
        self
    }

    /// Set the rotation using x, y, z Euler axes.
    ///
    /// All angles are specified in radians. Euler order is x → y → z.
    ///
    /// # Arguments
    ///
    ///  - x - The angle to apply around the x axis.
    ///  - y - The angle to apply around the y axis.
    ///  - z - The angle to apply around the z axis.
    ///
    /// # Note on Euler angle semantics and `nalgebra`
    ///
    /// `nalgebra` has a few methods related to Euler angles, and they use
    /// roll, pitch, and yaw as arguments instead of x, y, and z axes specifically.
    /// Yaw has the semantic meaning of rotation about the "up" axis, roll about the
    /// "forward axis", and pitch about the "right" axis respectively. However, `nalgebra`
    /// assumes a +Z = up coordinate system for its roll, pitch, and yaw semantics, while
    /// Amethyst uses a +Y = up coordinate system. Therefore, the `nalgebra` Euler angle
    /// methods are slightly confusing to use in concert with Amethyst, and so we've
    /// provided our own with semantics that match the rest of Amethyst. If you do end up
    /// using `nalgebra`'s `euler_angles` or `from_euler_angles` methods, be aware that
    /// 'roll' in that context will mean rotation about the x axis, 'pitch' will mean
    /// rotation about the y axis, and 'yaw' will mean rotation about the z axis.
    ///
    /// ```
    /// # use amethyst_core::transform::Transform;
    /// let mut transform = Transform::default();
    ///
    /// transform.set_rotation_euler(1.0, 0.0, 0.0);
    ///
    /// assert_eq!(transform.rotation().euler_angles().0, 1.0);
    /// ```
    pub fn set_rotation_euler<N: Into<Float>>(&mut self, x: N, y: N, z: N) -> &mut Self {
        self.isometry.rotation = UnitQuaternion::from_euler_angles(x.into(), y.into(), z.into());
        self
    }

    /// Get the Euler angles of the current rotation. Returns
    /// in a tuple of the form (x, y, z), where `x`, `y`, and `z`
    /// are the current rotation about that axis in radians.
    ///
    /// # Note on Euler angle semantics and `nalgebra`
    ///
    /// `nalgebra` has a few methods related to Euler angles, and they use
    /// roll, pitch, and yaw as arguments instead of x, y, and z axes specifically.
    /// Yaw has the semantic meaning of rotation about the "up" axis, roll about the
    /// "forward axis", and pitch about the "right" axis respectively. However, `nalgebra`
    /// assumes a +Z = up coordinate system for its roll, pitch, and yaw semantics, while
    /// Amethyst uses a +Y = up coordinate system. Therefore, the `nalgebra` Euler angle
    /// methods are slightly confusing to use in concert with Amethyst, and so we've
    /// provided our own with semantics that match the rest of Amethyst. If you do end up
    /// using `nalgebra`'s `euler_angles` or `from_euler_angles` methods, be aware that
    /// 'roll' in that context will mean rotation about the x axis, 'pitch' will mean
    /// rotation about the y axis, and 'yaw' will mean rotation about the z axis.
    pub fn euler_angles(&self) -> (Float, Float, Float) {
        self.isometry.rotation.euler_angles()
    }

    /// Concatenates another transform onto `self`.
    ///
    /// Concatenating is roughly equivalent to doing matrix multiplication except for the fact that
    /// it's done on `Transform` which is decomposed.
    pub fn concat(&mut self, other: &Self) -> &mut Self {
        // The order of these is somewhat important as the translation relies on the rotation and
        // scaling not having been modified already.
        self.isometry.translation.vector +=
            self.isometry.rotation * other.isometry.translation.vector.component_mul(&self.scale);
        self.scale.component_mul_assign(&other.scale);
        self.isometry.rotation *= other.isometry.rotation;
        self
    }

    /// Verifies that the global `Matrix4` doesn't contain any NaN values.
    pub fn is_finite(&self) -> bool {
        self.global_matrix
            .as_slice()
            .iter()
            .all(|f| Float::is_finite(f))
    }

    /// Calculates the inverse of this transform, which we need to render.
    ///
    /// We can exploit the extra information we have to perform this inverse faster than `O(n^3)`.
    pub fn view_matrix(&self) -> Matrix4<Float> {
        // TODO: check if this actually is faster
        let inv_scale = Vector3::new(
            Float(1.0) / self.scale.x,
            Float(1.0) / self.scale.y,
            Float(1.0) / self.scale.z,
        );
        self.isometry
            .inverse()
            .to_homogeneous()
            .append_nonuniform_scaling(&inv_scale)
    }
}

impl Default for Transform {
    /// The default transform does nothing when used to transform an entity.
    fn default() -> Self {
        Transform {
            isometry: Isometry3::identity(),
            scale: Vector3::from_element(1.0.into()),
            global_matrix: na::one(),
        }
    }
}

impl Component for Transform {
    type Storage = FlaggedStorage<Self, DenseVecStorage<Self>>;
}

/// Creates a Transform using the `Vector3` as the translation vector.
///
/// ```
/// # use amethyst_core::transform::Transform;
/// # use amethyst_core::math::Vector3;
/// let transform = Transform::from(Vector3::new(100.0, 200.0, 300.0));
///
/// assert_eq!(transform.translation().x, 100.0);
/// ```
impl From<Vector3<Float>> for Transform {
    fn from(translation: Vector3<Float>) -> Self {
        Transform {
            isometry: Isometry3::new(translation, na::zero()),
            ..Default::default()
        }
    }
}

impl<'de> Deserialize<'de> for Transform {
    fn deserialize<D>(deserializer: D) -> Result<Transform, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Translation,
            Rotation,
            Scale,
        };

        #[derive(Default)]
        struct TransformVisitor {}

        impl<'de> Visitor<'de> for TransformVisitor {
            type Value = Transform;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("struct Transform")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let translation: [FloatBase; 3] = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let rotation: [FloatBase; 4] = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                let scale: [FloatBase; 3] = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(2, &self))?;

                let isometry = Isometry3::from_parts(
                    Translation3::new(
                        Float(translation[0]),
                        Float(translation[1]),
                        Float(translation[2]),
                    ),
                    Unit::new_normalize(Quaternion::new(
                        Float(rotation[3]),
                        Float(rotation[0]),
                        Float(rotation[1]),
                        Float(rotation[2]),
                    )),
                );
                let scale = Vector3::new(scale[0].into(), scale[1].into(), scale[2].into());

                Ok(Transform {
                    isometry,
                    scale,
                    ..Default::default()
                })
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut translation = None;
                let mut rotation = None;
                let mut scale = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Translation => {
                            if translation.is_some() {
                                return Err(de::Error::duplicate_field("translation"));
                            }
                            translation = Some(map.next_value()?);
                        }
                        Field::Rotation => {
                            if rotation.is_some() {
                                return Err(de::Error::duplicate_field("rotation"));
                            }
                            rotation = Some(map.next_value()?);
                        }
                        Field::Scale => {
                            if scale.is_some() {
                                return Err(de::Error::duplicate_field("scale"));
                            }
                            scale = Some(map.next_value()?);
                        }
                    }
                }
                let translation: [FloatBase; 3] = translation.unwrap_or([FloatBase::zero(); 3]);
                let rotation: [FloatBase; 4] = rotation.unwrap_or([
                    FloatBase::one(),
                    FloatBase::zero(),
                    FloatBase::zero(),
                    FloatBase::zero(),
                ]);
                let scale: [FloatBase; 3] = scale.unwrap_or([FloatBase::one(); 3]);

                let isometry = Isometry3::from_parts(
                    Translation3::new(
                        Float(translation[0]),
                        Float(translation[1]),
                        Float(translation[2]),
                    ),
                    Unit::new_normalize(Quaternion::new(
                        Float(rotation[3]),
                        Float(rotation[0]),
                        Float(rotation[1]),
                        Float(rotation[2]),
                    )),
                );
                let scale = Vector3::new(scale[0].into(), scale[1].into(), scale[2].into());

                Ok(Transform {
                    isometry,
                    scale,
                    ..Default::default()
                })
            }
        }

        const FIELDS: &'static [&'static str] = &["translation", "rotation", "scale"];
        deserializer.deserialize_struct("Transform", FIELDS, TransformVisitor::default())
    }
}

impl Serialize for Transform {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct TransformValues {
            translation: [FloatBase; 3],
            rotation: [FloatBase; 4],
            scale: [FloatBase; 3],
        }

        let pos: [Float; 3] = self.isometry.translation.vector.into();
        let rot: [Float; 4] = self.isometry.rotation.as_ref().coords.into();
        let scale: [Float; 3] = self.scale.into();

        Serialize::serialize(
            &TransformValues {
                translation: [pos[0].0, pos[1].0, pos[2].0],
                rotation: [rot[0].0, rot[1].0, rot[2].0, rot[3].0],
                scale: [scale[0].0, scale[1].0, scale[2].0],
            },
            serializer,
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        approx::*,
        math::{UnitQuaternion, Vector3},
        Transform,
    };

    /// Sanity test for concat operation
    #[test]
    fn test_mul() {
        // For the condition to hold both scales must be uniform
        let mut first = Transform::default();
        first.set_translation_xyz(20., 10., -3.);
        first.set_scale(Vector3::new(2., 2., 2.));
        first.set_rotation(
            UnitQuaternion::rotation_between(&Vector3::new(-1., 1., 2.), &Vector3::new(1., 0., 0.))
                .unwrap(),
        );

        let mut second = Transform::default();
        second.set_translation_xyz(2., 1., -3.);
        second.set_scale(Vector3::new(1., 1., 1.));
        second.set_rotation(
            UnitQuaternion::rotation_between(&Vector3::new(7., -1., 3.), &Vector3::new(2., 1., 1.))
                .unwrap(),
        );

        // check Mat(first * second) == Mat(first) * Mat(second)
        assert_relative_eq!(
            first.matrix() * second.matrix(),
            first.concat(&second).matrix(),
            max_relative = 0.0000000000001,
        );
        assert_relative_eq!(
            first.matrix() * second.matrix(),
            first.concat(&second).matrix(),
            max_relative = 0.0000000000001,
        );
    }

    #[test]
    fn test_view_matrix() {
        let mut transform = Transform::default();
        transform.set_translation_xyz(5.0, 70.1, 43.7);
        transform.set_scale(Vector3::new(1.0, 5.0, 8.9));
        transform.set_rotation(
            UnitQuaternion::rotation_between(&Vector3::new(-1., 1., 2.), &Vector3::new(1., 0., 0.))
                .unwrap(),
        );

        assert_ulps_eq!(
            transform.matrix().try_inverse().unwrap(),
            transform.view_matrix(),
        );
    }

    #[test]
    fn ser_deser() {
        let mut transform = Transform::default();
        transform.set_translation_xyz(1.0, 2.0, 3.0);
        transform.set_scale(Vector3::new(4.0, 5.0, 6.0));
        transform.set_rotation(
            UnitQuaternion::rotation_between(&Vector3::new(-1., 1., 2.), &Vector3::new(1., 0., 0.))
                .unwrap(),
        );
        let s: String =
            ron::ser::to_string_pretty(&transform, ron::ser::PrettyConfig::default()).unwrap();
        let transform2: Transform = ron::de::from_str(&s).unwrap();

        assert_eq!(transform, transform2);
    }

    #[test]
    fn is_finite() {
        let mut transform = Transform::default();
        assert!(transform.is_finite());

        transform.global_matrix.fill_row(2, std::f32::NAN);
        assert!(!transform.is_finite());
    }
}
