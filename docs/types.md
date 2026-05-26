# Shared ONVIF schema types

> Cross-service primitive/complex types from the ONVIF core schema. Service files in this
> directory link here instead of repeating these definitions. Reference for implementing oxvif.

- **Schema:** `http://www.onvif.org/ver10/schema/onvif.xsd` (prefix `tt`), version **25.12**
- **Provenance:** types marked ✔ were verified against the fetched schema; types marked ◆ are
  stable ONVIF core definitions (unchanged for many releases) written from spec knowledge —
  re-confirm against onvif.xsd v25.12 if a field looks load-bearing.

Cardinality: `[1]` required · `[0..1]` optional · `[0..*]` repeated · `(attr)` attribute.

---

## Token / string primitives

- **`tt:ReferenceToken`** ◆ — `xs:string` restricted to `maxLength 64`. Used everywhere a
  config/profile/source is addressed by token.
- **`tt:Name`** ✔ — `xs:string` restricted to `maxLength 64`.
- **`tt:IntAttrList`, `tt:FloatAttrList`, `tt:StringAttrList`** ◆ — whitespace-separated lists.

## Ranges / geometry

- **`tt:IntRange`** ◆ — elements: `Min` `xs:int` [1], `Max` `xs:int` [1].
- **`tt:FloatRange`** ✔ — elements: `Min` `xs:float` [1], `Max` `xs:float` [1].
- **`tt:IntRectangle`** ✔ — attrs: `x` `xs:int` [1], `y` [1], `width` [1], `height` [1].
- **`tt:Rectangle`** ◆ — attrs: `bottom` `xs:float`, `top`, `right`, `left` (normalised coords).
- **`tt:Vector`** ◆ — attrs: `x` `xs:float` [1], `y` `xs:float` [1]. (2-D point.)
- **`tt:Vector2D`** ◆ — attrs: `x` `xs:float` [1], `y` [1], `space` `xs:anyURI` [0..1].
- **`tt:Vector1D`** ◆ — attrs: `x` `xs:float` [1], `space` `xs:anyURI` [0..1].

## PTZ

- **`tt:PTZVector`** ◆ — `PanTilt` `tt:Vector2D` [0..1], `Zoom` `tt:Vector1D` [0..1].
- **`tt:PTZSpeed`** ◆ — `PanTilt` `tt:Vector2D` [0..1], `Zoom` `tt:Vector1D` [0..1].
- **`tt:PTZStatus`** ◆ — `Position` `tt:PTZVector` [0..1], `MoveStatus` `tt:PTZMoveStatus` [0..1]
  (`PanTilt`/`Zoom` each an `IDLE|MOVING|UNKNOWN` enum), `Error` `xs:string` [0..1],
  `UtcTime` `xs:dateTime` [0..1].

## Configuration containers (analytics, rules, metadata)

- **`tt:Config`** ◆ — attrs `Name` `xs:string` [1], `Type` `xs:QName` [1]; element
  `Parameters` `tt:ItemList` [1]. Represents one configured rule / analytics module.
- **`tt:ItemList`** ◆ — name/value parameter bag:
  - `SimpleItem` [0..*]: attrs `Name` `xs:string`, `Value` `xs:string`.
  - `ElementItem` [0..*]: attr `Name` `xs:string`; arbitrary XML element content `[0..1]`.
  - `Extension` [0..1].
- **`tt:ItemListDescription`** ◆ — describes allowed items:
  - `SimpleItemDescription` [0..*]: attr `Name`; attr `Type` `xs:QName`.
  - `ElementItemDescription` [0..*]: attr `Name`; attr `Type` `xs:QName`.
- **`tt:ConfigDescription`** ◆ — attr `Name` `xs:QName`; `Parameters` `tt:ItemListDescription` [1];
  `Messages` [0..*]; attr `MaxInstances` `xs:int` [0..1]. (Full message structure: see onvif.xsd.)

## Date / time

- **`tt:DateTime`** ◆ — `Time` (`Hour`/`Minute`/`Second` ints) [1], `Date` (`Year`/`Month`/`Day` ints) [1].
- Wire timestamps in WS-* headers and event messages use `xs:dateTime` (ISO-8601).

---

_Sources: onvif.xsd v25.12 (fetched 2026-05). ✔ verified this fetch; ◆ stable core, verify against
schema when a field is load-bearing._
