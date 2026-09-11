# Media2 frame-rate migration

[English](media2-frame-rate.md) | [繁體中文](media2-frame-rate_zh.md)

Status: unreleased, approved for the next minor release. This is a source-breaking
correction, not a claim about the installed 0.16.0 binary.

| Section | Purpose |
| --- | --- |
| [Rust callers](#rust-callers) | Fractional rates and source migration |
| [Errors and absence](#errors-and-absence) | Distinguish missing, invalid and zero |
| [JSON and mock state](#json-and-mock-state) | Persisted numbers and Media1 compatibility |
| [Limits](#limits) | What this correction does not implement |

## Rust callers

`VideoRateControl2.frame_rate_limit` changes from `u32` to `f32`. Media2 reads
previously converted valid fractional rates such as `12.5` into `0`; reads and
writes now preserve them. Change integer struct literals and integer-only helper
signatures. Do not cast returned rates to an integer to make compilation pass.

```rust
use oxvif::VideoRateControl2;

let rate = VideoRateControl2 {
    frame_rate_limit: 12.5, // Use 25.0 for an integral rate.
    bitrate_limit: 2048,
};
assert_eq!(rate.frame_rate_limit, 12.5);
```

Both `OnvifClient` and `OnvifSession` use the same contract. Query the selected
encoder's options before writing; acceptance of a finite number does not establish
that a camera supports that setting. `f32` has normal binary floating-point rounding:
do not require decimal string equality or use an unchecked float-to-integer cast.
Media1's separate `VideoRateControl` public type is unchanged.

## Errors and absence

| Input | Result |
| --- | --- |
| No `RateControl` | `None`; the device did not provide it |
| Valid `FrameRateLimit` of `0` | `Some` with `0.0`; not substituted for an error |
| Missing required rate/bitrate within present `RateControl` | `SoapError::MissingField` with the field path |
| Invalid, duplicate or nested scalar; duplicate `RateControl` | `SoapError::InvalidValue` with the affected path |
| Negative or nonfinite rate | Read error; writes fail before transport |

Rate paths are `Configuration/RateControl/FrameRateLimit` and
`Configuration/RateControl/BitrateLimit`. Bitrate remains `u32`; invalid text is
not silently turned into zero. Optional absence is not a request to invent a rate.

## JSON and mock state

Serde still accepts ordinary old integer JSON rates, for example
`{"frame_rate_limit":25,"bitrate_limit":2048}`. New JSON uses numeric values that
can be fractional; consumers must not require an integer-only representation.
Large integers are subject to `f32` precision. Serialization and deserialization
reject negative and nonfinite rates with `frame rate must be finite and nonnegative`;
serialization must not replace a nonfinite rate with `null`.

`mock::VideoEncoderState.frame_rate_limit` also becomes `f32`. Both Media services
share it. The mock validates a present scoped rate block before changing state;
omission retains the existing rate. A refused write preserves state, change hooks
and built-in replay recordings. Successful encoder writes retire dependent
encoder/profile recordings across both services.

Media1 cannot represent fractional rates. A mock Media1 list or profile containing
an incompatible encoder returns a top-level Receiver / `mock:RequestPolicy` Fault:
`Encoder frame rate cannot be represented by Media1; use Media2`. It does not
round, omit the encoder or rewrite shared state. An unaffected individual encoder
remains readable. Media1 writes must be nonnegative `i32` values exactly representable
in the shared `f32`; incompatible large integers refuse instead of rounding.
Invalid manually seeded rates also refuse rendering, without modifying the seed.
These are explicit mock policies, not assertions about a real device's Faults.

## Limits

The subsequent VE1 batch adds bounded encoder selectors, options, bitrate adaptation,
complete-candidate validation, codec views and synthetic capacity. The mock adapts
provided rates to its advertised choices (zero becomes 1); client parsing still
preserves a valid wire zero. Unmodeled streaming effects refuse explicitly; see the
[encoder contract](mock-server.md#622-encoder-configuration-contract). The CLI profile
report still uses Media1; no automatic Media2 fallback was added. Mock validation
does not establish camera behavior or full ONVIF conformance. See the
[encoder plan and evidence](active/mock-fidelity-video-encoder.md).
