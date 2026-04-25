//! Runtime tests: things that SHOULD compile and produce the documented
//! behavior. Compile-fail cases live under `tests/ui/` and are driven by
//! the `ui` test below via `trybuild`.

use edr_jsonrpc_error_macro::rpc_error;
use edr_jsonrpc_error_structured::RpcStructuredErrorTag;

#[rpc_error(tag = "not-found")]
pub struct NotFound {
    resource: String,
    id: u64,
}

#[test]
fn basic_serialization_and_tag() {
    let err = NotFound {
        resource: "user".into(),
        id: 42,
    };
    assert_eq!(NotFound::ERROR_TAG, "not-found");
    assert_eq!(
        serde_json::to_string(&err).unwrap(),
        r#"{"resource":"user","id":42}"#,
    );
}

#[rpc_error] // No tag is generated
pub struct PermissionDenied {
    user_id: u64,
    reason: String,
}

#[test]
fn basic_serialization_no_tag() {
    let err = PermissionDenied {
        user_id: 7,
        reason: "not admin".into(),
    };
    assert_eq!(
        serde_json::to_string(&err).unwrap(),
        r#"{"user_id":7,"reason":"not admin"}"#,
    );
}

#[rpc_error] // empty struct is allowed — empty object
pub struct ServerBusy {}

#[test]
fn empty_struct_serializes_to_empty_object() {
    let err = ServerBusy {};
    assert_eq!(serde_json::to_string(&err).unwrap(), "{}");
}

#[rpc_error(tag = "validation")]
#[serde(rename_all = "camelCase")]
pub struct Validation {
    field_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    hint: Option<String>,
    #[serde(rename = "errorCode")]
    code: u32,
}

#[test]
fn whitelisted_serde_attrs_flow_through() {
    let err = Validation {
        field_name: "email".into(),
        hint: None,
        code: 400,
    };
    // rename_all=camelCase applies to field_name (→fieldName);
    // field-level rename overrides it on `code` (→errorCode);
    // skip_serializing_if omits `hint` when None.
    assert_eq!(
        serde_json::to_string(&err).unwrap(),
        r#"{"fieldName":"email","errorCode":400}"#,
    );
}

#[rpc_error]
/// Doc comments and unrelated attributes must flow through untouched.
#[deprecated(note = "test deprecation")]
pub struct DocCommented {
    /// Field doc.
    x: String,
}

#[test]
#[allow(deprecated)]
fn doc_comments_and_unrelated_attrs_flow_through() {
    let err = DocCommented { x: "hi".into() };
    assert_eq!(serde_json::to_string(&err).unwrap(), r#"{"x":"hi"}"#);
}

#[rpc_error(tag = "cfg-good")]
#[cfg_attr(all(), serde(rename_all = "camelCase"))] // whitelisted inside cfg_attr
pub struct CfgAttrGood {
    user_id: u32,
}

#[test]
fn whitelisted_serde_attrs_inside_cfg_attr_apply() {
    let err = CfgAttrGood { user_id: 7 };
    assert_eq!(serde_json::to_string(&err).unwrap(), r#"{"userId":7}"#);
}

#[rpc_error(tag = "cfg-benign")]
#[cfg_attr(all(), allow(dead_code))] // non-serde inside cfg_attr — fine
pub struct CfgAttrBenign {
    x: String,
}

#[test]
fn benign_cfg_attr_does_not_interfere() {
    let err = CfgAttrBenign { x: "hi".into() };
    assert_eq!(serde_json::to_string(&err).unwrap(), r#"{"x":"hi"}"#);
    assert_eq!(CfgAttrBenign::ERROR_TAG, "cfg-benign");
}

#[test]
fn embeds_inside_parent_via_serialize() {
    #[derive(serde::Serialize)]
    struct Parent<'a> {
        req_id: u64,
        error: &'a NotFound,
    }
    let err = NotFound {
        resource: "user".into(),
        id: 42,
    };
    let parent = Parent {
        req_id: 1,
        error: &err,
    };
    assert_eq!(
        serde_json::to_string(&parent).unwrap(),
        r#"{"req_id":1,"error":{"resource":"user","id":42}}"#,
    );
}

#[test]
fn tag_trait_is_associated_const_not_method() {
    // The tag is available at compile time — this must typecheck.
    const _T: &str = NotFound::ERROR_TAG;
    assert_eq!(_T, "not-found");
}

/// Compile-fail test suite. Each .rs file under tests/ui/ represents a
/// shape or attribute that must be rejected at compile time. The matching
/// .stderr file captures the expected error output.
#[test]
fn compile_fail_cases() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
