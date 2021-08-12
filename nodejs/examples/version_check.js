// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: Copyright 2021 contributors to the Rethnet project.

//! Loads the rethnet node module and returns the version string.
//! Before running this example, you mut first build the node module by running
//! `npm run build` in the root directory of the repository.

const rethnet = require('../rethnet.node')
console.log('using rethnet version', rethnet.RETHNET_SDK_VERSION)