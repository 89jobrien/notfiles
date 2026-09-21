# notgraph Report

## Crate Dependency Graph

- **notstrap** (fan-in: 1, fan-out: 5)
- **nothooks** (fan-in: 2, fan-out: 1)
- **integration** (fan-in: 0, fan-out: 5)
- **notgraph** (fan-in: 0, fan-out: 0)
- **notsecrets** (fan-in: 2, fan-out: 1)
- **notforge** (fan-in: 0, fan-out: 1)
- **notnet** (fan-in: 1, fan-out: 0)
- **notcore** (fan-in: 6, fan-out: 0)
- **notfiles** (fan-in: 2, fan-out: 1)

## Module Graphs

### notcore [OK]

- Modules: 9
- Cycles: 0

### notfiles [OK]

- Modules: 15
- Cycles: 0

### notsecrets [OK]

- Modules: 53
- Cycles: 0

### nothooks [OK]

- Modules: 3
- Cycles: 0

### notnet [OK]

- Modules: 6
- Cycles: 0

### notstrap [OK]

- Modules: 4
- Cycles: 0

### notgraph [OK]

- Modules: 11
- Cycles: 0

### notforge [OK]

- Modules: 6
- Cycles: 0

## Hotspots

### Fan-in (most depended-upon)

| Name | Score |
|------|-------|
| notcore | 6 |
| nothooks | 2 |
| notsecrets | 2 |
| notfiles | 2 |
| notstrap | 1 |
| notnet | 1 |
| notcore::types | 1 |
| notcore::types::tests | 1 |
| notcore::reporter | 1 |
| notcore::error | 1 |
| notcore::config | 1 |
| notcore::config::tests | 1 |
| notcore::paths | 1 |
| notcore::paths::tests | 1 |
| notfiles::ports | 1 |
| notfiles::adapters | 1 |

### Fan-out (highest coupling)

| Name | Score |
|------|-------|
| notstrap | 5 |
| integration | 5 |
| nothooks | 1 |
| notsecrets | 1 |
| notforge | 1 |
| notfiles | 1 |
| notsecrets::sources | 14 |
| notsecrets | 11 |
| notfiles | 8 |
| notgraph | 6 |
| notcore | 5 |
| notnet | 5 |
| notforge | 5 |
| notsecrets::identities | 4 |
| notfiles::adapters | 3 |
| notsecrets::recipients | 3 |

## Public Symbol Inventory

### notcore

**struct**: HookSpec, PackageSpec, Step, Report, SilentReporter, Config, Defaults, PackageConfig

**enum**: HookPhase, StepStatus, LinkEvent, NotfilesError, Method

**trait**: Reporter

**fn**: load_toml_file, default_config_path, default_dotfiles_dir, suggest_package, starter_toml, expand_tilde, dotfiles_dir

### notfiles

**struct**: TerminalReporter, JsonReporter, InMemoryFileStore, FileStoreImpl, DetectedManager, DetectedPackage, StatusEntry, DiffEntry, StateEntry, State, LinkOptions, LinkResult, Cli, IgnoreMatcher

**enum**: ManagerKind, FileStatus, DiffKind, Command

**trait**: FileStore

**fn**: link, link_with_store, unlink, unlink_with_store, discover_packages_filtered, discover_packages_filtered_with_store, discover_packages, discover_packages_with_store, resolve_packages, resolve_packages_with_store, resolve_packages_filtered_with_store, collect_files, collect_files_with_store, detect, print_detected, print_detected_json, package_status, diff_package, print_diff, print_status, print_status_json, link_package, unlink_package, adopt_files

### notsecrets

**struct**: Decryptor, SecretsConfig, Encryptor, ScryptRecipient, SshEd25519Recipient, X25519Recipient, NuenvSource, YubikeySource, EnvSource, DotenvySource, DotenvxSource, BitwardenSource, DirenvSource, PromptSource, GsmSource, MiseSource, SopsSource, OpSource, VaultSource, FileSource, SecretResolver, EncryptedIdentity, ScryptIdentity, SshEd25519Identity, X25519Identity, FileKey, Stanza, Header

**enum**: AgeError, SecretsError, Provider, ProviderConfig, SecretRef

**trait**: IdentitySource, SecretSource, EnumerableSecretSource, Recipient, Identity

**fn**: parse_header, serialize_header, header_bytes_up_to_footer, load_config, resolve_identities

### nothooks

**struct**: HookRunner, HookState

**enum**: HookResult

**fn**: run_phase

### notnet

**struct**: TailscaleOptions

**enum**: NotnetError

**fn**: ensure_connected, is_connected, resolve_auth_key, is_installed, install

### notstrap

**struct**: Prereq, NotstrapConfig, BootstrapSection, BootstrapOptions

**fn**: check_prerequisites, run, parse_env_line, clone_if_missing

### notgraph

**struct**: CrateGraph, ModuleGraph, Symbol, SymbolTable, FanStats, ModStats, Hotspot, GraphStats

**enum**: SymbolKind, HotspotKind

**fn**: build, path_to_mod, fan_stats, detect_cycles, hotspots, analyse, write_all, build, build

**type**: CrateName, ModPath

### notforge

**struct**: RemoteRepository, ForgeVersion, LocalRepository, ForgeConfig, GiteaConfig, LocalProcessConfig, LocalContainerConfig, VmSystemdConfig, ForgeAuthConfig, RepoSpec, GiteaHttpRequest, GiteaHttpResponse, GiteaHttpApi, UreqGiteaTransport

**enum**: ForgeAuth, LifecycleStatus, RemoteStatus, PushStatus, NotforgeError, GiteaMode, ForgeSecretRef, HttpMethod

**trait**: ForgeApi, ForgeLifecycle, GitRemoteManager, SecretResolverPort, GiteaTransport

**fn**: load_config

**const**: VERSION

## Cycle Report

No cycles detected.
