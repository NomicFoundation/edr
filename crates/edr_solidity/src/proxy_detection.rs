//! Proxy chain detection from call traces.
//!
//! Detects proxy delegation patterns by identifying DELEGATECALL calls that
//! forward the same calldata and return the same returndata as the caller.

use edr_primitives::Address;
use revm_inspectors::tracing::{types::CallKind, CallTraceArena};

/// Detects a proxy delegation chain starting from a given node in the call
/// trace arena.
///
/// Returns a `Vec<Address>` representing the chain from the outermost proxy to
/// the final implementation, e.g. `[proxy1, proxy2, ...,  implementation]`.
///
/// Returns an empty Vec if the node does not exhibit a proxy pattern.
///
/// A node is considered a proxy if it performs a DELEGATECALL to a child with
/// the same calldata and the child returns the same returndata.
pub fn detect_proxy_chain(arena: &CallTraceArena, node_idx: usize) -> Vec<Address> {
    let nodes = arena.nodes();
    let Some(node) = nodes.get(node_idx) else {
        return Vec::new();
    };

    // Only CALL nodes can be proxies (not creates)
    if node.trace.kind.is_any_create() {
        return Vec::new();
    }

    for &child_idx in &node.children {
        let Some(child) = nodes.get(child_idx) else {
            continue;
        };

        if child.trace.kind != CallKind::DelegateCall {
            continue;
        }

        // Proxy pattern: same calldata forwarded and same returndata returned
        if child.trace.data != node.trace.data {
            continue;
        }
        if child.trace.output != node.trace.output {
            continue;
        }

        // Found a proxy delegation. Recurse to detect chained proxies.
        let inner_chain = detect_proxy_chain(arena, child_idx);
        let mut chain = vec![node.trace.address];
        if inner_chain.is_empty() {
            // End of chain: child is the implementation
            chain.push(child.trace.address);
        } else {
            chain.extend(inner_chain);
        }
        return chain;
    }

    Vec::new()
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{Address, Bytes};
    use revm_inspectors::tracing::types::CallKind;

    use super::*;

    fn build_arena(configs: Vec<(CallKind, Address, Bytes, Bytes, Vec<usize>)>) -> CallTraceArena {
        let mut arena = CallTraceArena::default();
        let nodes = arena.nodes_mut();

        // The default arena has one root node at index 0. Replace it and add
        // more.
        for (i, (kind, address, data, output, children)) in configs.into_iter().enumerate() {
            if i == 0 {
                nodes[0].trace.kind = kind;
                nodes[0].trace.address = address;
                nodes[0].trace.data = data;
                nodes[0].trace.output = output;
                nodes[0].children = children;
            } else {
                nodes.push(revm_inspectors::tracing::types::CallTraceNode {
                    idx: i,
                    trace: revm_inspectors::tracing::types::CallTrace {
                        kind,
                        address,
                        data,
                        output,
                        ..Default::default()
                    },
                    children,
                    ..Default::default()
                });
            }
        }

        arena
    }

    #[test]
    fn test_no_proxy_no_children() {
        let arena = build_arena(vec![(
            CallKind::Call,
            Address::repeat_byte(1),
            Bytes::from_static(b"calldata"),
            Bytes::from_static(b"output"),
            vec![],
        )]);
        let chain = detect_proxy_chain(&arena, 0);
        assert!(chain.is_empty());
    }

    #[test]
    fn test_simple_proxy() {
        let proxy_addr = Address::repeat_byte(1);
        let impl_addr = Address::repeat_byte(2);
        let calldata = Bytes::from_static(b"calldata");
        let output = Bytes::from_static(b"output");

        let arena = build_arena(vec![
            (
                CallKind::Call,
                proxy_addr,
                calldata.clone(),
                output.clone(),
                vec![1],
            ),
            (CallKind::DelegateCall, impl_addr, calldata, output, vec![]),
        ]);
        let chain = detect_proxy_chain(&arena, 0);
        assert_eq!(chain, vec![proxy_addr, impl_addr]);
    }

    #[test]
    fn test_chained_proxies() {
        let proxy1 = Address::repeat_byte(1);
        let proxy2 = Address::repeat_byte(2);
        let impl_addr = Address::repeat_byte(3);
        let calldata = Bytes::from_static(b"calldata");
        let output = Bytes::from_static(b"output");

        let arena = build_arena(vec![
            (
                CallKind::Call,
                proxy1,
                calldata.clone(),
                output.clone(),
                vec![1],
            ),
            (
                CallKind::DelegateCall,
                proxy2,
                calldata.clone(),
                output.clone(),
                vec![2],
            ),
            (CallKind::DelegateCall, impl_addr, calldata, output, vec![]),
        ]);
        let chain = detect_proxy_chain(&arena, 0);
        assert_eq!(chain, vec![proxy1, proxy2, impl_addr]);
    }

    #[test]
    fn test_different_returndata_not_proxy() {
        let arena = build_arena(vec![
            (
                CallKind::Call,
                Address::repeat_byte(1),
                Bytes::from_static(b"calldata"),
                Bytes::from_static(b"output_a"),
                vec![1],
            ),
            (
                CallKind::DelegateCall,
                Address::repeat_byte(2),
                Bytes::from_static(b"calldata"),
                Bytes::from_static(b"output_b"),
                vec![],
            ),
        ]);
        let chain = detect_proxy_chain(&arena, 0);
        assert!(chain.is_empty());
    }

    #[test]
    fn test_regular_call_not_proxy() {
        let calldata = Bytes::from_static(b"calldata");
        let output = Bytes::from_static(b"output");

        let arena = build_arena(vec![
            (
                CallKind::Call,
                Address::repeat_byte(1),
                calldata.clone(),
                output.clone(),
                vec![1],
            ),
            // Regular CALL, not DelegateCall
            (
                CallKind::Call,
                Address::repeat_byte(2),
                calldata,
                output,
                vec![],
            ),
        ]);
        let chain = detect_proxy_chain(&arena, 0);
        assert!(chain.is_empty());
    }
}
