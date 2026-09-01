#!/usr/bin/env python3
"""Create normalized release archives using only the Python standard library."""

from __future__ import annotations

import argparse
import gzip
import os
from pathlib import Path
import stat
import tarfile
import zipfile


ZIP_EPOCH = 315532800  # 1980-01-01T00:00:00Z, the earliest ZIP timestamp.


def archive_paths(source: Path) -> list[Path]:
    return [source, *sorted(source.rglob("*"), key=lambda path: path.relative_to(source).as_posix())]


def archive_name(source: Path, path: Path) -> str:
    if path == source:
        return source.name
    return f"{source.name}/{path.relative_to(source).as_posix()}"


def normalized_mode(path: Path) -> int:
    if path.is_dir():
        return 0o755
    mode = stat.S_IMODE(path.stat().st_mode)
    return 0o755 if mode & 0o111 else 0o644


def create_tar_gz(source: Path, output: Path, epoch: int) -> None:
    with output.open("wb") as raw:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, compresslevel=9, mtime=epoch) as compressed:
            with tarfile.open(fileobj=compressed, mode="w", format=tarfile.USTAR_FORMAT) as archive:
                for path in archive_paths(source):
                    info = archive.gettarinfo(str(path), archive_name(source, path))
                    info.uid = 0
                    info.gid = 0
                    info.uname = ""
                    info.gname = ""
                    info.mtime = epoch
                    info.mode = normalized_mode(path)
                    if info.isreg():
                        with path.open("rb") as payload:
                            archive.addfile(info, payload)
                    else:
                        archive.addfile(info)


def create_zip(source: Path, output: Path, epoch: int) -> None:
    timestamp = tuple(__import__("time").gmtime(max(epoch, ZIP_EPOCH))[:6])
    with zipfile.ZipFile(
        output,
        mode="w",
        compression=zipfile.ZIP_DEFLATED,
        compresslevel=9,
        strict_timestamps=False,
    ) as archive:
        for path in archive_paths(source):
            name = archive_name(source, path)
            is_directory = path.is_dir()
            if is_directory:
                name += "/"
            info = zipfile.ZipInfo(name, timestamp)
            info.create_system = 3
            file_type = stat.S_IFDIR if is_directory else stat.S_IFREG
            info.external_attr = (file_type | normalized_mode(path)) << 16
            info.compress_type = zipfile.ZIP_DEFLATED
            if is_directory:
                archive.writestr(info, b"")
            else:
                archive.writestr(info, path.read_bytes())


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--format", choices=("tar.gz", "zip"), required=True)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--epoch", type=int, required=True)
    args = parser.parse_args()

    source = args.source.resolve(strict=True)
    if not source.is_dir():
        parser.error("--source must be a directory")
    if args.epoch < 0:
        parser.error("--epoch must be non-negative")

    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_name(f".{output.name}.tmp")
    try:
        if args.format == "tar.gz":
            create_tar_gz(source, temporary, args.epoch)
        else:
            create_zip(source, temporary, args.epoch)
        os.replace(temporary, output)
    finally:
        temporary.unlink(missing_ok=True)


if __name__ == "__main__":
    main()
