namespace "gettext" do
  desc "Update pot/po files."
  task :updatepo do
    require 'gettext_rails/tools'
    GetText.update_pofiles("golaberto", Dir.glob("{app,lib,bin}/**/*.{rb,erb,rjs,rhtml}"), "golaberto 1.0.0")
  end

  desc "Create mo-files"
  task :makemo do
    require 'gettext_rails/tools'
    GetText.create_mofiles
  end
end
