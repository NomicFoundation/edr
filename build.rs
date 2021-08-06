// Copyright 2021 The Rethnet Authors.
// Licensed under the Apache License, Version 2.0.

//! Configures the N-API for the current crate allowing nodejs interop.

fn main() {
  napi_build::setup();
}
