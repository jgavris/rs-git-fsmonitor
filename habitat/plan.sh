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
  core/patchelf
  core/rust
)
pkg_bin_dirs=(bin)

do_build() {
  cargo build --release --verbose
}

do_install() {
  cargo install --path . --root "${pkg_prefix}" --verbose

  pushd "${pkg_prefix}/bin" > /dev/null

  build_line "Patching runpath in rust binary"
  patchelf --set-interpreter "$(pkg_path_for glibc)/lib/ld-linux-x86-64.so.2" --set-rpath "${LD_RUN_PATH}" "rs-git-fsmonitor"

  build_line "Generating wrapper script for portable execution"
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

do_strip() {
  # skip stripping the rust binary, it breaks it
  return 0
}
