# Homebrew formula for the CLI, built from source. Put it in a tap as
# Formula/hibernate-cli.rb. `brew install <owner>/tap/hibernate-cli`.
class HibernateCli < Formula
  desc "Scan developer projects and hibernate regeneratable build artifacts"
  homepage "https://github.com/lewisjohnvillamor/developer-project-cleanup"
  url "https://github.com/lewisjohnvillamor/developer-project-cleanup/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "REPLACE_WITH_SHA256_OF_SOURCE_TARBALL"
  license "MIT"
  head "https://github.com/lewisjohnvillamor/developer-project-cleanup.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: "crates/hibernate-cli")
  end

  test do
    (testpath/"demo").mkpath
    (testpath/"demo/package.json").write "{}"
    assert_match "demo", shell_output("#{bin}/hibernate scan #{testpath}")
  end
end
