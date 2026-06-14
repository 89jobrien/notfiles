# nu_libs — load all lib/* modules on shell startup
# Managed by notfiles (nushell package). Link with: notfiles link nushell
# lib/ai is excluded from lib/mod.nu (requires runtime deps) — loaded separately below.
# lib/extensions is excluded (requires external project repos) — load manually as needed.

const NU_LIBS     = "/Users/joe/dev/nu_libs/lib/mod.nu"
const NU_LIBS_AI  = "/Users/joe/dev/nu_libs/lib/ai/mod.nu"

use $NU_LIBS *
use $NU_LIBS_AI *
