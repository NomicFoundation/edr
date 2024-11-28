// Trait for retrieving a sub-type from a higher-kind type.
pub trait HigherKinded<ParamT> {
    type Type;
}
