# Compatibility Database

The compatibility database maps known applications and hashes to recommended OpenNTX behavior.

## Recognition

Profiles may identify apps by:

- app name
- vendor
- version
- SHA-256 hashes
- installer filename patterns
- detected imported DLLs

## Status Levels

- `unknown`: no reliable data.
- `broken`: known not to work.
- `boots`: process starts but app may not be usable.
- `usable`: core workflow works with issues.
- `silver`: mostly works with known limitations.
- `gold`: works well with documented settings.
- `native-feeling`: integrates cleanly with Linux desktop expectations.

## Profile Contents

- recommended Windows version
- required runtime modules
- DLL overrides
- graphics policy
- sandbox policy
- known fixes
- known issues
- notes

Compatibility profiles must not overstate support.
