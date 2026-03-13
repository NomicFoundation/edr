//! Proxy chain detection from call traces.
//!
//! Detects proxy delegation patterns by identifying DELEGATECALL calls that
//! forward the same calldata and return the same returndata as the caller.

use edr_defaults::SELECTOR_LEN;
use edr_primitives::Selector;
use foundry_evm_traces::CallTrace;
use revm_inspectors::tracing::{types::CallKind, CallTraceArena};

/// Detects a proxy delegation chain starting from a given node in the call
/// trace arena.
///
/// Returns a `Vec<Address>` representing the chain from the final
/// implementation to the outermost proxy, e.g. `[implementation, proxyN, ...,
/// proxy1]`.
///
/// This order is chosen to minimise reallocation when building the chain
/// recursively.
///
/// Returns an empty Vec if the node does not exhibit a proxy pattern.
///
/// A node is considered a proxy if it performs a DELEGATECALL to a child with
/// the same calldata and the child returns the same returndata.
pub fn detect_proxy_chain(arena: &CallTraceArena, node_idx: usize) -> Option<Vec<&CallTrace>> {
    let nodes = arena.nodes();
    let node = nodes.get(node_idx)?;

    // Only CALL nodes can be proxies (not creates)
    if node.trace.kind.is_any_create() {
        return None;
    }

    for &child_idx in &node.children {
        let Some(child) = nodes.get(child_idx) else {
            continue;
        };

        if child.trace.kind != CallKind::DelegateCall {
            continue;
        }

        if !is_proxy_selector_and_output(&node.trace, &child.trace) {
            continue;
        }

        // Found a proxy delegation. Recurse to detect chained proxies.
        let inner_chain = detect_proxy_chain(arena, child_idx);
        let chain = if let Some(mut inner_chain) = inner_chain {
            inner_chain.push(&node.trace);
            inner_chain
        } else {
            // End of chain: child is the implementation
            vec![&child.trace, &node.trace]
        };

        return Some(chain);
    }

    None
}

/// Checks whether the child trace's function selector and returndata matches
/// the parent trace, indicating a potential proxy delegation.
pub fn is_proxy_selector_and_output(parent_trace: &CallTrace, child_trace: &CallTrace) -> bool {
    if child_trace.output != parent_trace.output {
        return false;
    }

    let Some(Ok(parent_selector)) = parent_trace
        .data
        .get(..SELECTOR_LEN)
        .map(Selector::try_from)
    else {
        return false;
    };

    let Some(Ok(child_selector)) = child_trace.data.get(..SELECTOR_LEN).map(Selector::try_from)
    else {
        return false;
    };

    parent_selector == child_selector
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
        assert!(chain.is_none());
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
        let proxy_call = &arena.nodes()[0].trace;
        let impl_call = &arena.nodes()[1].trace;

        let chain = detect_proxy_chain(&arena, 0);
        assert_eq!(chain, Some(vec![impl_call, proxy_call]));
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
        let proxy1_call = &arena.nodes()[0].trace;
        let proxy2_call = &arena.nodes()[1].trace;
        let impl_call = &arena.nodes()[2].trace;

        let chain = detect_proxy_chain(&arena, 0);
        assert_eq!(chain, Some(vec![impl_call, proxy2_call, proxy1_call]));
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
        assert!(chain.is_none());
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
        assert!(chain.is_none());
    }
}
