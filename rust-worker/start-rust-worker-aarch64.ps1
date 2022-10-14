$env:RUST_LOG="info"

foreach ($i in 1..500) {
    Start-Process -NoNewWindow -FilePath "target\aarch64-pc-windows-msvc\release\rust-worker.exe"
}