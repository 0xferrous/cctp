# circle-iris

Rust client crate for Circle's Iris / CCTP HTTP API.

## OpenAPI source

This crate pins Circle's published CCTP OpenAPI document from:

- `https://developers.circle.com/openapi/cctp.yaml`

We discovered that spec URL from the Circle docs page:

- `https://developers.circle.com/api-reference/cctp/all/get-public-keys-v2`

## Local mutations to the pinned spec

The pinned `openapi/cctp.yaml` is not byte-for-byte identical to Circle's hosted document.
We made two local compatibility edits so `progenitor` can generate code successfully:

1. Added a shared `components/schemas/ErrorResponse` schema.
2. Changed `components/responses/NotFound` and `components/responses/BadRequest` to reference that shared schema instead of each defining their own inline error object schema.
3. Removed `format: uuid` from `components/schemas/XRequestId`.

These are codegen-compatibility changes only; they are not intended to change the API semantics.

## Runtime/schema mismatch note

Most of the crate is generated from the published OpenAPI spec, but we also keep
one handwritten compatibility layer for `GET /v2/messages/{sourceDomainId}`.

Reason:
- the published spec models several decoded fields as EVM `0x...` addresses
- real Iris responses can include Solana/base58 values in those fields for
  cross-ecosystem transfers
- strict codegen deserialization fails on those real responses

So `src/compat.rs` exists as a tolerant runtime parser for that endpoint.

## Metadata

The crate also exposes a small metadata module for Circle-published supported
chain/domain information:

- `circle_iris::metadata::supported_domains(...)`
- `circle_iris::metadata::find_domain(...)`
- `circle_iris::metadata::find_chain(...)`

## Notes

- Generated with `progenitor`
- Intended to cover the complete published CCTP API surface
- A thin handwritten convenience layer can be added on top of the generated client over time
