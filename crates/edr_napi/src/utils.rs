//! Convenience utilities for working with N-API objects.

use napi::{
    bindgen_prelude::{ClassInstance, FromNapiValue, Object, Undefined},
    Env, NapiRaw,
};

pub trait ExplicitEitherIntoOption
where
    Self: Sized,
{
    type Output;
    fn into_option(self) -> Option<Self::Output>;
}

impl<T> ExplicitEitherIntoOption for napi::Either<T, Undefined> {
    type Output = T;
    fn into_option(self) -> Option<T> {
        match self {
            napi::Either::A(value) => Some(value),
            napi::Either::B(()) => None,
        }
    }
}

/// A convenience wrapper around the original [`ClassInstance`]
/// that holds the reference to the original object and allows
/// for easy object comparison and unwrapping the native Rust value.
pub struct ClassInstanceRef<T: 'static> {
    r#ref: napi::Ref<()>,
    marker: std::marker::PhantomData<T>,
}

impl<T> ClassInstanceRef<T> {
    /// Constructs a new value from a valid `ClassInstance` object.
    pub fn from_obj(instance: ClassInstance<T>, env: Env) -> napi::Result<ClassInstanceRef<T>> {
        let obj = instance.as_object(env);
        let r#ref = env.create_reference(obj)?;

        Ok(ClassInstanceRef {
            r#ref,
            marker: std::marker::PhantomData,
        })
    }

    /// Returns the underlying [`Object`].
    pub fn as_inner(&self, env: Env) -> napi::Result<Object> {
        env.get_reference_value::<Object>(&self.r#ref)
    }

    /// Unwraps the inner value as the original [`ClassInstance`] object.
    pub fn as_instance(&self, env: Env) -> napi::Result<ClassInstance<T>> {
        let inner = self.as_inner(env)?;
        // SAFETY: We are only constructed from valid `ClassInstance` objects
        // and we refcount the underlying raw object, so it's safe to unwrap it
        // back as the `ClassInstance`
        unsafe {
            let raw = inner.raw();
            FromNapiValue::from_napi_value(env.raw(), raw)
        }
    }

    /// Compares the underlying objects (==) for equality.
    pub fn ref_equals(&self, other: &ClassInstanceRef<T>, env: Env) -> napi::Result<bool> {
        let obj = self.as_inner(env)?;
        let other_obj = other.as_inner(env)?;

        env.strict_equals(obj, other_obj)
    }
}
