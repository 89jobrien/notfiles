# notfiles toolkit — auto-loaded by toolkit-hook when you cd into this repo

export def "fmt"        [] { cargo fmt --all }
export def "check"      [] { cargo check --workspace }
export def "lint"       [] { cargo clippy --workspace -- -D warnings }
export def "test"       [] { cargo nextest run --workspace }
export def "ci"         [] { just ci }
export def "pre-commit" [] { just pre-commit }
export def "prepush"    [] { just prepush }
export def "install"    [] { just install }
export def "workspace"  [] { just workspace }
export def "clean"      [] { cargo clean }

export def "help" [] {
    scope commands
    | where name =~ "^tk "
    | select name
    | sort-by name
}
