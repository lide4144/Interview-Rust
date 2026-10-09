@call "C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvarsall.bat" x64
@cd /d D:\vscode\Interview\src-tauri
cargo test %* 2>&1