class Reeknote < Formula
  desc "Command-line Evernote client"
  homepage "https://gitlab.com/vitaly-zdanevich/reeknote"
  url "https://gitlab.com/vitaly-zdanevich/reeknote.git",
      tag: "0.8.4"
  version "0.8.4"
  license "GPL-3.0-only"
  head "https://gitlab.com/vitaly-zdanevich/reeknote.git", branch: "master"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: ".")
  end

  def caveats
    <<~EOS
      Audio playback from notes requires mpv:
        brew install mpv

      Inline image display works in Kitty-compatible terminals.
    EOS
  end

  test do
    assert_match "Reeknote - a command line client for Evernote.", shell_output("#{bin}/reeknote")
    assert_match "Usage: rnsync", shell_output("#{bin}/rnsync --help")
  end
end
