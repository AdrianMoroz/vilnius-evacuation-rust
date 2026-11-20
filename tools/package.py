"""Package a built Windows executable with the data and dependency notices."""

import argparse
import json
from pathlib import Path
import subprocess
from zipfile import ZIP_DEFLATED, ZipFile


def package(executable, target, output):
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", target],
        text=True,
        encoding="utf-8",
    ))
    root = metadata["resolve"]["root"]
    dependencies = {node["id"]: node["deps"] for node in metadata["resolve"]["nodes"]}
    pending = [root]
    selected = set()
    while pending:
        identifier = pending.pop()
        if identifier in selected:
            continue
        selected.add(identifier)
        pending.extend(
            dep["pkg"] for dep in dependencies[identifier]
            if any(kind["kind"] is None for kind in dep["dep_kinds"])
        )

    output.parent.mkdir(parents=True, exist_ok=True)
    with ZipFile(output, "w", compression=ZIP_DEFLATED, compresslevel=9) as archive:
        archive.write(executable, "VilniusEvacuation.exe")
        archive.write("README.md", "README.md")
        archive.write("assets/README.md", "OPENSTREETMAP.md")
        notices = ["Third-party dependencies", ""]
        for dependency in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
            if dependency["id"] not in selected or dependency["id"] == root:
                continue
            name = f'{dependency["name"]}-{dependency["version"]}'
            folder = Path(dependency["manifest_path"]).parent
            notices.append(f"{name} — {dependency['license'] or 'See bundled notices'}")
            for path in sorted(folder.rglob("*")):
                if path.is_file() and path.name.upper().startswith(("LICENSE", "COPYING", "COPYRIGHT", "NOTICE", "OFL")):
                    archive.write(path, f"licenses/{name}/{path.relative_to(folder).as_posix()}")
        archive.writestr("THIRD_PARTY_NOTICES.txt", "\n".join(notices) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("executable", type=Path)
    parser.add_argument("target")
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    package(args.executable, args.target, args.output)
