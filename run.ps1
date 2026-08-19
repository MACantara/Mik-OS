param(
    [string]$Kernel = "target/mik-64-kernel.bin"
)

$ErrorActionPreference = "Stop"

cargo build
cargo run -p mik-os -- $Kernel
cargo run -p mik-emu -- $Kernel
