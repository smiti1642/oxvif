# Read-only source/document consistency check. Not a Rust parser or schema validator.
[CmdletBinding()]
param([switch]$SelfTest)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Get-Routes([string]$Source) {
    $production = ($Source -split '#\[cfg\(test\)\]', 2)[0]
    $functions = [regex]::Matches($production, '(?ms)^fn dispatch_(\w+)\([^\n]*\)[^{]*\{(.*?)^\}')
    if ($functions.Count -eq 0) { throw 'No dispatch functions extracted; audit extractor.' }
    $routes = @()
    foreach ($function in $functions) {
        $service = $function.Groups[1].Value
        $body = [regex]::Replace($function.Groups[2].Value, '(?m)^\s*//[^\r\n]*', '')
        $arms = [regex]::Matches($body, '"([A-Za-z0-9]+)"\s*=>\s*(?:\{\s*)?([a-z0-9_:]+)\(([^)]*)\)')
        $arrows = [regex]::Matches($body, '=>').Count
        if ($arms.Count -eq 0 -or $arrows -ne ($arms.Count + 1) -or $body -notmatch '_\s*=>\s*return None') {
            throw "Unrecognized route syntax in $service; audit extractor, do not lower counts."
        }
        foreach ($arm in $arms) {
            $routes += [pscustomobject]@{
                Key = "$service.$($arm.Groups[1].Value)"
                Handler = $arm.Groups[2].Value
                Args = [regex]::Replace($arm.Groups[3].Value.Trim(), '\s+', ' ')
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
    Write-Output 'PASS: inventory self-tests (positive plus 10 rejection cases); no files modified.'
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
    $parts = $row.Handler -split '::'
    $path = if ($parts.Count -eq 2) { "src/mock/services/$($parts[0]).rs" } else { 'src/mock/helpers.rs' }
    $implementation = Get-Content -LiteralPath (Join-Path $repository $path) -Encoding UTF8 -Raw
    if ($implementation -notmatch ('\bfn\s+' + [regex]::Escape($parts[-1]) + '\s*\(')) {
        throw "Handler definition not found: $($row.Handler) in $path"
    }
}
Write-Output "PASS: $($sourceRows.Count) source routes equal both ledgers; handlers and tracking checked."
Write-Output 'NOT CHECKED: normative contracts, handler internals, full Action URI validation, fidelity, or CI execution.'
