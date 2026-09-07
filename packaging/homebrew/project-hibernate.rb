# Homebrew cask. Lives in a tap (e.g. github.com/<owner>/homebrew-tap/Casks/project-hibernate.rb)
# until the project qualifies for homebrew-cask. Update `version` and `sha256`
# per release; `brew fetch --cask` prints the checksum.
cask "project-hibernate" do
  version "0.1.0"
  sha256 arm:   "REPLACE_WITH_SHA256_OF_ARM64_DMG",
         intel: "REPLACE_WITH_SHA256_OF_X64_DMG"

  arch arm: "aarch64", intel: "x64"

  url "https://github.com/lewisjohnvillamor/developer-project-cleanup/releases/download/v#{version}/Project.Hibernate_#{version}_#{arch}.dmg"
  name "Project Hibernate"
  desc "Reclaim disk space from dormant developer projects without deleting source code"
  homepage "https://github.com/lewisjohnvillamor/developer-project-cleanup"

  livecheck do
    url :url
    strategy :github_latest
  end

  app "Project Hibernate.app"

  zap trash: [
    "~/Library/Application Support/ProjectHibernate",
    "~/Library/Caches/dev.projecthibernate.app",
    "~/Library/Preferences/dev.projecthibernate.app.plist",
  ]
end
