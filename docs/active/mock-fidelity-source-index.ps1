# Dot-sourced by the read-only inventory checker. Source observations, not schema facts.
function Get-ActionSites([string]$Source, [string]$Path) {
    $production = ($Source -split '#\[cfg\(test\)\]', 2)[0]
    $production = [regex]::Replace($production, '(?m)^\s*//[^\r\n]*', '')
    $constants = [regex]::Matches($production, 'const ACTION:\s*&str\s*=\s*"([^"]+)"\s*;')
    if ([regex]::Matches($production, '\b(?:const|let|static)\s+ACTION\b').Count -ne $constants.Count) {
        throw "Unsupported ACTION declaration: $Path; audit extractor."
    }
    foreach ($constant in $constants) {
        $functions = [regex]::Matches($production.Substring(0, $constant.Index), '\bfn\s+(\w+)')
        if ($functions.Count -eq 0) { throw "ACTION without source method: $Path" }
        [pscustomobject]@{
            Site = "${Path}::$($functions[$functions.Count - 1].Groups[1].Value)"
            Action = $constant.Groups[1].Value
        }
    }
}

function Get-ActionRoute([string]$Action) {
    # These are the current source routing families, not a normative service catalogue.
    $families = @{
        'ver10/device' = 'device'; 'ver10/deviceio' = 'device_io'
        'ver10/media' = 'media'; 'ver20/media' = 'media2'
        'ver20/ptz' = 'ptz'; 'ver20/imaging' = 'imaging'
        'ver10/recording' = 'recording'; 'ver10/search' = 'search'; 'ver10/replay' = 'replay'
    }
    $match = [regex]::Match($Action, '^http://www\.onvif\.org/(ver\d+/[a-z]+)/wsdl/([A-Za-z0-9]+)$')
    if ($match.Success -and $families.ContainsKey($match.Groups[1].Value)) {
        return "$($families[$match.Groups[1].Value]).$($match.Groups[2].Value)"
    }
    $match = [regex]::Match($Action, '^http://(?:www\.onvif\.org/ver10/events/wsdl/(?:EventPortType|PullPointSubscription)|docs\.oasis-open\.org/wsn/bw-2/(?:SubscriptionManager|NotificationProducer))/([A-Za-z0-9]+)$')
    if ($match.Success) { return "events.$($match.Groups[1].Value)" }
    throw "Unmapped source Action: $Action; record an explicit routing review."
}

function Get-ReaderSites([string]$Source, [string]$Path) {
    $lines = $Source -split '\r?\n'
    $symbol = '<module>'
    $test = $false
    for ($index = 0; $index -lt $lines.Count; $index++) {
        $line = $lines[$index]
        if ($line -match '^#\[cfg\(test\)\]') { $test = $true }
        if ($line -match '^\s*//') { continue }
        if ($line -match '\bfn\s+(\w+)') { $symbol = $matches[1]; continue }
        foreach ($match in [regex]::Matches($line, '\b(extract_tag|extract_all_tags|extract_attr|required_text|XmlNode::parse)\s*\(')) {
            [pscustomobject]@{
                Site = "${Path}::$symbol"
                Helper = $match.Groups[1].Value
                Kind = if ($test) { 'test' } else { 'production' }
            }
        }
    }
}

function Assert-IndexEqual($Expected, $Actual, [string]$Label) {
    # A multiset catches moved/added/removed occurrences, not merely unique names.
    $counts = [System.Collections.Generic.Dictionary[string,int]]::new([System.StringComparer]::Ordinal)
    foreach ($item in $Expected) {
        if (-not $counts.ContainsKey($item)) { $counts[$item] = 0 }
        $counts[$item]++
    }
    foreach ($item in $Actual) {
        if (-not $counts.ContainsKey($item) -or $counts[$item] -eq 0) {
            throw "$Label unexpected entry: $item"
        }
        $counts[$item]--
    }
    foreach ($item in $counts.Keys) {
        if ($counts[$item] -ne 0) { throw "$Label missing entry: $item" }
    }
}

function Get-AuditRows([string]$Document, [string]$Kind) {
    foreach ($match in [regex]::Matches($Document, '(?m)^\| `(src/[^`]+)` \| `([^`]+)` \| `([^`]+)` \|$')) {
        $site = $match.Groups[1].Value
        $value = $match.Groups[2].Value
        $last = $match.Groups[3].Value
        if ($Kind -eq 'action' -and $value.StartsWith('http://')) { "$site|$value|$last" }
        if ($Kind -eq 'reader' -and $last -match '^(production|test):[1-9][0-9]*$') {
            $parts = $last.Split(':')
            for ($i = 0; $i -lt [int]$parts[1]; $i++) { "$site|$value|$($parts[0])" }
        }
    }
}
