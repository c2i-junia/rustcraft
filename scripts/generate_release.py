#!/bin/python3

import os
import shutil
import platform
import subprocess
import sys

def log(message):
    """Log messages to the console."""
    print(f"[INFO] {message}")

def run_command(command):
    """Run a shell command and ensure it succeeds."""
    log(f"Executing: {command}")
    result = subprocess.run(command, shell=True)
    if result.returncode != 0:
        log(f"Command failed: {command}")
        sys.exit(1)

def run_python_script(script, args=""):
    """Run a Python script cross-platform."""
    python_executable = "python3" if platform.system() != "Windows" else "python"
    command = f"{python_executable} {script} {args}"
    run_command(command)

def main():
    if len(sys.argv) < 2:
        print("Usage: python build_package.py <version> [--no-compression]")
        sys.exit(1)

    version = sys.argv[1]
    no_compression = "--no-compression" in sys.argv
    working_dir = "release"
    versioned_dir = ""

    # Step 1: Run `just release`
    run_command("just generate-release-folder")

    # Step 2: Create a `version.txt` file
    version_file_path = os.path.join(working_dir, "version.txt")
    with open(version_file_path, "w") as version_file:
        version_file.write(version)
    log(f"Created version.txt with version: {version}")

    # Step 3: Detect operating system
    os_name = platform.system().lower()

    if os_name == "linux":
        versioned_dir = f"rustcraft-{version}-linux-x86_64"
        shutil.move(working_dir, versioned_dir)
        log(f"Renamed {working_dir} to {versioned_dir}")
        if not no_compression:
            tar_file = f"{versioned_dir}.tar.gz"
            shutil.make_archive(versioned_dir, 'gztar', root_dir=versioned_dir)
            log(f"Compressed {versioned_dir} into {tar_file}")
    elif os_name == "windows":
        versioned_dir = f"rustcraft-{version}-windows-x86_64"
        shutil.move(working_dir, versioned_dir)
        log(f"Renamed {working_dir} to {versioned_dir}")
        if not no_compression:
            zip_file = f"{versioned_dir}.zip"
            shutil.make_archive(versioned_dir, 'zip', root_dir=versioned_dir)
            log(f"Compressed {versioned_dir} into {zip_file}")
    else:
        log("Unsupported operating system. Only Linux and Windows are supported.")
        sys.exit(1)

    log("Build and packaging process completed successfully.")

if __name__ == "__main__":
    main()
