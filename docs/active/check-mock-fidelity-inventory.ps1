# Read-only source/document consistency check. Not a Rust parser or schema validator.
[CmdletBinding()]
param([switch]$SelfTest)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot 'mock-fidelity-source-index.ps1')

function Get-Routes([string]$Source) {
    $production = ($Source -split '#\[cfg\(test\)\]', 2)[0]
    $functions = [regex]::Matches($production, '(?ms)^fn dispatch_(\w+)\([^)]*\)[^{]*\{(.*?)^\}')
    if ($functions.Count -eq 0) { throw 'No dispatch functions extracted; audit extractor.' }
    $routes = @()
    foreach ($function in $functions) {
        $service = $function.Groups[1].Value
        $body = [regex]::Replace($function.Groups[2].Value, '(?m)^\s*//[^\r\n]*', '')
        $arms = [regex]::Matches($body, '(?<=[{,}])\s*((?:"[A-Za-z0-9]+"\s*\|\s*)*"[A-Za-z0-9]+")\s*=>\s*(?:\{\s*)?([a-z0-9_:]+)\(([^)]*)\)')
        $arrows = [regex]::Matches($body, '=>').Count
        if ($arms.Count -eq 0 -or $arrows -ne ($arms.Count + 1) -or $body -notmatch '_\s*=>\s*return None') {
            throw "Unrecognized route syntax in $service; audit extractor, do not lower counts."
        }
        foreach ($arm in $arms) {
            foreach ($operation in [regex]::Matches($arm.Groups[1].Value, '"([A-Za-z0-9]+)"')) {
                $routes += [pscustomobject]@{
                    Key = "$service.$($operation.Groups[1].Value)"
                    Handler = $arm.Groups[2].Value
                    Args = [regex]::Replace($arm.Groups[3].Value.Trim(), '\s+', ' ')
                }
            }
        }
    }
    # Also detect a dispatcher that no longer matches the supported function shape.
    if ([regex]::Matches($production, '\bfn dispatch_\w+\(').Count -ne $functions.Count) {
        throw 'Unrecognized dispatch function shape; audit extractor.'
    }
    return $routes
}

function Get-Ledger([string]$Document) {
    $rows = @()
    foreach ($line in ($Document -split '\r?\n')) {
        if ($line -notmatch '^\| `([a-z0-9_]+\.[A-Za-z0-9]+)` \|') { continue }
        $columns = $line.Split('|')
        if ($columns.Count -ne 12) { throw "Invalid ledger row: $($matches[1])" }
        $rows += [pscustomobject]@{
            Key = $columns[1].Trim().Trim([char]96)
            Handler = $columns[2].Trim().Trim([char]96)
            Args = $columns[3].Trim().Trim([char]96)
            Tracking = ($columns[4..10] | ForEach-Object { $_.Trim() }) -join '|'
        }
    }
    if ($rows.Count -eq 0) { throw 'No ledger rows found; cannot pass an empty inventory.' }
    return $rows
}

function Assert-RoutesMatch($Expected, $Actual) {
    $lookup = [System.Collections.Generic.Dictionary[string,object]]::new([System.StringComparer]::Ordinal)
    foreach ($row in $Actual) {
        if ($lookup.ContainsKey($row.Key)) { throw "Duplicate ledger route: $($row.Key)" }
        $lookup[$row.Key] = $row
    }
    $seen = [System.Collections.Generic.Dictionary[string,object]]::new([System.StringComparer]::Ordinal)
    foreach ($row in $Expected) {
        if ($seen.ContainsKey($row.Key)) { throw "Duplicate source route: $($row.Key)" }
        $seen[$row.Key] = $true
        if (-not $lookup.ContainsKey($row.Key)) { throw "Missing ledger route: $($row.Key)" }
        $other = $lookup[$row.Key]
        if ($row.Handler -cne $other.Handler -or $row.Args -cne $other.Args) {
            throw "Route target/arguments changed: $($row.Key); review and update both ledgers."
        }
    }
    foreach ($key in $lookup.Keys) {
        if (-not $seen.ContainsKey($key)) { throw "Stale ledger route: $key" }
    }
}

function Assert-Rejected([scriptblock]$Probe, [string]$ExpectedMessage) {
    try { & $Probe } catch {
        if ($_.Exception.Message -like "$ExpectedMessage*") { return }
        throw
    }
    throw "Self-test did not reject: $ExpectedMessage"
}

if ($SelfTest) {
    $source = @'
fn dispatch_demo(op: &str) -> Option<String> {
    Some(match op {
        "Read" => demo::read(state),
        "Write" => {
            demo::write(state, body)
        }
        _ => return None,
    })
}
'@
    $document = @'
| `demo.Read` | `demo::read` | `state` | W00 | TODO | TODO | TODO | TODO | TODO | - |
| `demo.Write` | `demo::write` | `state, body` | W00 | TODO | TODO | TODO | TODO | TODO | - |
'@
    $expected = @(Get-Routes $source)
    $actual = @(Get-Ledger $document)
    if ($expected.Count -ne 2) { throw 'Self-test route count mismatch.' }
    Assert-RoutesMatch $expected $actual
    $multiline = $source.Replace('fn dispatch_demo(op: &str)', "fn dispatch_demo(`n    op: &str,`n)")
    Assert-RoutesMatch $expected @(Get-Routes $multiline)
    $grouped = $source.Replace('"Read" =>', '"Read" | "List" =>')
    $groupedRows = @(Get-Routes $grouped)
    Assert-IndexEqual @('demo.Read', 'demo.List', 'demo.Write') @($groupedRows.Key) 'Grouped routes'
    Assert-Rejected { Assert-RoutesMatch $groupedRows $actual } 'Missing ledger route: demo.List'
    Assert-Rejected { Get-Routes ($grouped.Replace('"Read" |', 'READ |')) } 'Unrecognized route syntax'
    Assert-Rejected { Assert-RoutesMatch $expected @($actual[0]) } 'Missing ledger route'
    Assert-Rejected { Assert-RoutesMatch @($expected[0]) $actual } 'Stale ledger route'
    Assert-Rejected { Assert-RoutesMatch $expected @($actual + $actual[0]) } 'Duplicate ledger route'
    Assert-Rejected { Assert-RoutesMatch @($expected + $expected[0]) $actual } 'Duplicate source route'
    Assert-Rejected { Assert-RoutesMatch $expected @(Get-Ledger ($document.Replace('demo::read', 'demo::other'))) } 'Route target/arguments changed'
    Assert-Rejected { Assert-RoutesMatch $expected @(Get-Ledger ($document.Replace('`state`', '`body`'))) } 'Route target/arguments changed'
    Assert-Rejected { Assert-RoutesMatch $expected @(Get-Ledger ($document.Replace('demo.Read', 'demo.read'))) } 'Missing ledger route'
    Assert-Rejected { Get-Routes '' } 'No dispatch functions'
    Assert-Rejected { Get-Ledger '' } 'No ledger rows'
    Assert-Rejected { Get-Routes ($source.Replace('demo::read(state)', 'if yes { demo::read(state) } else { demo::other(state) }')) } 'Unrecognized route syntax'
    Write-Output 'PASS: inventory self-tests (positive plus 12 rejection cases); no files modified.'
    $actionSource = 'fn sample() { const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/GetProfiles"; }'
    $actionSites = @(Get-ActionSites $actionSource 'src/demo.rs')
    if ($actionSites.Count -ne 1 -or $actionSites[0].Site -cne 'src/demo.rs::sample' -or
        (Get-ActionRoute $actionSites[0].Action) -cne 'media.GetProfiles') { throw 'Action extraction control failed.' }
    $overlayNamespace = 'http://www.onvif.org/ver20/media/wsdl'
    $overlaySource = 'async fn read() { self.service.call("GetOSDs", "GetOSDsResponse", body); } async fn set() { self.write("SetOSD", "SetOSDResponse", body); }'
    $overlayActions = @(Get-OverlayActionSites $overlaySource 'src/demo.rs' $overlayNamespace)
    Assert-IndexEqual @('src/demo.rs::read|media2.GetOSDs', 'src/demo.rs::set|media2.SetOSD') @(
        $overlayActions | ForEach-Object { "$($_.Site)|$(Get-ActionRoute $_.Action)" }
    ) 'Overlay Action'
    Assert-Rejected { Get-OverlayActionSites ($overlaySource.Replace('"GetOSDs"', 'operation')) 'src/demo.rs' $overlayNamespace } 'Unsupported overlay Action'
    Assert-Rejected { Get-ActionSites 'fn sample() { const ACTION: &str = concat!("a", "b"); }' 'src/demo.rs' } 'Unsupported ACTION declaration'
    Assert-Rejected { Get-ActionRoute 'urn:audit/events/wsdl/GetProfiles' } 'Unmapped source Action'
    Assert-Rejected { Get-ActionRoute 'http://www.onvif.org/ver10/media/wsdl/extra/GetProfiles' } 'Unmapped source Action'
    Assert-Rejected { Assert-IndexEqual @('A|uri|route') @('A|other|route') 'Action' } 'Action unexpected entry'
    Assert-Rejected { Assert-IndexEqual @('A', 'B') @('A') 'Index' } 'Index missing entry'
    Assert-Rejected { Assert-IndexEqual @('A') @('A', 'A') 'Index' } 'Index unexpected entry'
    $readerSource = "fn sample() {`n extract_tag(body, name);`n extract_tag(body, name);`n}`n#[cfg(test)]`nmod tests {`nfn check() {`n extract_attr(body, name, attr);`n}`n}"
    $readerSites = @(Get-ReaderSites $readerSource 'src/demo.rs')
    Assert-IndexEqual @('src/demo.rs::sample|extract_tag|production', 'src/demo.rs::sample|extract_tag|production', 'src/demo.rs::check|extract_attr|test') @($readerSites | ForEach-Object { "$($_.Site)|$($_.Helper)|$($_.Kind)" }) 'Reader control'
    Write-Output 'PASS: Action/reader positive controls and 7 rejection controls; no files modified.'
}

$repository = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../..')).Path
$dispatchPath = Join-Path $repository 'src/mock/dispatch.rs'
$sourceRows = @(Get-Routes (Get-Content -LiteralPath $dispatchPath -Encoding UTF8 -Raw))
$english = @(Get-Ledger (Get-Content -LiteralPath (Join-Path $PSScriptRoot 'mock-fidelity-operation-ledger.md') -Encoding UTF8 -Raw))
$chinese = @(Get-Ledger (Get-Content -LiteralPath (Join-Path $PSScriptRoot 'mock-fidelity-operation-ledger_zh.md') -Encoding UTF8 -Raw))
Assert-RoutesMatch $sourceRows $english
Assert-RoutesMatch $sourceRows $chinese
$translations = [System.Collections.Generic.Dictionary[string,object]]::new([System.StringComparer]::Ordinal)
foreach ($row in $chinese) { $translations[$row.Key] = $row.Tracking }
foreach ($row in $english) {
    if ($row.Tracking -cne $translations[$row.Key]) { throw "Bilingual tracking differs: $($row.Key)" }
}
foreach ($row in $sourceRows) {
    $parts = ($row.Handler -creplace '^crate::mock::services::', '') -split '::'
    $path = if ($parts.Count -ge 2) {
        'src/mock/services/' + ($parts[0..($parts.Count - 2)] -join '/') + '.rs'
    } else { 'src/mock/helpers.rs' }
    $implementation = Get-Content -LiteralPath (Join-Path $repository $path) -Encoding UTF8 -Raw
    if ($implementation -notmatch ('\bfn\s+' + [regex]::Escape($parts[-1]) + '\s*\(')) {
        throw "Handler definition not found: $($row.Handler) in $path"
    }
}
Write-Output "PASS: $($sourceRows.Count) source routes equal both ledgers; handlers and tracking checked."
$actions = @()
foreach ($file in @((Get-ChildItem -LiteralPath (Join-Path $repository 'src/client') -Filter '*.rs')) + @((Get-Item -LiteralPath (Join-Path $repository 'src/session.rs')))) {
    $path = $file.FullName.Substring($repository.Length + 1).Replace('\', '/')
    $actions += @(Get-ActionSites (Get-Content -LiteralPath $file.FullName -Encoding UTF8 -Raw) $path)
}
$overlaySource = Get-Content -LiteralPath (Join-Path $repository 'src/overlays.rs') -Encoding UTF8 -Raw
$scopedRpc = Get-Content -LiteralPath (Join-Path $repository 'src/scoped_rpc.rs') -Encoding UTF8 -Raw
$binding = [regex]::Match($scopedRpc, '(?m)^pub\(crate\) const MEDIA2: &str = "([^"]+)";\r?$')
if (-not $binding.Success -or $overlaySource -cnotmatch '\bnamespace:\s*MEDIA2\s*,') {
    throw 'Media2 overlay binding changed; review the scoped Action inventory.'
}
foreach ($path in @('src/overlays.rs', 'src/overlays/osd.rs', 'src/overlays/mask.rs')) {
    $source = Get-Content -LiteralPath (Join-Path $repository $path) -Encoding UTF8 -Raw
    $actions += @(Get-OverlayActionSites $source $path $binding.Groups[1].Value)
}
if ($actions.Count -eq 0) { throw 'Empty client Action inventory.' }
$actionRows = @($actions | ForEach-Object { "$($_.Site)|$($_.Action)|$(Get-ActionRoute $_.Action)" })
$keys = @($actions | ForEach-Object { Get-ActionRoute $_.Action } | Sort-Object -Unique)
# GetSearchState is exposed by the Mock SOAP runtime and exercised via raw requests.
$mockOnlyKey = 'search.GetSearchState'
Assert-IndexEqual @($sourceRows | ForEach-Object { $_.Key }) ($keys + $mockOnlyKey) 'Action route coverage'
$readers = @()
foreach ($file in Get-ChildItem -LiteralPath (Join-Path $repository 'src/mock') -Recurse -Filter '*.rs') {
    $path = $file.FullName.Substring($repository.Length + 1).Replace('\', '/')
    $readers += @(Get-ReaderSites (Get-Content -LiteralPath $file.FullName -Encoding UTF8 -Raw) $path)
}
if ($readers.Count -eq 0) { throw 'Empty reader inventory.' }
$readerRows = @($readers | ForEach-Object { "$($_.Site)|$($_.Helper)|$($_.Kind)" })
foreach ($name in @('mock-fidelity-source-audit.md', 'mock-fidelity-source-audit_zh.md')) {
    $audit = Get-Content -LiteralPath (Join-Path $PSScriptRoot $name) -Encoding UTF8 -Raw
    Assert-IndexEqual $actionRows @(Get-AuditRows $audit 'action') 'Action source index'
    Assert-IndexEqual $readerRows @(Get-AuditRows $audit 'reader') 'Reader source index'
}
Write-Output "PASS: $($actions.Count) client Action sites cover $($keys.Count) routes plus one Mock-only route; $($readers.Count) reader occurrences match both source audits."
Write-Output 'NOT CHECKED: normative contracts, arbitrary Rust syntax/aliases, transitive call graphs, runtime Action rejection, fidelity, or CI execution.'
