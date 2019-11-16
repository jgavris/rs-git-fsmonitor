pkg_name=rs-git-fsmonitor
pkg_origin=jgavris
pkg_version="0.1.2"
pkg_maintainer="Jason Gavris <jgavris@gmail.com>"
pkg_license=("MIT")
pkg_upstream_url="https://github.com/jgavris/rs-git-fsmonitor"
pkg_deps=(
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
  cargo install --root "${pkg_prefix}"
}
