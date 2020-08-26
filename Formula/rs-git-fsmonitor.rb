class RsGitFsmonitor < Formula
  desc "Git fsmonitor hook written in Rust"
  homepage "https://github.com/jgavris/rs-git-fsmonitor"

  url "https://github.com/jgavris/rs-git-fsmonitor/releases/download/v0.1.3/rs-git-fsmonitor"
  sha256 "a221bbd9d44a23190d913b6f7e97d00dfac5f83452fa419122bad73b147af473"

  depends_on "watchman"

  def install
    bin.install "rs-git-fsmonitor"
  end

  def post_install
    ohai "Run `git config core.fsmonitor rs-git-fsmonitor` to install!"
  end

  test do
    system "git init . && git config core.fsmonitor rs-git-fsmonitor && git status"
  end
end
