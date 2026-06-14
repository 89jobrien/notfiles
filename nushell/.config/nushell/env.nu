# Environment variables

# Guard: reset PWD to $HOME if inherited value is not an absolute path.
# if not ($env.PWD | str starts-with "/") {
#     $env.PWD = $env.HOME
# }

# ── PATH ────────────────────────────────────────────────────────────────────
$env.PATH = (
    $env.PATH
    | prepend ($env.HOME | path join ".local/bin")
    | prepend ($env.HOME | path join ".local/share/mise/shims")
    | prepend ($env.HOME | path join ".bun/bin")
    | prepend "/opt/zerobrew/bin"
    | prepend ($env.HOME | path join ".zerobrew/bin")
    | prepend ($env.HOME | path join ".nix-profile/bin")
    | prepend "/nix/var/nix/profiles/default/bin"
    | prepend ($env.HOME | path join ".cargo/bin")
    | prepend "/opt/homebrew/bin"
    | prepend "/opt/homebrew/sbin"
    | prepend "/opt/homebrew/opt/openjdk/bin"
    | prepend "/opt/homebrew/share/google-cloud-sdk/bin"
    | uniq
)

# ── ENV_CONVERSIONS ──────────────────────────────────────────────────────────
# Teach nushell to handle colon-separated vars from external tools
$env.ENV_CONVERSIONS = {
  PATH: {
    from_string: { |s| $s | split row (char esep) | path expand -n }
    to_string:   { |v| $v | str join (char esep) }
  }
  XDG_DATA_DIRS: {
    from_string: { |s| $s | split row (char esep) }
    to_string:   { |v| $v | str join (char esep) }
  }
}

# ── Core env ─────────────────────────────────────────────────────────────────
$env.SHLVL = 1
$env.EDITOR = "vim"
$env.VISUAL = "zed --wait"
$env.BUN_INSTALL = ($env.HOME | path join ".bun")
$env.JAVA_HOME = "/opt/homebrew/opt/openjdk"
$env.RTK_HOOK_AUDIT = "1"

# ── mise ─────────────────────────────────────────────────────────────────────
# Shims are on PATH above. Full activation (hooks, cd triggers) requires
# sourcing `mise activate nu` in config.nu — see settings.nu note.
$env.MISE_SHELL = "nu"

# ── Secrets / SOPS ──────────────────────────────────────────────────────────
let age_key = ($env.HOME | path join ".config/sops/age/keys.txt")
# Populate keys.txt from 1Password if it's missing or contains only a placeholder
if (which op | is-not-empty) {
    let needs_refresh = (
        not ($age_key | path exists) or
        (open $age_key | str trim) == "AGE-SECRET-KEY-1TESTKEY"
    )
    if $needs_refresh {
        try {
            op item get 6meypnchchq3tsb32mdnzxtlia --fields notesPlain
            | str replace --all '"' ''
            | lines
            | where { |l| $l | str starts-with "AGE-SECRET-KEY" }
            | str join "\n"
            | save --force $age_key
        }
    }
}
if ($age_key | path exists) {
    $env.SOPS_AGE_KEY_FILE = $age_key
    $env.MISE_SOPS_AGE_KEY_FILE = $age_key
}

# Bootstrap secrets (dotenv format, no op:// refs)
let bootstrap_secrets = ($env.HOME | path join ".config/dev-bootstrap/secrets.env")
if ($bootstrap_secrets | path exists) {
    open $bootstrap_secrets
    | lines
    | where { |l| not ($l | str starts-with "#") and ($l | str trim | str length) > 0 }
    | each { |l| $l | parse "{key}={value}" | first }
    | each { |kv| load-env {($kv.key): $kv.value} }
    | ignore
}

# Cache dotenvx private key so nuenv .env.nu never re-prompts 1Password
if (which op | is-not-empty) and ($env.DOTENV_PRIVATE_KEY? | default "" | is-empty) {
    try {
        $env.DOTENV_PRIVATE_KEY = (
            op read "op://Personal/nihl7o2bojy53zy4aqtr7txyqi/password"
                --account=my.1password.com
        )
    }
}

# ── Colima / Docker ──────────────────────────────────────────────────────────
let colima_dev_sock = ($env.HOME | path join ".colima/dev/docker.sock")
let colima_default_sock = ($env.HOME | path join ".config/colima/default/docker.sock")
if ($colima_dev_sock | path exists) {
    $env.DOCKER_HOST = $"unix://($colima_dev_sock)"
} else if ($colima_default_sock | path exists) {
    $env.DOCKER_HOST = $"unix://($colima_default_sock)"
}

# ── Maestro ──────────────────────────────────────────────────────────────────
$env.MAESTRO_API_URL = "https://api.maestro-staging.toptal.net"
$env.MAESTRO_RESOURCE_PROFILE = "development"


# ── Homebrew ──────────────────────────────────────────────────────────────────
$env.HOMEBREW_PREFIX     = "/opt/homebrew"
$env.HOMEBREW_CELLAR     = "/opt/homebrew/Cellar"
$env.HOMEBREW_REPOSITORY = "/opt/homebrew"

# ── Handon banner ────────────────────────────────────────────────────────────
# Show top handoff items for current repo on shell start.
if (which handoff-detect | is-not-empty) {
    let _result = (do { run-external "handoff-detect" $env.PWD } | complete)
    if $_result.exit_code == 0 and ($_result.stdout | str trim | is-not-empty) {
        print $"(ansi cyan)--- handoff ---"
        print $_result.stdout
        print (ansi reset)
    }
}

# ── SSH agent ────────────────────────────────────────────────────────────────
# Prefer gpg-agent (YubiKey/OpenPGP), fall back to native macOS launchd agent.
$env.SSH_AUTH_SOCK = (
    [
        (^gpgconf --list-dirs agent-ssh-socket | str trim)
        (glob "/var/run/com.apple.launchd.*/Listeners" | first | default "")
    ]
    | where { |it| $it | path exists }
    | first
    | default ""
)
$env.PATH = ($env.PATH | split row (char esep) | where { $in != "/Users/joe/Library/Application Support/carapace/bin" } | prepend "/Users/joe/Library/Application Support/carapace/bin")

def --env get-env [name] { $env | get $name }
def --env set-env [name, value] { load-env { $name: $value } }
def --env unset-env [name] { hide-env $name }

let carapace_completer = {|spans|
  load-env {
  	CARAPACE_SHELL_BUILTINS: (help commands | where category != "" | get name | each { split row " " | first } | uniq  | str join "\n")
  	CARAPACE_SHELL_FUNCTIONS: (help commands | where category == "" | get name | each { split row " " | first } | uniq  | str join "\n")
  }

  # if the current command is an alias, get it's expansion
  let expanded_alias = (scope aliases | where name == $spans.0 | $in.0?.expansion?)

  # overwrite
  let spans = (if $expanded_alias != null  {
    # put the first word of the expanded alias first in the span
    $spans | skip 1 | prepend ($expanded_alias | split row " " | take 1)
  } else {
    $spans | skip 1 | prepend ($spans.0)
  })

  carapace $spans.0 nushell ...$spans
  | from json
}

mut current = (($env | default {} config).config | default {} completions)
$current.completions = ($current.completions | default {} external)
$current.completions.external = ($current.completions.external
| default true enable
# backwards compatible workaround for default, see nushell #15654
| upsert completer { if $in == null { $carapace_completer } else { $in } })

$env.config = $current
    
