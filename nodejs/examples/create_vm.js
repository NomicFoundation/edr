// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: Copyright 2021 contributors to the Rethnet project.

const rethnet = require('../rethnet.node')

const networkConfig = {
    chainId: 3, // ropsen
    hardfork: 'london'
}

const genesisBlockConfig = {
    block_hashes: [],
    block_number: '0x0000000000000000000000000000000000000001',
    block_coinbase: '0x2adc25665018aa1fe0e6bc666dac8fc2697ff9ba',
    block_timestamp: '0x03e8',
    block_difficulty: '0x020000',
    block_gas_limit: '0x3b9aca00'
}

const vm = new rethnet.VirtualMachine(networkConfig, genesisBlockConfig)
const network = vm.network

console.log(vm)
console.log(network)