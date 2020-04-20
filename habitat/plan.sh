pkg_name=rs-git-fsmonitor
pkg_origin=jgavris
pkg_version="0.1.3"
pkg_maintainer="Jason Gavris <jgavris@gmail.com>"
pkg_license=("MIT")
pkg_upstream_url="https://github.com/jgavris/rs-git-fsmonitor"
pkg_deps=(
  core/bash
  core/glibc
  core/gcc-libs
  jarvus/watchman
)
pkg_build_deps=(
  core/rust
)
pkg_bin_dirs=(bin)

do_build() {
  LD_LIBRARY_PATH="$LD_RUN_PATH" cargo build --release
}

do_install() {
  cargo install --path . --root "${pkg_prefix}"

  build_line "Generating wrapper script for portable execution"
  pushd "${pkg_prefix}/bin" > /dev/null
  mkdir "../bin.real"
  mv -v "rs-git-fsmonitor" "../bin.real/rs-git-fsmonitor"

  cat <<EOF > "rs-git-fsmonitor"
#!$(pkg_path_for core/bash)/bin/bash -e

set -a
.  ${pkg_prefix}/RUNTIME_ENVIRONMENT
set +a

exec ${pkg_prefix}/bin.real/rs-git-fsmonitor "\$@"
EOF

  chmod -v 755 "rs-git-fsmonitor"

  popd > /dev/null
}
