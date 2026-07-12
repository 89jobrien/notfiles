#!/usr/bin/env nu
# preflight.nu — notfiles environment validation

def check [label: string, pass: bool, detail: string = ""] {
    if $pass {
        print $"[ok]   ($label)"
    } else if $detail != "" {
        print $"[fail] ($label) — ($detail)"
    } else {
        print $"[fail] ($label)"
    }
    $pass
}

print "=== notfiles preflight ==="

let results = [
    (check "cargo on PATH" (which cargo | length) > 0),
    (check "just on PATH" (which just | length) > 0),
    (check "nu on PATH" (which nu | length) > 0 "nushell required for hook scripts"),
    (check "age on PATH" (which age | length) > 0 "optional — needed for SOPS decryption"),
    (check "sops on PATH" (which sops | length) > 0 "optional — needed for encrypted configs"),
    (check "notfiles on PATH" (which notfiles | length) > 0 "run: cargo install --path crates/notfiles"),
    (check "op on PATH" (which op | length) > 0),
    (check "1Password authed" (do { op account list } | complete | get exit_code) == 0),
    (check "git repo clean" (do { git status --porcelain } | complete | get stdout | str trim | is-empty)),
]

let failed = $results | where { |r| not $r } | length
let total = $results | length

print ""
if $failed == 0 {
    print $"preflight passed ($total)/($total)"
} else {
    print $"preflight ($total - $failed)/($total) — ($failed) check(s) failed"
}
