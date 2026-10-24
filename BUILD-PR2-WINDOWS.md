# Veiga's S3CC Manager — PR #2 Windows x64 test build

**This build does not use GitHub Actions.** GitHub has rejected the hosted jobs because of an account billing/payment or spending-limit issue; it has not executed Rust, Node, or Windows tests. Retrying the same workflow without resolving billing will not compile the application.

## One-click local build

1. Open the PR #2 branch at `https://github.com/MrDavidVeiga/S3CC-Manager/tree/fix/beta3-elif-stability-20261024`.
2. Download the branch source using **Code → Download ZIP** and extract it on a **Windows x64** computer.
3. Install **Node.js LTS** and **Rust stable with the MSVC toolchain**, including the Microsoft C++ Build Tools, if they are not installed.
4. Double-click **`GERAR-EXE-PR2-WINDOWS.cmd`** in the extracted source root (or run it in the local Codex terminal).

The builder installs frontend dependencies, restores/generates the icons, compiles the actual Windows Rust/Tauri executable (not a simulated UI), and produces:

- `TESTE-PR2-BETA3-WINDOWS-X64/portable/Veiga's S3CC Manager - Beta 3 PR2 Test.exe`
- `TESTE-PR2-BETA3-WINDOWS-X64/VeigasS3CCManager_PR2_Beta3_Test_Windows_x64_<commit>.zip`
- `TESTE-PR2-BETA3-WINDOWS-X64/SHA256.txt`
- `TESTE-PR2-BETA3-WINDOWS-X64/build.log`

**Tests are not an executable build gate.** The optional `-RunTests` switch runs the regression checks *after* the executable has been produced.

To build from an existing Git checkout instead, ensure you are on `fix/beta3-elif-stability-20261024`, fetch/pull the latest changes, and run the same CMD launcher. The script refuses to mislabel an executable built from an unrelated Git branch.

## First smoke test

Use a **copy** of your Mods folder. Reproduce: scroll thumb dragging in Duplicates; expand/collapse several first, middle, and last entries in Duplicates and Conflicts; cancel Health midway through a long analysis; report a crash or hang with its repro and `build.log`.

The script does not delete, move, or modify any Sims 3 CC files, does not change any release tags, and does not claim that the resulting executable is fully validated.

If the builder fails, provide the last part of `build.log`; this is a **real compilation error or a local prerequisite error**, separate from the GitHub Actions billing blocker.
