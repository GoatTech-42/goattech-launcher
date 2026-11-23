$ErrorActionPreference = 'Stop'
# All fixture bytes are made here. No downloads, executable game, tokens or real accounts.
$gameHome = $env:HORIZON_HOME
$data = $env:HORIZON_LAUNCHER_HOME
$resource = Join-Path $gameHome 'resourcepacks/synthetic-resource-pack'
$world = Join-Path $gameHome 'saves/Synthetic_World'
$datapack = Join-Path $world 'datapacks/synthetic-data-pack'
$port = Join-Path $gameHome 'ports/synthetic-detail'
foreach ($dir in @($resource,$datapack,$port,(Join-Path $data 'native'))) { New-Item -ItemType Directory -Force $dir | Out-Null }
'{"pack":{"pack_format":15,"description":"Synthetic resource pack, capture only"}}' | Set-Content (Join-Path $resource 'pack.mcmeta')
'{"pack":{"pack_format":15,"description":"Synthetic data pack, capture only"}}' | Set-Content (Join-Path $datapack 'pack.mcmeta')
# Empty level.dat is a local metadata-only world fixture; no regions or game state.
[IO.File]::WriteAllBytes((Join-Path $world 'level.dat'),[byte[]]@())
'{"name":"Synthetic detail card","format":"instance","minecraft":"1.21.1","loader":"fabric","loader_version":"0.16.0","files":[],"overrides":{}}' | Set-Content (Join-Path $port 'pack.json')
'[]' | Set-Content (Join-Path $port 'coverage.json')
'[]' | Set-Content (Join-Path $port 'inventory.json')
1..40 | ForEach-Object { "[synthetic $_] capture fixture only, no build was run" } | Set-Content (Join-Path $data 'native/build.log')

# Exact shipping manifest schema: SourceId/Kind serialize in kebab-case.
# Horizon source avoids Modrinth icon lookups. No executable payload is supplied.
$entries = @(
  @{source='horizon';project='capture-enabled';slug='capture-enabled';title='Synthetic enabled installed mod with a deliberately long title for layout review';kind='mod';version='capture-v1';number='1.0.0-capture-long-version-label';file='horizon-mods/capture_enabled';checksum='SHA-256:capture-only-not-a-download';page='';namespaces=@('capture_enabled')},
  @{source='horizon';project='capture-disabled';slug='capture-disabled';title='Synthetic disabled installed mod, missing icon';kind='mod';version='capture-v2';number='2.0.0-capture';file='horizon-mods/capture_disabled';checksum='SHA-256:capture-only-not-a-download';page='';namespaces=@('capture_disabled')}
)
foreach ($id in @('capture_enabled','capture_disabled')) { New-Item -ItemType Directory -Force (Join-Path $gameHome "horizon-mods/$id") | Out-Null }
@{entries=$entries} | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $gameHome 'mod-browser.json')
'mod.capture_enabled:true' | Set-Content (Join-Path $gameHome 'horizon-mods.txt')
# Merely shown as field text, never saved through UI or executed.
$launch = @{pre_launch='';wrapper='';post_exit='';env=@{CAPTURE_FIXTURE='synthetic local UI review only'};memory_limit_mb=4096}
@{instances=@{horizon=@{launch=$launch}}} | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $data 'instances.json')
# Existing empty directories satisfy servers::list. No API file, server.toml or runtime.
$serverRows = @()
foreach ($i in 1..2) {
  $dir = Join-Path $env:CAP "synthetic-stopped-server-$i-long-path-for-layout-review"
  New-Item -ItemType Directory -Force $dir | Out-Null
  $serverRows += @{name="Synthetic stopped server $i - deliberately long local name";dir=$dir}
}
ConvertTo-Json -InputObject $serverRows -Depth 4 | Set-Content (Join-Path $data 'servers.json')
if (Test-Path (Join-Path $data 'server-runtime')) { throw 'Capture must have no server runtime' }
