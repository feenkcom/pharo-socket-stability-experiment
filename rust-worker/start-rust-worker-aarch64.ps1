$env:RUST_LOG="info"

foreach ($i in 1..960) {
	Start-Sleep -Milliseconds 20
    Start-Process -NoNewWindow -FilePath "target\aarch64-pc-windows-msvc\release\rust-worker.exe"
}