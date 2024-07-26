//! Convenience utilities for working with N-API objects.

use std::{
    cell::RefCell,
    ops::{Deref, DerefMut},
};

use napi::{
    bindgen_prelude::{ClassInstance, FromNapiValue, Object},
    Env, NapiRaw,
};

/// A convenience wrapper around the original [`ClassInstance`]
/// that holds the reference to the original object and allows
/// for easy object comparison and unwrapping the native Rust value.
pub struct ClassInstanceRef<T: 'static> {
    r#ref: napi::Ref<()>,
    marker: std::marker::PhantomData<T>,
    // Best effort to ensure that the object is not aliased when unwrapped
    // and that it's not mutably borrowed when already borrowed immutably
    cell: RefCell<()>,
}

pub struct ClassInstanceRefGuard<'a, T> {
    _guard: std::cell::Ref<'a, ()>,
    ptr: *const T,
}

impl<T> Deref for ClassInstanceRefGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: We are holding the ref lock for the duration of the guard, so
        // it's safe to dereference the pointer.
        unsafe { &*self.ptr }
    }
}

pub struct ClassInstanceRefMutGuard<'a, T> {
    _guard: std::cell::RefMut<'a, ()>,
    ptr: *mut T,
}

impl<T> Deref for ClassInstanceRefMutGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: We are holding the ref lock for the duration of the guard, so
        // it's safe to dereference the pointer.
        unsafe { &*self.ptr }
    }
}

impl<T> DerefMut for ClassInstanceRefMutGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: We are holding the ref mut lock for the duration of the guard,
        // so it's safe to dereference the pointer.
        unsafe { &mut *self.ptr }
    }
}

impl<T> ClassInstanceRef<T> {
    /// Constructs a new value from a valid `ClassInstance` object.
    pub fn from_obj(instance: ClassInstance<T>, env: Env) -> napi::Result<ClassInstanceRef<T>> {
        let obj = instance.as_object(env);
        let r#ref = env.create_reference(obj)?;

        Ok(ClassInstanceRef {
            r#ref,
            marker: std::marker::PhantomData,
            cell: RefCell::new(()),
        })
    }

    /// Returns the underlying [`Object`].
    pub fn as_object(&self, env: Env) -> napi::Result<Object> {
        env.get_reference_value::<Object>(&self.r#ref)
    }

    pub fn borrow(&self, env: Env) -> napi::Result<ClassInstanceRefGuard<'_, T>> {
        let _guard = self.cell.try_borrow().map_err(|_e| {
            napi::Error::from_reason(format!(
                "Cannot borrow a reference immutably when already borrowed mutably: {}",
                std::any::type_name::<T>()
            ))
        })?;

        // SAFETY: This actually manifests a &'static mut T internally, which is
        // clearly wrong in general and there may be other immutable references
        // already alive and pointing to the underlying native object.
        // This is a bit of a hack, but it's the best we can do without calling
        // `napi_unwrap` directly.
        let ptr = unsafe { &*self.as_instance(env)? } as *const T;

        Ok(ClassInstanceRefGuard { _guard, ptr })
    }

    pub fn borrow_mut(&self, env: Env) -> napi::Result<ClassInstanceRefMutGuard<'_, T>> {
        let _guard = self.cell.try_borrow_mut().map_err(|_e| {
            napi::Error::from_reason(format!(
                "Cannot borrow a reference mutably when already borrowed immutably: {}",
                std::any::type_name::<T>()
            ))
        })?;

        // SAFETY: Converts an internal &'static mut T to a mutable reference
        // with a shorter lifetime.
        let ptr = unsafe { &mut *self.as_instance(env)? } as *mut T;

        Ok(ClassInstanceRefMutGuard { _guard, ptr })
    }

    /// Unwraps the inner value as the original [`ClassInstance`] object.
    unsafe fn as_instance(&self, env: Env) -> napi::Result<ClassInstance<T>> {
        let inner = self.as_object(env)?;
        // SAFETY: We are only constructed from valid `ClassInstance` objects
        // and this does the opposite. Uses the wrapped object that's refcounted,
        // so alive.
        unsafe {
            let raw = inner.raw();
            FromNapiValue::from_napi_value(env.raw(), raw)
        }
    }

    /// Compares the underlying objects (==) for equality.
    pub fn ref_equals(&self, other: &ClassInstanceRef<T>, env: Env) -> napi::Result<bool> {
        let obj = self.as_object(env)?;
        let other_obj = other.as_object(env)?;

        env.strict_equals(obj, other_obj)
    }
}
