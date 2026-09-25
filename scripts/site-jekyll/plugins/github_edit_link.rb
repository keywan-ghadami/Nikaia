# `{% github_edit_link "text" %}`, for builds outside GitHub Pages.
#
# `_layouts/default.html` puts an "Improve this page" link in every footer
# with this tag, which comes from `jekyll-github-metadata`. That plugin is not
# part of the build `scripts/site-build.sh` runs (the `Gemfile` one directory
# up says why), so this writes the same link from `site.github.repository_url` and
# `site.github.branch`, which the build script sets. GitHub Pages never loads
# this file: it builds in safe mode, and has the original tag.
module NikaiaSite
  class GitHubEditLink < Liquid::Tag
    TEXT = /\A\s*(?:"([^"]*)"|'([^']*)')\s*\z/

    def initialize(tag_name, markup, tokens)
      super
      found = markup.match(TEXT)
      @text = found && (found[1] || found[2])
    end

    def render(context)
      github = context.registers[:site].config["github"] || {}
      path = context.registers[:page]["path"].to_s.sub(%r{\A/}, "")
      url = "#{github["repository_url"]}/edit/#{github["branch"]}/#{path}"
      @text ? %(<a href="#{url}">#{@text}</a>) : url
    end
  end
end

Liquid::Template.register_tag("github_edit_link", NikaiaSite::GitHubEditLink)
