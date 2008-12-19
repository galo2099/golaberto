require 'rubygems'

#require 'quilt'
require '../lib/quilt'


# 15.times do |i|
#  icon = Quilt::Identicon.new rand(10000).to_s, :size => 100
#  icon.write "file/#{i}.png"
# end

icon = Quilt::Identicon.new 'motemen'
icon.write "file/motemen.png"

icon = Quilt::Identicon.new 'motemen', :size => 30
icon.write "file/motemen30.png"

icon = Quilt::Identicon.new 'motemen', :size => 40
icon.write "file/motemen40.png"

icon = Quilt::Identicon.new 'motemen', :size => 100
icon.write "file/motemen100.png"

icon = Quilt::Identicon.new 'motemen', :size => 150
icon.write "file/motemen150.png"

