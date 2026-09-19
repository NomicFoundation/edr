//! Defining the structured error objects that cross the N-API boundary.
//!
//! Each one is a member of a discriminated union, so it carries a `kind` tag
//! whose TypeScript type is the string literal of that tag. The literal is what
//! lets a consumer narrow the union on `kind`, which is the whole reason these
//! are objects rather than strings.

/// Defines one member of a structured-error union.
///
/// Generates the `#[napi(object)]` attribute, the `kind` field and its
/// documentation, and a `new` constructor that sets the tag. A definition
/// therefore states only what is unique to it.
///
/// A member of a union discriminated by type name needs no tag of its own:
///
/// ```ignore
/// impl_structured_napi_error! {
///     /// The solc version predates the oldest Solidity grammar available.
///     pub struct TestSourceUnsupportedSolcVersion {
///         /// The solc version the source's artifact was compiled with.
///         pub version: String,
///     }
/// }
/// ```
///
/// A member of a union discriminated by role states the tag it answers to:
///
/// ```ignore
/// impl_structured_napi_error! {
///     /// A problem with the source itself.
///     pub struct TestSourceFileError tagged "source" {
///         /// The solc source name the problem was found in.
///         pub source_name: String,
///     }
/// }
/// ```
///
/// The tag's TypeScript type is derived from the tag, never written out:
/// `pastey!` rewrites the token stream before `napi_derive` parses the
/// attribute, so it can build the quoted literal that `ts_type` requires.
macro_rules! impl_structured_napi_error {
    (
        @define
        [$($type_doc:literal)*]
        $name:ident, [$($tag:tt)*], $tag_value:expr,
        [$(
            [$($field_doc:literal)*]
            $field:ident: $field_ty:ty,
        )*]
    ) => {
        pastey::paste! {
            $(#[doc = $type_doc])*
            ///
            /// Build it with [`Self::new`], which sets the `kind` tag.
            #[napi_derive::napi(object)]
            #[non_exhaustive]
            pub struct $name {
                /// Discriminant tag for the union this belongs to.
                #[napi(ts_type = "\"" $($tag)* "\"")]
                pub kind: String,
                $(
                    $(#[doc = $field_doc])*
                    pub $field: $field_ty,
                )*
            }
        }

        // A defaulted structured error would carry a tag with no problem
        // behind it, so these are built only where a problem is reported.
        #[allow(clippy::new_without_default)]
        impl $name {
            #[doc = concat!("Builds a [`", stringify!($name), "`] with its `kind` tag set.")]
            #[must_use]
            pub fn new($($field: $field_ty),*) -> Self {
                Self {
                    kind: $tag_value.to_owned(),
                    $($field),*
                }
            }
        }
    };

    (
        $(#[doc = $type_doc:literal])*
        pub struct $name:ident tagged $tag:literal {
            $(
                $(#[doc = $field_doc:literal])*
                pub $field:ident: $field_ty:ty,
            )*
        }
    ) => {
        impl_structured_napi_error! {
            @define
            [$($type_doc)*]
            $name, [$tag], $tag,
            [$([$($field_doc)*] $field: $field_ty,)*]
        }
    };

    (
        $(#[doc = $type_doc:literal])*
        pub struct $name:ident {
            $(
                $(#[doc = $field_doc:literal])*
                pub $field:ident: $field_ty:ty,
            )*
        }
    ) => {
        impl_structured_napi_error! {
            @define
            [$($type_doc)*]
            $name, [$name], stringify!($name),
            [$([$($field_doc)*] $field: $field_ty,)*]
        }
    };
}

pub(crate) use impl_structured_napi_error;
