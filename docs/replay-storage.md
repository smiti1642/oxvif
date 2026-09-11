# Recording storage and report identity

[English](replay-storage.md) | [繁體中文](replay-storage_zh.md)

Status: 0.17 candidate behavior; not part of published 0.16 artifacts.

| Section | Purpose |
| --- | --- |
| [Storage and lookup](#storage-and-lookup) | Collision retention and API migration |
| [Existing files and privacy](#existing-files-and-privacy) | Load/save and downgrade limits |
| [Reports](#reports) | Nonunique grouping keys and conservative differences |
| [Scope](#scope) | What the repair does not establish |

## Storage and lookup

`FixtureStore` uses `(action, key_canon)` to find a bucket, not a unique request.
The legacy projection strips namespace distinctions, normalizes some text and
masks local-name fields. Distinct XML requests can therefore share a key.

- `record` replaces an entry only when its sanitized request is equivalent within
  the same bucket. Otherwise it appends another fixture. Replacement preserves
  the entry's position; distinct requests retain insertion order.
- `lookup(action, key_canon)` returns a fixture only for a singleton bucket.
  An ambiguous key returns `None`, rather than selecting the last recording.
- Use `lookup_request(action, request_xml)` when the request is available. It
  selects a unique matching request after the existing credential transforms.
  No unique match returns `None`. Built-in replay uses this API and retains its
  existing synthetic fallback and committed-effect invalidation policies.

Request equivalence accepts supported namespace aliases, decoded scalar spellings
and selected qualified SOAP-header ephemera. An unqualified `Header/MessageID` or
a body `Created` field is not trusted transport ephemera. Nonidentical malformed
XML, mixed content and unresolved `xsi:type` bindings do not establish equivalence;
exact sanitized bytes remain supported for intentional raw fixtures. This is a
bounded comparison, not general XML canonicalization or ONVIF validation.

## Existing files and privacy

The `fixtures.json` shape is unchanged. Loading removes URL credential pairs from
legacy keys in memory and builds collision buckets. It retains distinct requests;
equivalent duplicates in a bucket retain the last stored entry. Loading does not
rewrite the source file. Explicit `save` persists the current store.

Keep a backup before downgrade: older readers can accept this JSON but collapse
its collision buckets, and subsequently saving can lose entries. Records already
overwritten by older versions cannot be reconstructed by this repair.

New recordings use the existing targeted WS-Security Password/Nonce and literal
URL `user:pass@` transforms. Loading does not resanitize legacy raw envelopes.
Neither operation certifies arbitrary device data as secret-free; review captures
and old copies before sharing. Save durability/atomic replacement and a versioned
key-format redesign are not included in this repair.

## Reports

`(action, key_canon)` is a grouping key, not a unique row ID. Parsing, structural
comparison and side-by-side reports retain all fixtures. For full reports from
the same unchanged store, use the insertion ordinal to associate rows;
`FixtureProgress.done` is the one-based ordinal within its pass. Do not join a
filtered `QuirkReport` to other reports using the pair alone or assuming its row
positions still match the full store.

Legacy `QuirkReport` rows contain no complete request identity. If either side of
a `QuirkDiff` group has duplicate pairs, the comparison retains all current rows
in `appeared` and all previous rows in `resolved`, without inventing `changed`
pairings. These are **unmatched observations**, not proof of a newly introduced or
fixed device defect. Even identical ambiguous reports can therefore yield a
nonempty diff. Inspect the underlying captures to establish correspondence.
Singleton groups retain the existing path-difference behavior.

## Scope

This repairs K27 storage loss and the related report-map collapse without changing
ONVIF methods, SOAP encoding, the JSON shape or raw adapter contracts. It does not
complete W19, all replay dependencies, QName semantics or protocol conformance.
Release-wide acceptance remains governed by the
[0.17 release cut](active/release-0.17-cut.md).
