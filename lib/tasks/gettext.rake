namespace :gettext do
  def files_to_translate
    Dir.glob("{app,lib,config,locale}/**/*.{rb,erb,haml,slim,rhtml,rjs}")
  end
end
