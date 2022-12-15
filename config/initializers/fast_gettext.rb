FastGettext.add_text_domain 'app', :path => 'locale', :type => :po
FastGettext.default_available_locales = ['en-US','pt-BR', "en_US", "pt_BR"] #all you want to allow
FastGettext.default_text_domain = 'app'
# Workaround
I18n::Backend::Simple.send(:include, I18n::Backend::Fallbacks)
