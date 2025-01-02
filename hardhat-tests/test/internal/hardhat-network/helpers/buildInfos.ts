export const exampleBuildInfo = {
  id: "6dccf5d5d3b5e72e738c81b1acfbf389",
  _format: "hh-sol-build-info-1",
  solcVersion: "0.8.0",
  solcLongVersion: "0.8.0+commit.c7dfd78e",
  input: {
    language: "Solidity",
    sources: {
      "contracts/Example.sol": {
        content:
          "// SPDX-License-Identifier: Unlicense\npragma solidity ^0.8.0;\ncontract Example {\n    uint public x;\n\n    function inc() public {\n        x++;\n    }\n}",
      },
    },
    settings: {
      optimizer: {
        enabled: false,
        runs: 200,
      },
      outputSelection: {
        "*": {
          "*": [
            "abi",
            "evm.bytecode",
            "evm.deployedBytecode",
            "evm.methodIdentifiers",
            "metadata",
          ],
          "": ["ast"],
        },
      },
    },
  },
  output: {
    sources: {
      "contracts/Example.sol": {
        ast: {
          absolutePath: "contracts/Example.sol",
          exportedSymbols: {
            Example: [11],
          },
          id: 12,
          license: "Unlicense",
          nodeType: "SourceUnit",
          nodes: [
            {
              id: 1,
              literals: ["solidity", "^", "0.8", ".0"],
              nodeType: "PragmaDirective",
              src: "38:23:0",
            },
            {
              abstract: false,
              baseContracts: [],
              contractDependencies: [],
              contractKind: "contract",
              fullyImplemented: true,
              id: 11,
              linearizedBaseContracts: [11],
              name: "Example",
              nodeType: "ContractDefinition",
              nodes: [
                {
                  constant: false,
                  functionSelector: "0c55699c",
                  id: 3,
                  mutability: "mutable",
                  name: "x",
                  nodeType: "VariableDeclaration",
                  scope: 11,
                  src: "85:13:0",
                  stateVariable: true,
                  storageLocation: "default",
                  typeDescriptions: {
                    typeIdentifier: "t_uint256",
                    typeString: "uint256",
                  },
                  typeName: {
                    id: 2,
                    name: "uint",
                    nodeType: "ElementaryTypeName",
                    src: "85:4:0",
                    typeDescriptions: {
                      typeIdentifier: "t_uint256",
                      typeString: "uint256",
                    },
                  },
                  visibility: "public",
                },
                {
                  body: {
                    id: 9,
                    nodeType: "Block",
                    src: "127:20:0",
                    statements: [
                      {
                        expression: {
                          id: 7,
                          isConstant: false,
                          isLValue: false,
                          isPure: false,
                          lValueRequested: false,
                          nodeType: "UnaryOperation",
                          operator: "++",
                          prefix: false,
                          src: "137:3:0",
                          subExpression: {
                            id: 6,
                            name: "x",
                            nodeType: "Identifier",
                            overloadedDeclarations: [],
                            referencedDeclaration: 3,
                            src: "137:1:0",
                            typeDescriptions: {
                              typeIdentifier: "t_uint256",
                              typeString: "uint256",
                            },
                          },
                          typeDescriptions: {
                            typeIdentifier: "t_uint256",
                            typeString: "uint256",
                          },
                        },
                        id: 8,
                        nodeType: "ExpressionStatement",
                        src: "137:3:0",
                      },
                    ],
                  },
                  functionSelector: "371303c0",
                  id: 10,
                  implemented: true,
                  kind: "function",
                  modifiers: [],
                  name: "inc",
                  nodeType: "FunctionDefinition",
                  parameters: {
                    id: 4,
                    nodeType: "ParameterList",
                    parameters: [],
                    src: "117:2:0",
                  },
                  returnParameters: {
                    id: 5,
                    nodeType: "ParameterList",
                    parameters: [],
                    src: "127:0:0",
                  },
                  scope: 11,
                  src: "105:42:0",
                  stateMutability: "nonpayable",
                  virtual: false,
                  visibility: "public",
                },
              ],
              scope: 12,
              src: "62:87:0",
            },
          ],
          src: "38:111:0",
        },
        id: 0,
      },
    },
    contracts: {
      "contracts/Example.sol": {
        Example: {
          abi: [
            {
              inputs: [],
              name: "inc",
              outputs: [],
              stateMutability: "nonpayable",
              type: "function",
            },
            {
              inputs: [],
              name: "x",
              outputs: [
                {
                  internalType: "uint256",
                  name: "",
                  type: "uint256",
                },
              ],
              stateMutability: "view",
              type: "function",
            },
          ],
          evm: {
            bytecode: {
              generatedSources: [],
              linkReferences: {},
              object:
                "608060405234801561001057600080fd5b50610164806100206000396000f3fe608060405234801561001057600080fd5b50600436106100365760003560e01c80630c55699c1461003b578063371303c014610059575b600080fd5b610043610063565b6040516100509190610091565b60405180910390f35b610061610069565b005b60005481565b60008081548092919061007b906100b6565b9190505550565b61008b816100ac565b82525050565b60006020820190506100a66000830184610082565b92915050565b6000819050919050565b60006100c1826100ac565b91507fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff8214156100f4576100f36100ff565b5b600182019050919050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052601160045260246000fdfea2646970667358221220f059e1364306f26c1c4d0e1654e8eb8c7aa23a31ade199b12e96e2c80bc4b6ce64736f6c63430008000033",
              opcodes:
                "PUSH1 0x80 PUSH1 0x40 MSTORE CALLVALUE DUP1 ISZERO PUSH2 0x10 JUMPI PUSH1 0x0 DUP1 REVERT JUMPDEST POP PUSH2 0x164 DUP1 PUSH2 0x20 PUSH1 0x0 CODECOPY PUSH1 0x0 RETURN INVALID PUSH1 0x80 PUSH1 0x40 MSTORE CALLVALUE DUP1 ISZERO PUSH2 0x10 JUMPI PUSH1 0x0 DUP1 REVERT JUMPDEST POP PUSH1 0x4 CALLDATASIZE LT PUSH2 0x36 JUMPI PUSH1 0x0 CALLDATALOAD PUSH1 0xE0 SHR DUP1 PUSH4 0xC55699C EQ PUSH2 0x3B JUMPI DUP1 PUSH4 0x371303C0 EQ PUSH2 0x59 JUMPI JUMPDEST PUSH1 0x0 DUP1 REVERT JUMPDEST PUSH2 0x43 PUSH2 0x63 JUMP JUMPDEST PUSH1 0x40 MLOAD PUSH2 0x50 SWAP2 SWAP1 PUSH2 0x91 JUMP JUMPDEST PUSH1 0x40 MLOAD DUP1 SWAP2 SUB SWAP1 RETURN JUMPDEST PUSH2 0x61 PUSH2 0x69 JUMP JUMPDEST STOP JUMPDEST PUSH1 0x0 SLOAD DUP2 JUMP JUMPDEST PUSH1 0x0 DUP1 DUP2 SLOAD DUP1 SWAP3 SWAP2 SWAP1 PUSH2 0x7B SWAP1 PUSH2 0xB6 JUMP JUMPDEST SWAP2 SWAP1 POP SSTORE POP JUMP JUMPDEST PUSH2 0x8B DUP2 PUSH2 0xAC JUMP JUMPDEST DUP3 MSTORE POP POP JUMP JUMPDEST PUSH1 0x0 PUSH1 0x20 DUP3 ADD SWAP1 POP PUSH2 0xA6 PUSH1 0x0 DUP4 ADD DUP5 PUSH2 0x82 JUMP JUMPDEST SWAP3 SWAP2 POP POP JUMP JUMPDEST PUSH1 0x0 DUP2 SWAP1 POP SWAP2 SWAP1 POP JUMP JUMPDEST PUSH1 0x0 PUSH2 0xC1 DUP3 PUSH2 0xAC JUMP JUMPDEST SWAP2 POP PUSH32 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF DUP3 EQ ISZERO PUSH2 0xF4 JUMPI PUSH2 0xF3 PUSH2 0xFF JUMP JUMPDEST JUMPDEST PUSH1 0x1 DUP3 ADD SWAP1 POP SWAP2 SWAP1 POP JUMP JUMPDEST PUSH32 0x4E487B7100000000000000000000000000000000000000000000000000000000 PUSH1 0x0 MSTORE PUSH1 0x11 PUSH1 0x4 MSTORE PUSH1 0x24 PUSH1 0x0 REVERT INVALID LOG2 PUSH5 0x6970667358 0x22 SLT KECCAK256 CREATE MSIZE 0xE1 CALLDATASIZE NUMBER MOD CALLCODE PUSH13 0x1C4D0E1654E8EB8C7AA23A31AD 0xE1 SWAP10 0xB1 0x2E SWAP7 0xE2 0xC8 SIGNEXTEND 0xC4 0xB6 0xCE PUSH5 0x736F6C6343 STOP ADDMOD STOP STOP CALLER ",
              sourceMap: "62:87:0:-:0;;;;;;;;;;;;;;;;;;;",
            },
            deployedBytecode: {
              generatedSources: [
                {
                  ast: {
                    nodeType: "YulBlock",
                    src: "0:864:1",
                    statements: [
                      {
                        body: {
                          nodeType: "YulBlock",
                          src: "72:53:1",
                          statements: [
                            {
                              expression: {
                                arguments: [
                                  {
                                    name: "pos",
                                    nodeType: "YulIdentifier",
                                    src: "89:3:1",
                                  },
                                  {
                                    arguments: [
                                      {
                                        name: "value",
                                        nodeType: "YulIdentifier",
                                        src: "112:5:1",
                                      },
                                    ],
                                    functionName: {
                                      name: "cleanup_t_uint256",
                                      nodeType: "YulIdentifier",
                                      src: "94:17:1",
                                    },
                                    nodeType: "YulFunctionCall",
                                    src: "94:24:1",
                                  },
                                ],
                                functionName: {
                                  name: "mstore",
                                  nodeType: "YulIdentifier",
                                  src: "82:6:1",
                                },
                                nodeType: "YulFunctionCall",
                                src: "82:37:1",
                              },
                              nodeType: "YulExpressionStatement",
                              src: "82:37:1",
                            },
                          ],
                        },
                        name: "abi_encode_t_uint256_to_t_uint256_fromStack",
                        nodeType: "YulFunctionDefinition",
                        parameters: [
                          {
                            name: "value",
                            nodeType: "YulTypedName",
                            src: "60:5:1",
                            type: "",
                          },
                          {
                            name: "pos",
                            nodeType: "YulTypedName",
                            src: "67:3:1",
                            type: "",
                          },
                        ],
                        src: "7:118:1",
                      },
                      {
                        body: {
                          nodeType: "YulBlock",
                          src: "229:124:1",
                          statements: [
                            {
                              nodeType: "YulAssignment",
                              src: "239:26:1",
                              value: {
                                arguments: [
                                  {
                                    name: "headStart",
                                    nodeType: "YulIdentifier",
                                    src: "251:9:1",
                                  },
                                  {
                                    kind: "number",
                                    nodeType: "YulLiteral",
                                    src: "262:2:1",
                                    type: "",
                                    value: "32",
                                  },
                                ],
                                functionName: {
                                  name: "add",
                                  nodeType: "YulIdentifier",
                                  src: "247:3:1",
                                },
                                nodeType: "YulFunctionCall",
                                src: "247:18:1",
                              },
                              variableNames: [
                                {
                                  name: "tail",
                                  nodeType: "YulIdentifier",
                                  src: "239:4:1",
                                },
                              ],
                            },
                            {
                              expression: {
                                arguments: [
                                  {
                                    name: "value0",
                                    nodeType: "YulIdentifier",
                                    src: "319:6:1",
                                  },
                                  {
                                    arguments: [
                                      {
                                        name: "headStart",
                                        nodeType: "YulIdentifier",
                                        src: "332:9:1",
                                      },
                                      {
                                        kind: "number",
                                        nodeType: "YulLiteral",
                                        src: "343:1:1",
                                        type: "",
                                        value: "0",
                                      },
                                    ],
                                    functionName: {
                                      name: "add",
                                      nodeType: "YulIdentifier",
                                      src: "328:3:1",
                                    },
                                    nodeType: "YulFunctionCall",
                                    src: "328:17:1",
                                  },
                                ],
                                functionName: {
                                  name: "abi_encode_t_uint256_to_t_uint256_fromStack",
                                  nodeType: "YulIdentifier",
                                  src: "275:43:1",
                                },
                                nodeType: "YulFunctionCall",
                                src: "275:71:1",
                              },
                              nodeType: "YulExpressionStatement",
                              src: "275:71:1",
                            },
                          ],
                        },
                        name: "abi_encode_tuple_t_uint256__to_t_uint256__fromStack_reversed",
                        nodeType: "YulFunctionDefinition",
                        parameters: [
                          {
                            name: "headStart",
                            nodeType: "YulTypedName",
                            src: "201:9:1",
                            type: "",
                          },
                          {
                            name: "value0",
                            nodeType: "YulTypedName",
                            src: "213:6:1",
                            type: "",
                          },
                        ],
                        returnVariables: [
                          {
                            name: "tail",
                            nodeType: "YulTypedName",
                            src: "224:4:1",
                            type: "",
                          },
                        ],
                        src: "131:222:1",
                      },
                      {
                        body: {
                          nodeType: "YulBlock",
                          src: "404:32:1",
                          statements: [
                            {
                              nodeType: "YulAssignment",
                              src: "414:16:1",
                              value: {
                                name: "value",
                                nodeType: "YulIdentifier",
                                src: "425:5:1",
                              },
                              variableNames: [
                                {
                                  name: "cleaned",
                                  nodeType: "YulIdentifier",
                                  src: "414:7:1",
                                },
                              ],
                            },
                          ],
                        },
                        name: "cleanup_t_uint256",
                        nodeType: "YulFunctionDefinition",
                        parameters: [
                          {
                            name: "value",
                            nodeType: "YulTypedName",
                            src: "386:5:1",
                            type: "",
                          },
                        ],
                        returnVariables: [
                          {
                            name: "cleaned",
                            nodeType: "YulTypedName",
                            src: "396:7:1",
                            type: "",
                          },
                        ],
                        src: "359:77:1",
                      },
                      {
                        body: {
                          nodeType: "YulBlock",
                          src: "485:190:1",
                          statements: [
                            {
                              nodeType: "YulAssignment",
                              src: "495:33:1",
                              value: {
                                arguments: [
                                  {
                                    name: "value",
                                    nodeType: "YulIdentifier",
                                    src: "522:5:1",
                                  },
                                ],
                                functionName: {
                                  name: "cleanup_t_uint256",
                                  nodeType: "YulIdentifier",
                                  src: "504:17:1",
                                },
                                nodeType: "YulFunctionCall",
                                src: "504:24:1",
                              },
                              variableNames: [
                                {
                                  name: "value",
                                  nodeType: "YulIdentifier",
                                  src: "495:5:1",
                                },
                              ],
                            },
                            {
                              body: {
                                nodeType: "YulBlock",
                                src: "618:22:1",
                                statements: [
                                  {
                                    expression: {
                                      arguments: [],
                                      functionName: {
                                        name: "panic_error_0x11",
                                        nodeType: "YulIdentifier",
                                        src: "620:16:1",
                                      },
                                      nodeType: "YulFunctionCall",
                                      src: "620:18:1",
                                    },
                                    nodeType: "YulExpressionStatement",
                                    src: "620:18:1",
                                  },
                                ],
                              },
                              condition: {
                                arguments: [
                                  {
                                    name: "value",
                                    nodeType: "YulIdentifier",
                                    src: "543:5:1",
                                  },
                                  {
                                    kind: "number",
                                    nodeType: "YulLiteral",
                                    src: "550:66:1",
                                    type: "",
                                    value:
                                      "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
                                  },
                                ],
                                functionName: {
                                  name: "eq",
                                  nodeType: "YulIdentifier",
                                  src: "540:2:1",
                                },
                                nodeType: "YulFunctionCall",
                                src: "540:77:1",
                              },
                              nodeType: "YulIf",
                              src: "537:2:1",
                            },
                            {
                              nodeType: "YulAssignment",
                              src: "649:20:1",
                              value: {
                                arguments: [
                                  {
                                    name: "value",
                                    nodeType: "YulIdentifier",
                                    src: "660:5:1",
                                  },
                                  {
                                    kind: "number",
                                    nodeType: "YulLiteral",
                                    src: "667:1:1",
                                    type: "",
                                    value: "1",
                                  },
                                ],
                                functionName: {
                                  name: "add",
                                  nodeType: "YulIdentifier",
                                  src: "656:3:1",
                                },
                                nodeType: "YulFunctionCall",
                                src: "656:13:1",
                              },
                              variableNames: [
                                {
                                  name: "ret",
                                  nodeType: "YulIdentifier",
                                  src: "649:3:1",
                                },
                              ],
                            },
                          ],
                        },
                        name: "increment_t_uint256",
                        nodeType: "YulFunctionDefinition",
                        parameters: [
                          {
                            name: "value",
                            nodeType: "YulTypedName",
                            src: "471:5:1",
                            type: "",
                          },
                        ],
                        returnVariables: [
                          {
                            name: "ret",
                            nodeType: "YulTypedName",
                            src: "481:3:1",
                            type: "",
                          },
                        ],
                        src: "442:233:1",
                      },
                      {
                        body: {
                          nodeType: "YulBlock",
                          src: "709:152:1",
                          statements: [
                            {
                              expression: {
                                arguments: [
                                  {
                                    kind: "number",
                                    nodeType: "YulLiteral",
                                    src: "726:1:1",
                                    type: "",
                                    value: "0",
                                  },
                                  {
                                    kind: "number",
                                    nodeType: "YulLiteral",
                                    src: "729:77:1",
                                    type: "",
                                    value:
                                      "35408467139433450592217433187231851964531694900788300625387963629091585785856",
                                  },
                                ],
                                functionName: {
                                  name: "mstore",
                                  nodeType: "YulIdentifier",
                                  src: "719:6:1",
                                },
                                nodeType: "YulFunctionCall",
                                src: "719:88:1",
                              },
                              nodeType: "YulExpressionStatement",
                              src: "719:88:1",
                            },
                            {
                              expression: {
                                arguments: [
                                  {
                                    kind: "number",
                                    nodeType: "YulLiteral",
                                    src: "823:1:1",
                                    type: "",
                                    value: "4",
                                  },
                                  {
                                    kind: "number",
                                    nodeType: "YulLiteral",
                                    src: "826:4:1",
                                    type: "",
                                    value: "0x11",
                                  },
                                ],
                                functionName: {
                                  name: "mstore",
                                  nodeType: "YulIdentifier",
                                  src: "816:6:1",
                                },
                                nodeType: "YulFunctionCall",
                                src: "816:15:1",
                              },
                              nodeType: "YulExpressionStatement",
                              src: "816:15:1",
                            },
                            {
                              expression: {
                                arguments: [
                                  {
                                    kind: "number",
                                    nodeType: "YulLiteral",
                                    src: "847:1:1",
                                    type: "",
                                    value: "0",
                                  },
                                  {
                                    kind: "number",
                                    nodeType: "YulLiteral",
                                    src: "850:4:1",
                                    type: "",
                                    value: "0x24",
                                  },
                                ],
                                functionName: {
                                  name: "revert",
                                  nodeType: "YulIdentifier",
                                  src: "840:6:1",
                                },
                                nodeType: "YulFunctionCall",
                                src: "840:15:1",
                              },
                              nodeType: "YulExpressionStatement",
                              src: "840:15:1",
                            },
                          ],
                        },
                        name: "panic_error_0x11",
                        nodeType: "YulFunctionDefinition",
                        src: "681:180:1",
                      },
                    ],
                  },
                  contents:
                    "{\n\n    function abi_encode_t_uint256_to_t_uint256_fromStack(value, pos) {\n        mstore(pos, cleanup_t_uint256(value))\n    }\n\n    function abi_encode_tuple_t_uint256__to_t_uint256__fromStack_reversed(headStart , value0) -> tail {\n        tail := add(headStart, 32)\n\n        abi_encode_t_uint256_to_t_uint256_fromStack(value0,  add(headStart, 0))\n\n    }\n\n    function cleanup_t_uint256(value) -> cleaned {\n        cleaned := value\n    }\n\n    function increment_t_uint256(value) -> ret {\n        value := cleanup_t_uint256(value)\n        if eq(value, 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff) { panic_error_0x11() }\n        ret := add(value, 1)\n    }\n\n    function panic_error_0x11() {\n        mstore(0, 35408467139433450592217433187231851964531694900788300625387963629091585785856)\n        mstore(4, 0x11)\n        revert(0, 0x24)\n    }\n\n}\n",
                  id: 1,
                  language: "Yul",
                  name: "#utility.yul",
                },
              ],
              immutableReferences: {},
              linkReferences: {},
              object:
                "608060405234801561001057600080fd5b50600436106100365760003560e01c80630c55699c1461003b578063371303c014610059575b600080fd5b610043610063565b6040516100509190610091565b60405180910390f35b610061610069565b005b60005481565b60008081548092919061007b906100b6565b9190505550565b61008b816100ac565b82525050565b60006020820190506100a66000830184610082565b92915050565b6000819050919050565b60006100c1826100ac565b91507fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff8214156100f4576100f36100ff565b5b600182019050919050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052601160045260246000fdfea2646970667358221220f059e1364306f26c1c4d0e1654e8eb8c7aa23a31ade199b12e96e2c80bc4b6ce64736f6c63430008000033",
              opcodes:
                "PUSH1 0x80 PUSH1 0x40 MSTORE CALLVALUE DUP1 ISZERO PUSH2 0x10 JUMPI PUSH1 0x0 DUP1 REVERT JUMPDEST POP PUSH1 0x4 CALLDATASIZE LT PUSH2 0x36 JUMPI PUSH1 0x0 CALLDATALOAD PUSH1 0xE0 SHR DUP1 PUSH4 0xC55699C EQ PUSH2 0x3B JUMPI DUP1 PUSH4 0x371303C0 EQ PUSH2 0x59 JUMPI JUMPDEST PUSH1 0x0 DUP1 REVERT JUMPDEST PUSH2 0x43 PUSH2 0x63 JUMP JUMPDEST PUSH1 0x40 MLOAD PUSH2 0x50 SWAP2 SWAP1 PUSH2 0x91 JUMP JUMPDEST PUSH1 0x40 MLOAD DUP1 SWAP2 SUB SWAP1 RETURN JUMPDEST PUSH2 0x61 PUSH2 0x69 JUMP JUMPDEST STOP JUMPDEST PUSH1 0x0 SLOAD DUP2 JUMP JUMPDEST PUSH1 0x0 DUP1 DUP2 SLOAD DUP1 SWAP3 SWAP2 SWAP1 PUSH2 0x7B SWAP1 PUSH2 0xB6 JUMP JUMPDEST SWAP2 SWAP1 POP SSTORE POP JUMP JUMPDEST PUSH2 0x8B DUP2 PUSH2 0xAC JUMP JUMPDEST DUP3 MSTORE POP POP JUMP JUMPDEST PUSH1 0x0 PUSH1 0x20 DUP3 ADD SWAP1 POP PUSH2 0xA6 PUSH1 0x0 DUP4 ADD DUP5 PUSH2 0x82 JUMP JUMPDEST SWAP3 SWAP2 POP POP JUMP JUMPDEST PUSH1 0x0 DUP2 SWAP1 POP SWAP2 SWAP1 POP JUMP JUMPDEST PUSH1 0x0 PUSH2 0xC1 DUP3 PUSH2 0xAC JUMP JUMPDEST SWAP2 POP PUSH32 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF DUP3 EQ ISZERO PUSH2 0xF4 JUMPI PUSH2 0xF3 PUSH2 0xFF JUMP JUMPDEST JUMPDEST PUSH1 0x1 DUP3 ADD SWAP1 POP SWAP2 SWAP1 POP JUMP JUMPDEST PUSH32 0x4E487B7100000000000000000000000000000000000000000000000000000000 PUSH1 0x0 MSTORE PUSH1 0x11 PUSH1 0x4 MSTORE PUSH1 0x24 PUSH1 0x0 REVERT INVALID LOG2 PUSH5 0x6970667358 0x22 SLT KECCAK256 CREATE MSIZE 0xE1 CALLDATASIZE NUMBER MOD CALLCODE PUSH13 0x1C4D0E1654E8EB8C7AA23A31AD 0xE1 SWAP10 0xB1 0x2E SWAP7 0xE2 0xC8 SIGNEXTEND 0xC4 0xB6 0xCE PUSH5 0x736F6C6343 STOP ADDMOD STOP STOP CALLER ",
              sourceMap:
                "62:87:0:-:0;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;85:13;;;:::i;:::-;;;;;;;:::i;:::-;;;;;;;;105:42;;;:::i;:::-;;85:13;;;;:::o;105:42::-;137:1;;:3;;;;;;;;;:::i;:::-;;;;;;105:42::o;7:118:1:-;94:24;112:5;94:24;:::i;:::-;89:3;82:37;72:53;;:::o;131:222::-;;262:2;251:9;247:18;239:26;;275:71;343:1;332:9;328:17;319:6;275:71;:::i;:::-;229:124;;;;:::o;359:77::-;;425:5;414:16;;404:32;;;:::o;442:233::-;;504:24;522:5;504:24;:::i;:::-;495:33;;550:66;543:5;540:77;537:2;;;620:18;;:::i;:::-;537:2;667:1;660:5;656:13;649:20;;485:190;;;:::o;681:180::-;729:77;726:1;719:88;826:4;823:1;816:15;850:4;847:1;840:15",
            },
            methodIdentifiers: {
              "inc()": "371303c0",
              "x()": "0c55699c",
            },
          },
          metadata:
            '{"compiler":{"version":"0.8.0+commit.c7dfd78e"},"language":"Solidity","output":{"abi":[{"inputs":[],"name":"inc","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[],"name":"x","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"}],"devdoc":{"kind":"dev","methods":{},"version":1},"userdoc":{"kind":"user","methods":{},"version":1}},"settings":{"compilationTarget":{"contracts/Example.sol":"Example"},"evmVersion":"istanbul","libraries":{},"metadata":{"bytecodeHash":"ipfs"},"optimizer":{"enabled":false,"runs":200},"remappings":[]},"sources":{"contracts/Example.sol":{"keccak256":"0x59b0856e75cd15dc2e53235ca43c23e33d46bd284bd87e25fae9c40a1b1f1f6e","license":"Unlicense","urls":["bzz-raw://7e94cba4b8a3d60b45c89e71ef6b594b9c1974ae1ab81d308fac7b6a4bde5d66","dweb:/ipfs/QmW41rG5XT6ADihjEswvg9V34LxJE6KonCK9iyD6CkVu7V"]}},"version":1}',
        },
      },
    },
  },
};
