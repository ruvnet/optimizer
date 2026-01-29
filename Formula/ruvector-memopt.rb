class RuvectorMemopt < Formula
  desc "Intelligent cross-platform memory optimizer with neural learning"
  homepage "https://github.com/ruvnet/optimizer"
  url "https://github.com/ruvnet/optimizer/archive/refs/tags/v0.5.0.tar.gz"
  sha256 ""  # TODO: Update after creating GitHub release
  license "MIT"
  head "https://github.com/ruvnet/optimizer.git", branch: "master"

  depends_on "rust" => :build

  def install
    system "cargo", "build", "--release", "--bin", "ruvector-memopt-macos"
    bin.install "target/release/ruvector-memopt-macos" => "ruvector-memopt"
  end

  service do
    run [opt_bin/"ruvector-memopt", "tray"]
    keep_alive false
    log_path var/"log/ruvector-memopt.log"
    error_log_path var/"log/ruvector-memopt.err"
  end

  def caveats
    <<~EOS
      To start the menu bar app:
        ruvector-memopt tray

      To start at login:
        brew services start ruvector-memopt

      To run a one-time optimization:
        ruvector-memopt optimize

      For full optimization (purge system caches):
        sudo ruvector-memopt optimize --aggressive

      The app will prompt for admin password when using Deep Clean
      from the menu bar without sudo.
    EOS
  end

  test do
    assert_match "Memory Status", shell_output("#{bin}/ruvector-memopt status")
  end
end
