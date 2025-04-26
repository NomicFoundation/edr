use edr_instrument::coverage::{self, Version};
use napi::bindgen_prelude::Buffer;
use napi_derive::napi;

#[napi(object)]
pub struct InstrumentationResult {
    #[napi(readonly)]
    pub source: String,
    #[napi(readonly)]
    pub metadata: Vec<InstrumentationMetadata>,
}

impl TryFrom<edr_instrument::coverage::InstrumentationResult> for InstrumentationResult {
    type Error = usize;

    fn try_from(
        value: edr_instrument::coverage::InstrumentationResult,
    ) -> Result<Self, Self::Error> {
        let metadata = value
            .metadata
            .into_iter()
            .map(InstrumentationMetadata::try_from)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(InstrumentationResult {
            source: value.source,
            metadata,
        })
    }
}

#[napi(object)]
pub struct InstrumentationMetadata {
    #[napi(readonly)]
    pub tag: Buffer,
    #[napi(readonly)]
    pub kind: String,
    #[napi(readonly)]
    pub start_utf16: i64,
    #[napi(readonly)]
    pub end_utf16: i64,
}

impl TryFrom<edr_instrument::coverage::InstrumentationMetadata> for InstrumentationMetadata {
    type Error = usize;

    fn try_from(
        value: edr_instrument::coverage::InstrumentationMetadata,
    ) -> Result<Self, Self::Error> {
        let start_utf16 = value
            .start_utf16
            .try_into()
            .map_err(|_error| value.start_utf16)?;
        let end_utf16 = value
            .end_utf16
            .try_into()
            .map_err(|_error| value.end_utf16)?;

        Ok(InstrumentationMetadata {
            tag: Buffer::from(value.tag.as_slice()),
            kind: value.kind.to_owned(),
            start_utf16,
            end_utf16,
        })
    }
}

#[napi]
pub fn add_statement_coverage_instrumentation(
    source_code: String,
    source_id: String,
    solidity_version: String,
) -> napi::Result<InstrumentationResult> {
    let solidity_version = Version::parse(&solidity_version).map_err(|error| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("Invalid Solidity version: {error}"),
        )
    })?;

    let instrumented = coverage::instrument_code(&source_code, &source_id, solidity_version)
        .map_err(|error| napi::Error::new(napi::Status::GenericFailure, error.to_string()))?;

    instrumented.try_into().map_err(|location| {
        napi::Error::new(
            napi::Status::GenericFailure,
            format!("Cannot represent source locations in JavaScript: {location}."),
        )
    })
}
