class RsGitFsmonitor < Formula
  desc "Git fsmonitor hook written in Rust"
  homepage "https://github.com/jgavris/rs-git-fsmonitor"

  url "https://github.com/jgavris/rs-git-fsmonitor/releases/download/v0.1.2/rs-git-fsmonitor"
  sha256 "69321d0b40b9adcc10b495190d63096e51c6506b2d94bc27395848223a06bf21"

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
