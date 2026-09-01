#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 3 ]; then
  echo "usage: $0 <repository-root> <suite> <signing-key-fingerprint>" >&2
  exit 64
fi

repository_root=$(realpath "$1")
suite=$2
signing_key=$3
distribution_root="$repository_root/dists/$suite"

if ! [[ "$suite" =~ ^[a-z0-9][a-z0-9._-]*$ ]]; then
  echo "invalid suite: $suite" >&2
  exit 64
fi

mapfile -t packages < <(find "$repository_root/pool" -type f -name '*.deb' -print | LC_ALL=C sort)
if [ "${#packages[@]}" -eq 0 ]; then
  echo "repository pool contains no Debian packages" >&2
  exit 66
fi

for architecture in amd64 arm64; do
  packages_dir="$distribution_root/main/binary-$architecture"
  mkdir -p "$packages_dir"
  (
    cd "$repository_root"
    dpkg-scanpackages --arch "$architecture" pool /dev/null
  ) > "$packages_dir/Packages"
  if [ ! -s "$packages_dir/Packages" ]; then
    echo "repository pool contains no $architecture package" >&2
    exit 66
  fi
  gzip -n -9 -c "$packages_dir/Packages" > "$packages_dir/Packages.gz"
done

release="$distribution_root/Release"
release_date=$(date -u -R)
cat > "$release" <<EOF
Origin: oxvif
Label: oxvif
Suite: $suite
Codename: $suite
Date: $release_date
Architectures: amd64 arm64
Components: main
Description: Signed project repository for the oxvif CLI
Acquire-By-Hash: no
SHA256:
EOF

while IFS= read -r relative_path; do
  digest=$(sha256sum "$distribution_root/$relative_path" | cut -d' ' -f1)
  size=$(stat --format='%s' "$distribution_root/$relative_path")
  printf ' %s %16s %s\n' "$digest" "$size" "$relative_path" >> "$release"
done < <(
  cd "$distribution_root"
  find main -type f -print | LC_ALL=C sort
)

gpg --batch --yes --local-user "$signing_key" --digest-algo SHA256 \
  --armor --detach-sign --output "$distribution_root/Release.gpg" "$release"
gpg --batch --yes --local-user "$signing_key" --digest-algo SHA256 \
  --clearsign --output "$distribution_root/InRelease" "$release"

