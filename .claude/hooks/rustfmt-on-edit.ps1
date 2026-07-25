# PostToolUse(Write|Edit) hook: format edited Rust files with rustfmt.
# Reads the tool-call JSON from stdin, extracts the edited file path, and
# runs rustfmt in place if it is a .rs file. Always exits 0 (never blocks edits).
$ErrorActionPreference = 'SilentlyContinue'
$raw = [Console]::In.ReadToEnd()
try { $j = $raw | ConvertFrom-Json } catch { exit 0 }
$f = $j.tool_input.file_path
if (-not $f) { $f = $j.tool_response.filePath }
if ($f -and $f.ToLower().EndsWith('.rs') -and (Test-Path $f)) {
    rustfmt --edition 2021 $f 2>$null
}
exit 0
