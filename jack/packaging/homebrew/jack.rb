class Jack < Formula
  desc "Tag and sort movie and TV files using TMDB and TheTVDB metadata"
  homepage "https://github.com/am517/windups"
  url "https://github.com/am517/windups/archive/refs/tags/v0.2.0.tar.gz"
  sha256 "f15a9eeea46e2468841e21c0017f3ab387488ba9b017ba3886e10e12bbd1df03"
  license "MIT"
  head "https://github.com/am517/windups.git", branch: "main"

  depends_on "rust" => :build
  depends_on "atomicparsley"
  depends_on "exiftool"
  depends_on "mkvtoolnix"

  def install
    # Optional build-time API keys (never stored in the formula):
    #   HOMEBREW_JACK_TMDB_TOKEN=... HOMEBREW_JACK_TVDB_KEY=... HOMEBREW_JACK_OMDB_KEY=... brew install --build-from-source am517/tap/jack
    ENV["JACK_TMDB_TOKEN"] = ENV["HOMEBREW_JACK_TMDB_TOKEN"] if ENV["HOMEBREW_JACK_TMDB_TOKEN"].present?
    ENV["JACK_TVDB_KEY"] = ENV["HOMEBREW_JACK_TVDB_KEY"] if ENV["HOMEBREW_JACK_TVDB_KEY"].present?
    ENV["JACK_OMDB_KEY"] = ENV["HOMEBREW_JACK_OMDB_KEY"] if ENV["HOMEBREW_JACK_OMDB_KEY"].present?
    # windups is a Cargo workspace; Jack lives in jack/
    system "cargo", "install", *std_cargo_args(path: "jack")
    pkgshare.install "jack/config/jack.toml" => "config.toml.example"
  end

  def caveats
    <<~EOS
      The first run (or `jack setup`) makes ~/windups/jack/ (config, jack.db, backups/)
      and links ~/.local/bin/jack. Add your TMDB / TheTVDB / OMDb API keys to
      ~/windups/jack/config.toml; every setting is in #{opt_pkgshare}/config.toml.example
    EOS
  end

  test do
    assert_match "jack #{version}", shell_output("#{bin}/jack --version")
    assert_match "name = \"jack\"", shell_output("#{bin}/jack --capabilities")
  end
end
