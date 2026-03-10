# Selector Index Optimization Plan

## Context

This optimization targets the `search_selector_in_all_contracts()` method in [`crates/edr_solidity/src/contracts_identifier.rs`](../crates/edr_solidity/src/contracts_identifier.rs).

### Current Implementation Problem

The current implementation at lines 236-263 performs a **linear search** through all known contracts to find a function matching a given 4-byte selector:

```rust
pub fn search_selector_in_all_contracts(&self, selector: &[u8]) -> SelectorSearchResult {
    let mut signatures = std::collections::HashSet::new();

    // O(n) iteration through ALL contracts
    for contract_metadata in &self.trie.descendants {
        let contract = contract_metadata.contract.read();
        
        if let Some(function) = contract.get_function_from_selector(selector) {
            // Build signature...
        }
    }

    SelectorSearchResult { signatures }
}
```

**Performance:** O(n × f) where n = number of contracts, f = average functions per contract

### Why This Matters

This method is called from [`contract_decoder.rs`](../crates/edr_solidity/src/contract_decoder.rs) as a fallback when a function selector is not found in the called contract's ABI (common for proxy contracts). For large projects with many contracts, this linear search is called repeatedly during trace processing.

---

## Solution: Pre-built Selector Index

Build a `HashMap<[u8; 4], HashSet<String>>` index when contracts are added, enabling O(1) lookup.

---

## Files to Modify

1. **`crates/edr_solidity/src/contracts_identifier.rs`** - Add the index and update methods

---

## Implementation Steps

### Step 1: Add Selector Index Field

**File:** `crates/edr_solidity/src/contracts_identifier.rs`  
**Location:** `ContractsIdentifier` struct definition (around line 47)

```rust
/// A data structure that allows searching for well-known bytecodes.
#[derive(Debug)]
pub struct ContractsIdentifier {
    trie: BytecodeTrie<Arc<ContractMetadata>>,
    cache: HashMap<Vec<u8>, Arc<ContractMetadata>>,
    enable_cache: bool,
    /// Index mapping function selectors to their signatures.
    /// Used for O(1) fallback lookup when selector isn't found in called contract.
    selector_index: HashMap<[u8; 4], HashSet<String>>,
}
```

**Imports needed:** Ensure `HashSet` is imported from `std::collections`.

---

### Step 2: Update Constructor

**File:** `crates/edr_solidity/src/contracts_identifier.rs`  
**Location:** `ContractsIdentifier::new()` method (around line 61)

```rust
pub fn new(enable_cache: Option<bool>) -> ContractsIdentifier {
    let enable_cache = enable_cache.unwrap_or(true);

    ContractsIdentifier {
        trie: BytecodeTrie::new_root(),
        cache: HashMap::new(),
        enable_cache,
        selector_index: HashMap::new(),
    }
}
```

---

### Step 3: Update `add_bytecode()` to Build Index

**File:** `crates/edr_solidity/src/contracts_identifier.rs`  
**Location:** `ContractsIdentifier::add_bytecode()` method (around line 72)

**Current implementation:**
```rust
pub fn add_bytecode(&mut self, bytecode: Arc<ContractMetadata>) {
    self.trie.add(bytecode);
    self.cache.clear();
}
```

**New implementation:**
```rust
pub fn add_bytecode(&mut self, bytecode: Arc<ContractMetadata>) {
    self.trie.add(bytecode.clone());
    self.cache.clear();
    
    // Index all function selectors from this contract
    self.index_contract_functions(&bytecode);
}

/// Indexes all function selectors from a contract for O(1) lookup.
fn index_contract_functions(&mut self, bytecode: &Arc<ContractMetadata>) {
    let contract = bytecode.contract.read();
    
    for function in contract.functions.values() {
        // Skip fallback/receive functions (they don't have meaningful selectors)
        if matches!(
            function.r#type,
            crate::build_model::ContractFunctionType::Fallback
                | crate::build_model::ContractFunctionType::Receive
        ) {
            continue;
        }
        
        if let Ok(abi_function) = alloy_json_abi::Function::try_from(&**function) {
            // Get the 4-byte selector
            let selector = abi_function.selector();
            
            // Build the signature string: "functionName(type1,type2,...)"
            let inputs = abi_function
                .inputs
                .iter()
                .map(|param| param.ty.clone())
                .collect::<Vec<_>>()
                .join(",");
            let signature = format!("{}({})", function.name, inputs);
            
            // Add to index (handles selector collisions via HashSet)
            self.selector_index
                .entry(selector)
                .or_default()
                .insert(signature);
        }
    }
}
```

**Note:** The `contract.functions` field is a `HashMap<Selector, Arc<ContractFunction>>` as defined in [`build_model.rs`](../crates/edr_solidity/src/build_model.rs).

---

### Step 4: Simplify `search_selector_in_all_contracts()`

**File:** `crates/edr_solidity/src/contracts_identifier.rs`  
**Location:** `search_selector_in_all_contracts()` method (around line 236)

**New implementation:**
```rust
/// Searches all known contracts for a function matching the given selector.
///
/// This is used as a fallback when the selector is not found in the called
/// contract's ABI, e.g., for proxy contracts.
///
/// Returns a [`SelectorSearchResult`] containing all matching function
/// signatures. Usually there's only one match, but selector collisions
/// are theoretically possible.
pub fn search_selector_in_all_contracts(&self, selector: &[u8]) -> SelectorSearchResult {
    // Convert selector slice to fixed 4-byte array
    let selector_array: [u8; 4] = match selector.try_into() {
        Ok(arr) => arr,
        Err(_) => {
            // Selector must be exactly 4 bytes
            return SelectorSearchResult {
                signatures: HashSet::new(),
            };
        }
    };
    
    // O(1) lookup in the pre-built index
    let signatures = self
        .selector_index
        .get(&selector_array)
        .cloned()
        .unwrap_or_default();
    
    SelectorSearchResult { signatures }
}
```

---

## Testing

Add a new test to the existing test module in `contracts_identifier.rs`:

```rust
#[test]
fn test_selector_index_lookup() {
    use crate::build_model::{ContractFunction, ContractFunctionType, ContractFunctionVisibility};
    
    let mut contracts_identifier = ContractsIdentifier::default();
    
    // Create a contract with a function that has selector 0x12345678
    let sources = create_sources();
    let contract = create_test_contract();
    
    // Add a function to the contract
    {
        let mut contract_write = contract.write();
        let function = Arc::new(ContractFunction {
            name: "transfer".to_string(),
            r#type: ContractFunctionType::Function,
            visibility: ContractFunctionVisibility::Public,
            location: contract_write.location.clone(),
            param_types: vec!["address".to_string(), "uint256".to_string()],
            // ... other fields as needed
        });
        // The selector for transfer(address,uint256) is 0xa9059cbb
        contract_write.functions.insert(
            [0xa9, 0x05, 0x9c, 0xbb],
            function,
        );
    }
    
    let bytecode = Arc::new(ContractMetadata::new(
        sources,
        contract,
        false, // is_deployment
        vec![1, 2, 3, 4, 5],
        vec![],
        vec![],
        vec![],
        "<dummy-version>".to_string(),
    ));
    
    contracts_identifier.add_bytecode(bytecode);
    
    // Now search for the selector
    let result = contracts_identifier.search_selector_in_all_contracts(&[0xa9, 0x05, 0x9c, 0xbb]);
    
    assert!(!result.signatures.is_empty());
    assert!(result.signatures.contains("transfer(address,uint256)"));
}
```

---

## Performance Impact

| Operation | Before | After |
|-----------|--------|-------|
| `add_bytecode()` | O(1) | O(f) where f = functions in contract |
| `search_selector_in_all_contracts()` | O(n × f) | O(1) |

Where:
- n = total number of contracts
- f = average number of functions per contract

The trade-off is acceptable because:
1. `add_bytecode()` is called once per contract during setup
2. `search_selector_in_all_contracts()` is called potentially many times during trace processing

---

## Memory Impact

Additional memory usage: approximately 4 bytes (selector) + ~30-50 bytes (signature string) per function.

For a project with 100 contracts averaging 20 functions each:
- 100 × 20 × 54 bytes ≈ 108 KB additional memory

This is negligible compared to the bytecode data already stored.

---

## Edge Cases to Handle

1. **Empty selector (< 4 bytes):** Return empty result (handled by `try_into()` error case)
2. **Selector collision:** Multiple functions with same 4-byte selector stored in HashSet
3. **Fallback/Receive functions:** Skip these as they don't have meaningful selectors

---

## Related Files for Reference

- [`crates/edr_solidity/src/build_model.rs`](../crates/edr_solidity/src/build_model.rs) - Contains `Contract` and `ContractFunction` definitions
- [`crates/edr_solidity/src/contract_decoder.rs`](../crates/edr_solidity/src/contract_decoder.rs) - Calls `search_selector_in_all_contracts()` at lines 243-249 and 341-352
- [`crates/edr_solidity/src/proxy_function_resolver.rs`](../crates/edr_solidity/src/proxy_function_resolver.rs) - Contains `SelectorSearchResult` type
