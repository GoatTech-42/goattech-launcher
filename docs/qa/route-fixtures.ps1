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
