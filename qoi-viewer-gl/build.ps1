if (-NOT(Test-Path 'RGFW.h')) {
    Invoke-WebRequest -OutFile 'RGFW.h' 'https://raw.githubusercontent.com/ColleagueRiley/RGFW/refs/heads/main/RGFW.h'
}
if (-NOT(Test-Path 'rglLoad.h')) {
    Invoke-WebRequest -OutFile 'rglLoad.h' 'https://github.com/ColleagueRiley/RGFW/raw/refs/heads/main/examples/gl33/rglLoad.h'
}

cargo build

cl.exe /nologo main.c /link ../target/debug/qoi_rs.dll.lib
