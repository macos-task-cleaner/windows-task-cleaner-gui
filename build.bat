@echo off
echo ========================================================
echo  Task Cleaner (WinUI 3) - 独立单文件 (Standalone) 发布
echo ========================================================
echo.
echo 正在发布 win-x64 单文件免安装独立可执行文件...
dotnet publish src\TaskCleaner.WinUI\TaskCleaner.WinUI.csproj -c Release -r win-x64 --self-contained true -p:PublishSingleFile=true -o publish\win-x64
if %ERRORLEVEL% EQU 0 (
    echo.
    echo [成功] 发布成功！单文件位于: publish\win-x64\TaskCleaner.WinUI.exe
) else (
    echo.
    echo [错误] 发布失败，请检查编译日志。
)
pause
