# Style Guide

This is a style guide for the EDR project.

## Procedural derive macros

When deriving multiple traits, use the following rules to order them:

1. Standard library traits before external crates
2. Sub-traits before super-traits
3. Alphabetical order

For example:

```rust
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
```

Or, for sub- and super-traits:

```rust
#[derive(PartialEq, Eq, PartialOrd, Ord)]
```

## Member ordering

When adding a variant to an `enum` or a field to a `struct` or enum variant, by default follow alphabetical order. If it makes more sense to follow custom ordering, feel free to do so.

### Member functions

For member functions, use the following default rules to order them:

1. Public members before private members
2. Alphabetical order

Again, if it makes more sense to follow custom ordering, feel free to do so.

### Example

```rust
struct Foo {
    bar: u32,
    baz: u32,
}

impl Foo {
    pub fn bar_mut(&mut self) -> &mut u32 {
        &mut self.bar
    }

    pub fn baz(&self) -> u32 {
        self.baz
    }

    fn bar(&self) -> u32 {
        self.bar
    }
}
```

## Use `#[napi(catch_unwind)]`

Annotate every function/method exposed from `edr_napi` to JS with the `catch_unwind` NAPI-RS macro attribute.

### Example

```rust
#[napi(catch_unwind)]
pub fn foo() {
  // The panic is turned into an error thrown on the JS side
  panic!("panic message")
}
```

### Rationale

Rust functions that panic on the Node.js main thread will not return a result and crash the Node.js process.

Annotating with the `catch_unwind` macro attribute will turn the panic into a `napi::Error`, which can then be returned as a `napi::Result`.

Note that this will not capture panics in background threads.
