$ErrorActionPreference = "Stop"

$repo = if ($env:CODEXCTL_REPO) { $env:CODEXCTL_REPO } else { "https://github.com/ChaosRealmsAI/codexctl" }

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Error "cargo was not found. Install Rust first: https://www.rust-lang.org/tools/install"
}

$argsList = @("install", "--git", $repo, "--force")

if ($env:CODEXCTL_TAG) {
    $argsList += @("--tag", $env:CODEXCTL_TAG)
} elseif ($env:CODEXCTL_REV) {
    $argsList += @("--rev", $env:CODEXCTL_REV)
} elseif ($env:CODEXCTL_BRANCH) {
    $argsList += @("--branch", $env:CODEXCTL_BRANCH)
}

& cargo @argsList
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

& codexctl --version
Write-Output "Run: codexctl doctor"
