require File.expand_path(File.join(File.dirname(__FILE__), 'lib', 'riff'))

ActiveRecord::Base.send :include, Riff
